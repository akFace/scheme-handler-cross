use std::error::Error;

pub fn register_scheme(_scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> { Err("this platform is not supported".into()) }
pub fn unregister_scheme(_scheme: &str) -> Result<(), Box<dyn Error + Send + Sync>> { Err("this platform is not supported".into()) }
