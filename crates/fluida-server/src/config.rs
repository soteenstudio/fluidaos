use std::{env, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub session_secret: String,
    pub trusted_origins: Vec<String>,
    pub storage_root: PathBuf,
    pub registry_root: PathBuf,
    pub project_root: PathBuf,
    pub device_control_enabled: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let project_root = env::current_dir().expect("working directory unavailable");
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            session_secret: env::var("SESSION_SECRET")
                .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string()),
            trusted_origins: env::var("TRUSTED_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect(),
            storage_root: project_root.join("os_storage"),
            registry_root: project_root.join("app_packages"),
            device_control_enabled: env::var("DEVICE_CONTROL_ENABLED")
                .is_ok_and(|value| value.eq_ignore_ascii_case("true") || value == "1"),
            project_root,
        }
    }
}
