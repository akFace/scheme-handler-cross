use std::{env, error::Error, path::PathBuf, sync::{mpsc::Sender, OnceLock}};

extern "C" {
    fn ush_macos_install_url_handler(callback: extern "C" fn(*const std::os::raw::c_char));
}

static URL_SENDER: OnceLock<Sender<String>> = OnceLock::new();

extern "C" fn url_callback(ptr: *const std::os::raw::c_char) {
    if ptr.is_null() { return; }
    let value = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    if let Some(sender) = URL_SENDER.get() {
        let _ = sender.send(value);
    }
}

pub fn install_url_handler(sender: Sender<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    URL_SENDER.set(sender).map_err(|_| "macOS URL handler was already installed")?;
    unsafe { ush_macos_install_url_handler(url_callback); }
    Ok(())
}

fn bundle_info_plist() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let exe = env::current_exe()?;
    let contents = exe.parent().ok_or("cannot determine app bundle")?;
    if contents.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return Err("macOS URL registration requires a .app bundle (Contents/MacOS/<executable>)".into());
    }
    Ok(contents.parent().ok_or("invalid Contents directory")?.join("Info.plist"))
}

pub fn register_scheme(_scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    // The scheme is declared in the bundled Info.plist. Do not rewrite a
    // signed app's Info.plist at runtime; doing so invalidates its signature.
    let plist = bundle_info_plist()?;
    if !plist.exists() {
        return Err("Info.plist does not exist; build the .app with packaging/macos/Info.plist".into());
    }
    if let Some(bundle) = plist.parent().and_then(|p| p.parent()) {
        let ls = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
        let status = std::process::Command::new(ls).args(["-f"]).arg(bundle).status()?;
        if !status.success() {
            return Err("failed to register the app with Launch Services".into());
        }
    }
    Ok(())
}

pub fn unregister_scheme(_scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Launch Services does not expose a safe "remove only this CFBundleURLScheme"
    // operation. Keeping the bundled declaration immutable is also necessary for
    // signed/notarized apps. Removing the app from Launch Services entirely is
    // surprising, so report the limitation instead.
    Err("macOS URL schemes are declared in the signed app bundle. Remove/disable the CFBundleURLTypes entry in packaging/macos/Info.plist and rebuild the app to unregister ush.".into())
}

pub fn bundle_template() -> &'static str { "packaging/macos/Info.plist" }
