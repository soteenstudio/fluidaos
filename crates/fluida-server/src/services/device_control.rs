use crate::error::ApiError;
use axum::http::StatusCode;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCapability {
    pub supported: bool,
    pub value: Option<u8>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceState {
    pub volume: DeviceCapability,
    pub brightness: DeviceCapability,
}

pub trait DeviceAdapter: Send + Sync {
    fn state(&self) -> DeviceState;
    fn set_volume(&self, value: u8) -> Result<(), String>;
    fn set_brightness(&self, value: u8) -> Result<(), String>;
}

#[derive(Clone)]
pub struct DeviceControl {
    adapter: Arc<dyn DeviceAdapter>,
    mutation_allowed: bool,
}

impl DeviceControl {
    pub fn new(adapter: Arc<dyn DeviceAdapter>, mutation_allowed: bool) -> Self {
        Self {
            adapter,
            mutation_allowed,
        }
    }

    pub fn state(&self) -> DeviceState {
        self.adapter.state()
    }

    pub fn set(&self, kind: &str, value: u8) -> Result<DeviceState, ApiError> {
        if !self.mutation_allowed {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "DEVICE_CONTROL_DISABLED",
                "Local device control is disabled",
            ));
        }
        if value > 100 {
            return Err(ApiError::validation("Device values must be between 0 and 100"));
        }
        let result = match kind {
            "volume" => self.adapter.set_volume(value),
            "brightness" => self.adapter.set_brightness(value),
            _ => return Err(ApiError::validation("Unknown device control")),
        };
        result.map_err(|message| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEVICE_CONTROL_UNAVAILABLE",
                message,
            )
        })?;
        Ok(self.state())
    }
}

pub fn native(enabled: bool, host: &str) -> DeviceControl {
    let local = matches!(host, "127.0.0.1" | "::1" | "localhost");
    let container = Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists();
    let allowed = enabled && local && !container;
    if allowed && cfg!(target_os = "linux") {
        DeviceControl::new(Arc::new(LinuxAdapter::discover()), true)
    } else {
        let reason = if !enabled {
            "Device control is disabled by the administrator"
        } else if !local {
            "Device control requires a loopback-only server"
        } else if container {
            "Device control is unavailable in containers"
        } else {
            "This operating system is unsupported"
        };
        DeviceControl::new(Arc::new(UnavailableAdapter(reason.into())), false)
    }
}

struct UnavailableAdapter(String);
impl DeviceAdapter for UnavailableAdapter {
    fn state(&self) -> DeviceState {
        let unavailable = || DeviceCapability {
            supported: false,
            value: None,
            reason: Some(self.0.clone()),
        };
        DeviceState {
            volume: unavailable(),
            brightness: unavailable(),
        }
    }
    fn set_volume(&self, _: u8) -> Result<(), String> {
        Err(self.0.clone())
    }
    fn set_brightness(&self, _: u8) -> Result<(), String> {
        Err(self.0.clone())
    }
}

