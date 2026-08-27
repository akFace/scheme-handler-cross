#![cfg_attr(windows, windows_subsystem = "windows")]

mod platform;

use base64::{engine::general_purpose, Engine as _};
use eframe::egui;
use flate2::read::GzDecoder;
use rfd::{FileDialog, MessageDialog, MessageLevel};
use serde::{Deserialize, Serialize};
use std::{
    env,
    error::Error,
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

#[cfg(target_os = "macos")]
use std::{sync::mpsc, time::{Duration, Instant}};

use thiserror::Error;
use urlencoding::decode;

const SCHEME: &str = "ush";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Error)]
enum AppError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("gzip decompression failed: {0}")]
    Gzip(#[from] std::io::Error),
    #[error("UTF-8 decode failed: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("application not found: {0}")]
    AppNotFound(String),
    #[error("application path is empty")]
    EmptyAppPath,
    #[error("command line is empty")]
    EmptyCommand,
    #[error("command failed: {0}")]
    Command(String),
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct Config {
    #[serde(default)]
    is_registry_added: bool,
    #[serde(default)]
    apps: Vec<AppConfig>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct AppConfig {
    name: String,
    path: Option<String>,
}

fn config_path() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        return dir.join("scheme-handler").join(CONFIG_FILE);
    }

    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_FILE)
}

impl Config {
    fn load() -> Self {
        let path = config_path();
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<(), std::io::Error> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_vec_pretty(self).expect("Config serialization cannot fail");
        File::create(&tmp)?.write_all(&data)?;
        fs::rename(tmp, path)?;
        Ok(())
    }
}

fn parse_url(input: &str) -> Result<(String, String), AppError> {
    let normalized = input.trim();
    let rest = normalized
        .strip_prefix("ush://")
        .ok_or_else(|| AppError::InvalidUrl("input does not start with ush://".into()))?;

    let (app, payload) = rest
        .split_once('?')
        .ok_or_else(|| AppError::InvalidUrl("expected ush://<app>?<gzip-base64>".into()))?;

    let app = decode(app.trim_end_matches('/'))
        .map_err(|e| AppError::InvalidUrl(e.to_string()))?
        .into_owned();
    if app.trim().is_empty() {
        return Err(AppError::InvalidUrl("application name is empty".into()));
    }
    if payload.trim().is_empty() {
        return Err(AppError::InvalidUrl("gzip payload is empty".into()));
    }
    Ok((app, payload.trim_end_matches('/').to_string()))
}

fn decompress_payload(payload: &str) -> Result<String, AppError> {
    let bytes = general_purpose::STANDARD.decode(payload)?;
    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut output = Vec::new();
    decoder.read_to_end(&mut output)?;
    Ok(String::from_utf8(output)?)
}

#[cfg(windows)]
fn run_target(path: &str, command_line: &str) -> Result<(), AppError> {
    use std::os::windows::process::CommandExt;

    Command::new(path)
        .raw_arg(command_line)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::Command(format!("{path}: {e}")))
}

#[cfg(target_os = "macos")]
fn run_target(path: &str, command_line: &str) -> Result<(), AppError> {
    let args = shell_words::split(command_line)
        .map_err(|e| AppError::Command(format!("cannot parse command arguments: {e}")))?;
    if args.is_empty() {
        return Err(AppError::EmptyCommand);
    }

    let path_buf = std::path::Path::new(path);
    let is_app_bundle = path_buf
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("app"))
        .unwrap_or(false);

    if is_app_bundle {
        // Use LaunchServices instead of executing the .app directory itself.
        // This preserves normal macOS application launching behavior.
        Command::new("/usr/bin/open")
            .arg("-a")
            .arg(path)
            .arg("--args")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::Command(format!("open -a {path}: {e}")))
    } else {
        Command::new(path)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::Command(format!("{path}: {e}")))
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn run_target(path: &str, command_line: &str) -> Result<(), AppError> {
    let args = shell_words::split(command_line)
        .map_err(|e| AppError::Command(format!("cannot parse command arguments: {e}")))?;
    if args.is_empty() {
        return Err(AppError::EmptyCommand);
    }

    Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::Command(format!("{path}: {e}")))
}

