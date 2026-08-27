use std::{env, error::Error, fs, path::PathBuf, process::Command};

fn applications_dir() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(xdg).join("applications"));
    }
    let home = env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/share/applications"))
}

fn desktop_path() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    Ok(applications_dir()?.join("scheme-handler-ush.desktop"))
}

fn quote_exec_arg(path: &str) -> String {
    // Desktop Entry Exec quoting: double quote and escape backslash, quote, dollar, backtick.
    let escaped = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");
    format!("\"{escaped}\"")
}

pub fn register_scheme(scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let exe = env::current_exe()?;
    let dir = applications_dir()?;
    fs::create_dir_all(&dir)?;
    let desktop = format!(
        "[Desktop Entry]\nVersion=1.0\nType=Application\nName=scheme-handler\nExec={} run %u\nNoDisplay=true\nTerminal=false\nMimeType=x-scheme-handler/{};\n",
        quote_exec_arg(&exe.to_string_lossy()),
        scheme
    );
    fs::write(desktop_path()?, desktop)?;

    let _ = Command::new("update-desktop-database").arg(&dir).status();
    let mime = format!("x-scheme-handler/{scheme}");
    let _ = Command::new("xdg-mime")
        .args(["default", "scheme-handler-ush.desktop", &mime])
        .status();
    Ok(())
}

pub fn unregister_scheme(scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let path = desktop_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    if let Ok(dir) = applications_dir() {
        let _ = Command::new("update-desktop-database").arg(dir).status();
    }
    // Do not point xdg-mime at a desktop file that we just removed.
    let _ = scheme;
    Ok(())
}