struct LinuxAdapter {
    backlight: Option<PathBuf>,
}
impl LinuxAdapter {
    fn discover() -> Self {
        let backlight = fs::read_dir("/sys/class/backlight").ok().and_then(|mut rows| rows.find_map(|row| row.ok().map(|row| row.path())));
        Self { backlight }
    }
    fn brightness(&self) -> Result<u8, String> {
        let path = self.backlight.as_ref().ok_or("No writable display backlight was detected")?;
        let current: u64 = read_number(&path.join("brightness"))?;
        let max: u64 = read_number(&path.join("max_brightness"))?;
        if max == 0 { return Err("The display reported an invalid maximum brightness".into()); }
        Ok(((current * 100 / max).min(100)) as u8)
    }
    fn volume() -> Result<u8, String> {
        let output = Command::new("pactl").args(["get-sink-volume", "@DEFAULT_SINK@"]).output().map_err(|_| "PipeWire/PulseAudio control is unavailable")?;
        if !output.status.success() { return Err("The audio server rejected the volume request".into()); }
        let text = String::from_utf8_lossy(&output.stdout);
        text.split_whitespace().find_map(|part| part.strip_suffix('%').and_then(|v| v.parse().ok())).ok_or("The audio server returned an invalid volume".into())
    }
}
fn read_number(path: &Path) -> Result<u64, String> {
    fs::read_to_string(path).map_err(|_| "Display backlight permissions are unavailable")?.trim().parse().map_err(|_| "The display returned an invalid brightness".into())
}
impl DeviceAdapter for LinuxAdapter {
    fn state(&self) -> DeviceState {
        fn capability(result: Result<u8, String>) -> DeviceCapability { match result { Ok(value) => DeviceCapability { supported: true, value: Some(value), reason: None }, Err(reason) => DeviceCapability { supported: false, value: None, reason: Some(reason) } } }
        DeviceState { volume: capability(Self::volume()), brightness: capability(self.brightness()) }
    }
    fn set_volume(&self, value: u8) -> Result<(), String> {
        let status = Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{value}%")]).status().map_err(|_| "PipeWire/PulseAudio control is unavailable")?;
        if status.success() { Ok(()) } else { Err("The audio server rejected the volume change".into()) }
    }
    fn set_brightness(&self, value: u8) -> Result<(), String> {
        let path = self.backlight.as_ref().ok_or("No writable display backlight was detected")?;
        let max = read_number(&path.join("max_brightness"))?;
        fs::write(path.join("brightness"), ((max * value as u64 / 100).max(1)).to_string()).map_err(|_| "Display backlight permissions are unavailable".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockAdapter {
        state: DeviceState,
        failure: Option<String>,
        writes: Mutex<Vec<(String, u8)>>,
    }
    impl DeviceAdapter for MockAdapter {
        fn state(&self) -> DeviceState { self.state.clone() }
        fn set_volume(&self, value: u8) -> Result<(), String> {
            if let Some(error) = &self.failure { return Err(error.clone()); }
            self.writes.lock().unwrap().push(("volume".into(), value)); Ok(())
        }
        fn set_brightness(&self, value: u8) -> Result<(), String> {
            if let Some(error) = &self.failure { return Err(error.clone()); }
            self.writes.lock().unwrap().push(("brightness".into(), value)); Ok(())
        }
    }
    fn capability(supported: bool) -> DeviceCapability {
        DeviceCapability { supported, value: supported.then_some(50), reason: (!supported).then(|| "unavailable".into()) }
    }
    fn adapter(supported: bool, failure: Option<&str>) -> Arc<MockAdapter> {
        Arc::new(MockAdapter { state: DeviceState { volume: capability(supported), brightness: capability(supported) }, failure: failure.map(str::to_owned), writes: Mutex::new(vec![]) })
    }
    #[test]
    fn mock_reports_supported_and_unavailable_capabilities() {
        assert!(DeviceControl::new(adapter(true, None), true).state().volume.supported);
        let unavailable = DeviceControl::new(adapter(false, None), true).state();
        assert!(!unavailable.brightness.supported);
        assert_eq!(unavailable.brightness.reason.as_deref(), Some("unavailable"));
    }
    #[test]
    fn mock_applies_supported_changes() {
        let mock = adapter(true, None);
        DeviceControl::new(mock.clone(), true).set("volume", 42).unwrap();
        assert_eq!(mock.writes.lock().unwrap().as_slice(), &[("volume".into(), 42)]);
    }
    #[test]
    fn mock_surfaces_adapter_failures() {
        let error = DeviceControl::new(adapter(true, Some("permission denied")), true).set("brightness", 80).unwrap_err();
        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    }
    #[test]
    fn mutations_are_rejected_when_not_authorized() {
        let error = DeviceControl::new(adapter(true, None), false).set("volume", 20).unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
    }
}