fn execute_url(input: &str) -> Result<(), AppError> {
    let (app_name, payload) = parse_url(input)?;
    let command_line = decompress_payload(&payload)?;
    if command_line.trim().is_empty() {
        return Err(AppError::EmptyCommand);
    }

    let config = Config::load();
    let app = config
        .apps
        .iter()
        .find(|app| app.name.trim() == app_name.trim())
        .ok_or_else(|| AppError::AppNotFound(app_name.clone()))?;
    let path = app.path.as_deref().unwrap_or("").trim();
    if path.is_empty() {
        return Err(AppError::EmptyAppPath);
    }

    println!("Executing: {path} {command_line}");
    run_target(path, &command_line)
}

fn show_error(title: &str, error: impl std::fmt::Display) {
    let _ = MessageDialog::new()
        .set_title(title)
        .set_description(error.to_string())
        .set_level(MessageLevel::Error)
        .show();
}

fn show_info(title: &str, message: &str) {
    let _ = MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(MessageLevel::Info)
        .show();
}

#[cfg(target_os = "macos")]
fn start_macos_url_worker() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (sender, receiver) = mpsc::channel::<String>();

    std::thread::Builder::new()
        .name("ush-url-worker".into())
        .spawn(move || {
            while let Ok(url) = receiver.recv() {
                if let Err(error) = execute_url(&url) {
                    // URL-triggered execution must not depend on the GUI being
                    // visible or focused. Log errors instead of opening a dialog.
                    eprintln!("URL Scheme Handler: {error}");
                }
            }
        })?;

    platform::install_url_handler(sender)?;
    Ok(())
}

fn open_settings() {
    #[cfg(target_os = "macos")]
    let initial_visible = !platform::url_was_received();

    let options = eframe::NativeOptions {
        viewport: {
            let viewport = egui::ViewportBuilder::default()
                .with_inner_size([700.0, 350.0])
                .with_min_inner_size([500.0, 260.0])
                .with_resizable(true);
            #[cfg(target_os = "macos")]
            let viewport = viewport.with_visible(initial_visible);
            viewport
        },
        centered: true,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "scheme-handler",
        options,
        Box::new(|_cc| Ok(Box::new(UrlSchemeHandler::new()))),
    );
}

struct UrlSchemeHandler {
    config: Config,
    #[cfg(target_os = "macos")]
    started_at: Instant,
    #[cfg(target_os = "macos")]
    ui_visibility_decided: bool,
}

impl UrlSchemeHandler {
    fn new() -> Self {
        let mut config = Config::load();

        // Linux URL handlers are desktop-entry based. Register automatically
        // when the GUI starts so the user does not need to click "Add to
        // Registry" after every launch. For AppImage, platform::register_scheme
        // uses $APPIMAGE so the registered path remains valid after exit.
        #[cfg(target_os = "linux")]
        {
            if platform::register_scheme(SCHEME).is_ok() {
                config.is_registry_added = true;
                let _ = config.save();
            }
        }

        #[cfg(target_os = "macos")]
        {
            let received = platform::url_was_received();
            return Self {
                config,
                started_at: Instant::now(),
                ui_visibility_decided: received,
            };
        }

        #[cfg(not(target_os = "macos"))]
        Self { config }
    }

    fn persist(&self) {
        if let Err(e) = self.config.save() {
            show_error("Error", format!("Failed to save config: {e}"));
        }
    }
}

