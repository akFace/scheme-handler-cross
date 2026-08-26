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
use std::sync::mpsc::{self, Receiver};

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
        return dir.join("url-scheme-handler").join(CONFIG_FILE);
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
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::Command(format!("{path}: {e}")))
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
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::Command(format!("{path}: {e}")))
}

#[cfg(target_os = "macos")]
fn run_target(path: &str, command_line: &str) -> Result<(), AppError> {
    use std::path::Path;

    let args = shell_words::split(command_line)
        .map_err(|e| AppError::Command(format!("cannot parse command arguments: {e}")))?;
    if args.is_empty() {
        return Err(AppError::EmptyCommand);
    }

    // A macOS .app is a bundle/directory, not an executable file. Calling
    // Command::new("/Applications/mpv.app") therefore results in EACCES
    // (Permission denied). Resolve CFBundleExecutable and launch the actual
    // binary under Contents/MacOS instead.
    let target = Path::new(path);
    let executable = if target.extension().and_then(|v| v.to_str()) == Some("app") {
        let plist = target.join("Contents").join("Info.plist");
        let output = Command::new("/usr/bin/plutil")
            .args([
                "-extract",
                "CFBundleExecutable",
                "raw",
                "-o",
                "-",
                plist.to_string_lossy().as_ref(),
            ])
            .output()
            .map_err(|e| AppError::Command(format!(
                "cannot read {path}/Contents/Info.plist: {e}"
            )))?;

        if !output.status.success() {
            return Err(AppError::Command(format!(
                "cannot read CFBundleExecutable from {path}/Contents/Info.plist: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let executable_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if executable_name.is_empty() {
            return Err(AppError::Command(format!(
                "CFBundleExecutable is missing in {path}/Contents/Info.plist"
            )));
        }

        target.join("Contents").join("MacOS").join(executable_name)
    } else {
        target.to_path_buf()
    };

    if !executable.exists() {
        return Err(AppError::Command(format!(
            "macOS executable does not exist: {}",
            executable.display()
        )));
    }

    Command::new(&executable)
        .args(args)
        .stdin(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::Command(format!("{}: {e}", executable.display())))
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
    // Do not wait for GUI applications such as mpv to exit. The handler should
    // return immediately after successfully spawning the target process.
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

fn open_settings() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 350.0])
            .with_min_inner_size([500.0, 260.0])
            .with_resizable(true),
        centered: true,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "URL Scheme Handler",
        options,
        Box::new(|_cc| Ok(Box::new(UrlSchemeHandler::new()))),
    );
}

struct UrlSchemeHandler {
    config: Config,
    #[cfg(target_os = "macos")]
    url_receiver: Receiver<String>,
}

impl UrlSchemeHandler {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            let (sender, receiver) = mpsc::channel();
            if let Err(e) = platform::install_url_handler(sender) {
                eprintln!("macOS URL handler installation failed: {e}");
            }
            return Self {
                config: Config::load(),
                url_receiver: receiver,
            };
        }

        #[cfg(not(target_os = "macos"))]
        Self {
            config: Config::load(),
        }
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
        while let Ok(url) = self.url_receiver.try_recv() {
            if let Err(e) = execute_url(&url) {
                show_error("URL Scheme Handler", e);
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
                                { dialog = dialog.add_filter("Executable", &["exe"]); }
                                if let Some(path) = dialog.pick_file() {
                                    self.config.apps[index].path = Some(path.to_string_lossy().into_owned());
                                    self.persist();
                                }
                            }
                            if ui.add_sized([35.0, 30.0], egui::Button::new("➖")).clicked() {
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
                    if ui
                        .add_sized([width, 30.0], egui::Button::new("➖ Remove from Registry"))
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

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    match args.as_slice() {
        [_] => open_settings(),
        [_, command, input] if command == "run" => {
            if let Err(e) = execute_url(input) {
                show_error("URL Scheme Handler", e);
            }
        }
        [_, input] if input.starts_with("ush://") => {
            if let Err(e) = execute_url(input) {
                show_error("URL Scheme Handler", e);
            }
        }
        _ => {
            show_error(
                "URL Scheme Handler",
                "Usage: url-scheme-handler run ush://<app_name>?<gzip_base64>",
            );
        }
    }
    Ok(())
}
