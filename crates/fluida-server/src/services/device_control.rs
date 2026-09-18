use crate::error::ApiError;
use axum::http::StatusCode;
use serde::Serialize;
use std::{
    fs,
    future::Future,
    net::SocketAddr,
    path::{Path, PathBuf},
    pin::Pin,
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command, time::timeout};

const PACTL_TIMEOUT: Duration = Duration::from_secs(2);
type AdapterFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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

pub(crate) trait DeviceAdapter: Send + Sync {
    fn state(&self) -> AdapterFuture<'_, DeviceState>;
    fn set_volume(&self, value: u8) -> AdapterFuture<'_, Result<(), String>>;
    fn set_brightness(&self, value: u8) -> AdapterFuture<'_, Result<(), String>>;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalAdministrator(());

impl LocalAdministrator {
    pub(crate) fn authenticate(peer: Option<SocketAddr>) -> Result<Self, ApiError> {
        if peer.is_some_and(|peer| peer.ip().is_loopback()) {
            Ok(Self(()))
        } else {
            Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "LOCAL_ADMIN_REQUIRED",
                "Device changes require a local administrator",
            ))
        }
    }
}

#[derive(Clone)]
pub struct DeviceControl {
    adapter: Arc<dyn DeviceAdapter>,
}

impl DeviceControl {
    pub(crate) fn new(adapter: Arc<dyn DeviceAdapter>) -> Self {
        Self { adapter }
    }

    pub async fn state(&self) -> DeviceState {
        self.adapter.state().await
    }

    pub(crate) async fn set(
        &self,
        _administrator: &LocalAdministrator,
        kind: &str,
        value: u8,
    ) -> Result<DeviceState, ApiError> {
        if value > 100 {
            return Err(ApiError::validation(
                "Device values must be between 0 and 100",
            ));
        }
        let result = match kind {
            "volume" => self.adapter.set_volume(value).await,
            "brightness" => self.adapter.set_brightness(value).await,
            _ => return Err(ApiError::validation("Unknown device control")),
        };
        result.map_err(|message| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEVICE_CONTROL_UNAVAILABLE",
                message,
            )
        })?;
        Ok(self.state().await)
    }
}

pub fn native(enabled: bool) -> DeviceControl {
    let container = Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists();
    let allowed = enabled && !container;
    if allowed && cfg!(target_os = "linux") {
        DeviceControl::new(Arc::new(LinuxAdapter::discover()))
    } else {
        let reason = if !enabled {
            "Device control is disabled by the administrator"
        } else if container {
            "Device control is unavailable in containers"
        } else {
            "This operating system is unsupported"
        };
        DeviceControl::new(Arc::new(UnavailableAdapter(reason.into())))
    }
}

struct UnavailableAdapter(String);
impl DeviceAdapter for UnavailableAdapter {
    fn state(&self) -> AdapterFuture<'_, DeviceState> {
        Box::pin(async move {
            let unavailable = || DeviceCapability {
                supported: false,
                value: None,
                reason: Some(self.0.clone()),
            };
            DeviceState {
                volume: unavailable(),
                brightness: unavailable(),
            }
        })
    }
    fn set_volume(&self, _: u8) -> AdapterFuture<'_, Result<(), String>> {
        Box::pin(async move { Err(self.0.clone()) })
    }
    fn set_brightness(&self, _: u8) -> AdapterFuture<'_, Result<(), String>> {
        Box::pin(async move { Err(self.0.clone()) })
    }
}