impl eframe::App for UrlSchemeHandler {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        {
            // URL invocations are processed by the native Apple Event callback
            // and a worker thread, not by egui's update loop. This means the
            // app can stay hidden/in the background and still launch the target.
            if !self.ui_visibility_decided {
                if platform::url_was_received() {
                    self.ui_visibility_decided = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                } else if self.started_at.elapsed() >= Duration::from_millis(800) {
                    self.ui_visibility_decided = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                } else {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);

                egui::ScrollArea::vertical().max_height(210.0).show(ui, |ui| {
                    let mut remove = None;
                    for index in 0..self.config.apps.len() {
                        ui.horizontal(|ui| {
                            let response = ui.add_sized(
                                [100.0, 30.0],
                                egui::TextEdit::singleline(&mut self.config.apps[index].name)
                                    .horizontal_align(egui::Align::Center),
                            );
                            if response.lost_focus() {
                                self.persist();
                            }

                            let button_width = (ui.available_width() - 45.0).max(100.0);
                            let label = self.config.apps[index]
                                .path
                                .as_deref()
                                .unwrap_or("Select Player Path");
                            if ui
                                .add_sized([button_width, 30.0], egui::Button::new(label))
                                .clicked()
                            {
                                let mut dialog = FileDialog::new();
                                #[cfg(windows)]
                                {
                                    dialog = dialog.add_filter("Executable", &["exe"]);
                                }
                                if let Some(path) = dialog.pick_file() {
                                    self.config.apps[index].path =
                                        Some(path.to_string_lossy().into_owned());
                                    self.persist();
                                }
                            }
                            if ui
                                .add_sized([35.0, 30.0], egui::Button::new("➖"))
                                .clicked()
                            {
                                remove = Some(index);
                            }
                        });
                        ui.add_space(8.0);
                    }
                    if let Some(index) = remove {
                        self.config.apps.remove(index);
                        self.persist();
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                if ui
                    .add_sized([ui.available_width(), 30.0], egui::Button::new("➕"))
                    .clicked()
                {
                    self.config.apps.push(AppConfig::default());
                    self.persist();
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let width = ui.available_width() / 2.0 - 5.0;
                    if ui
                        .add_sized(
                            [width, 30.0],
                            egui::Button::new(if self.config.is_registry_added {
                                "✅ Add to Registry"
                            } else {
                                "➕ Add to Registry"
                            }),
                        )
                        .clicked()
                    {
                        match platform::register_scheme(SCHEME) {
                            Ok(()) => {
                                self.config.is_registry_added = true;
                                self.persist();
                                show_info("Info", "Adding to registry success");
                            }
                            Err(e) => show_error("Error", e),
                        }
                    }
                    let remove_button = egui::Button::new("➖ Remove from Registry");
                    if ui
                        .add_enabled_ui(!cfg!(target_os = "macos"), |ui| {
                            ui.add_sized([width, 30.0], remove_button)
                        })
                        .inner
                        .clicked()
                    {
                        match platform::unregister_scheme(SCHEME) {
                            Ok(()) => {
                                self.config.is_registry_added = false;
                                self.persist();
                                show_info("Info", "Removing from registry success");
                            }
                            Err(e) => show_error("Error", e),
                        }
                    }
                });
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args: Vec<String> = env::args().collect();

    #[cfg(target_os = "macos")]
    if args.len() == 1 {
        // Install the native Apple Event handler before starting eframe. The
        // worker handles ush:// URLs independently of the GUI event loop.
        start_macos_url_worker()?;
        open_settings();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // Also refresh registration for direct URL launches. This is important
        // for AppImage: the application may have been moved since the last
        // registration, and $APPIMAGE gives us the current real file path.
        if let Err(e) = platform::register_scheme(SCHEME) {
            eprintln!("scheme-handler: Linux URL registration failed: {e}");
        }
    }

    match args.as_slice() {
        [_] => open_settings(),
        [_, command, input] if command == "run" => {
            if let Err(e) = execute_url(input) {
                show_error("scheme-handler", e);
            }
        }
        [_, input] if input.starts_with("ush://") => {
            if let Err(e) = execute_url(input) {
                show_error("scheme-handler", e);
            }
        }
        _ => {
            show_error(
                "scheme-handler",
                "Usage: scheme-handler run ush://<app_name>?<gzip_base64>",
            );
        }
    }

    Ok(())
}
