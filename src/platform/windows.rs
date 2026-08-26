use std::{env, error::Error};
use winreg::{enums::*, RegKey};

pub fn register_scheme(scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let exe = env::current_exe()?;
    let command = format!("\"{}\" run \"%1\"", exe.display());
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(format!("Software\\Classes\\{scheme}"))?;
    key.set_value("", &format!("URL: {} Protocol", scheme.to_uppercase()))?;
    key.set_value("URL Protocol", &"")?;
    let (command_key, _) = key.create_subkey("shell\\open\\command")?;
    command_key.set_value("", &command)?;
    Ok(())
}

pub fn unregister_scheme(scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.delete_subkey_all(format!("Software\\Classes\\{scheme}")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}