struct LinuxAdapter {
    backlight: Option<PathBuf>,
}
impl LinuxAdapter {
    fn discover() -> Self {
        let backlight = fs::read_dir("/sys/class/backlight")
            .ok()
            .and_then(|mut rows| rows.find_map(|row| row.ok().map(|row| row.path())));
        Self { backlight }
    }
    fn brightness(&self) -> Result<u8, String> {
        let path = self
            .backlight
            .as_ref()
            .ok_or("No writable display backlight was detected")?;
        let current: u64 = read_number(&path.join("brightness"))?;
        let max: u64 = read_number(&path.join("max_brightness"))?;
        if max == 0 {
            return Err("The display reported an invalid maximum brightness".into());
        }
        Ok(((current * 100 / max).min(100)) as u8)
    }
    async fn volume() -> Result<u8, String> {
        let mut command = Command::new("pactl");
        command
            .args(["get-sink-volume", "@DEFAULT_SINK@"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|_| "PipeWire/PulseAudio control is unavailable")?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or("PipeWire/PulseAudio control is unavailable")?;
        let mut bytes = Vec::new();
        let result = timeout(PACTL_TIMEOUT, async {
            let (read, status) = tokio::join!(stdout.read_to_end(&mut bytes), child.wait());
            read.map_err(|_| "PipeWire/PulseAudio control is unavailable")?;
            status.map_err(|_| "PipeWire/PulseAudio control is unavailable")
        })
        .await;
        let status = match result {
            Ok(result) => result?,
            Err(_) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err("The audio server timed out while reading volume".into());
            }
        };
        if !status.success() {
            return Err("The audio server rejected the volume request".into());
        }
        parse_volume(&String::from_utf8_lossy(&bytes))
    }
}
fn parse_volume(text: &str) -> Result<u8, String> {
    text.split_whitespace()
        .find_map(|part| {
            part.strip_suffix('%')
                .and_then(|value| value.parse::<u8>().ok())
                .map(|value| value.min(100))
        })
        .ok_or("The audio server returned an invalid volume".into())
}
fn read_number(path: &Path) -> Result<u64, String> {
    fs::read_to_string(path)
        .map_err(|_| "Display backlight permissions are unavailable")?
        .trim()
        .parse()
        .map_err(|_| "The display returned an invalid brightness".into())
}
impl DeviceAdapter for LinuxAdapter {
    fn state(&self) -> AdapterFuture<'_, DeviceState> {
        Box::pin(async move {
            fn capability(result: Result<u8, String>) -> DeviceCapability {
                match result {
                    Ok(value) => DeviceCapability {
                        supported: true,
                        value: Some(value),
                        reason: None,
                    },
                    Err(reason) => DeviceCapability {
                        supported: false,
                        value: None,
                        reason: Some(reason),
                    },
                }
            }
            DeviceState {
                volume: capability(Self::volume().await),
                brightness: capability(self.brightness()),
            }
        })
    }
    fn set_volume(&self, value: u8) -> AdapterFuture<'_, Result<(), String>> {
        Box::pin(async move {
            let mut child = Command::new("pactl")
                .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{value}%")])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .map_err(|_| "PipeWire/PulseAudio control is unavailable")?;
            let status = match timeout(PACTL_TIMEOUT, child.wait()).await {
                Ok(result) => result.map_err(|_| "PipeWire/PulseAudio control is unavailable")?,
                Err(_) => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err("The audio server timed out while changing volume".into());
                }
            };
            if status.success() {
                Ok(())
            } else {
                Err("The audio server rejected the volume change".into())
            }
        })
    }
    fn set_brightness(&self, value: u8) -> AdapterFuture<'_, Result<(), String>> {
        Box::pin(async move {
            let path = self
                .backlight
                .as_ref()
                .ok_or("No writable display backlight was detected")?;
            let max = read_number(&path.join("max_brightness"))?;
            fs::write(
                path.join("brightness"),
                ((max * value as u64 / 100).max(1)).to_string(),
            )
            .map_err(|_| "Display backlight permissions are unavailable".into())
        })
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
        fn state(&self) -> AdapterFuture<'_, DeviceState> {
            Box::pin(async move { self.state.clone() })
        }
        fn set_volume(&self, value: u8) -> AdapterFuture<'_, Result<(), String>> {
            Box::pin(async move {
                if let Some(error) = &self.failure {
                    return Err(error.clone());
                }
                self.writes.lock().unwrap().push(("volume".into(), value));
                Ok(())
            })
        }
        fn set_brightness(&self, value: u8) -> AdapterFuture<'_, Result<(), String>> {
            Box::pin(async move {
                if let Some(error) = &self.failure {
                    return Err(error.clone());
                }
                self.writes
                    .lock()
                    .unwrap()
                    .push(("brightness".into(), value));
                Ok(())
            })
        }
    }
    fn capability(supported: bool) -> DeviceCapability {
        DeviceCapability {
            supported,
            value: supported.then_some(50),
            reason: (!supported).then(|| "unavailable".into()),
        }
    }
    fn adapter(supported: bool, failure: Option<&str>) -> Arc<MockAdapter> {
        Arc::new(MockAdapter {
            state: DeviceState {
                volume: capability(supported),
                brightness: capability(supported),
            },
            failure: failure.map(str::to_owned),
            writes: Mutex::new(vec![]),
        })
    }
    fn administrator() -> LocalAdministrator {
        LocalAdministrator::authenticate(Some("127.0.0.1:3000".parse().unwrap())).unwrap()
    }
    #[tokio::test]
    async fn mock_reports_supported_and_unavailable_capabilities() {
        assert!(
            DeviceControl::new(adapter(true, None))
                .state()
                .await
                .volume
                .supported
        );
        let unavailable = DeviceControl::new(adapter(false, None)).state().await;
        assert!(!unavailable.brightness.supported);
        assert_eq!(
            unavailable.brightness.reason.as_deref(),
            Some("unavailable")
        );
    }
    #[tokio::test]
    async fn mock_applies_supported_changes() {
        let mock = adapter(true, None);
        DeviceControl::new(mock.clone())
            .set(&administrator(), "volume", 42)
            .await
            .unwrap();
        assert_eq!(
            mock.writes.lock().unwrap().as_slice(),
            &[("volume".into(), 42)]
        );
    }
    #[tokio::test]
    async fn mock_surfaces_adapter_failures() {
        let error = DeviceControl::new(adapter(true, Some("permission denied")))
            .set(&administrator(), "brightness", 80)
            .await
            .unwrap_err();
        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    }
    #[test]
    fn non_loopback_peers_are_not_local_administrators() {
        let error =
            LocalAdministrator::authenticate(Some("192.0.2.1:3000".parse().unwrap())).unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
    }
    #[test]
    fn pactl_percentages_are_clamped() {
        assert_eq!(
            parse_volume("Volume: front-left: 98304 / 150%").unwrap(),
            100
        );
        assert_eq!(parse_volume("Volume: front-left: 32768 / 50%").unwrap(), 50);
        assert_eq!(
            parse_volume("Volume: unknown").unwrap_err(),
            "The audio server returned an invalid volume"
        );
    }
}
