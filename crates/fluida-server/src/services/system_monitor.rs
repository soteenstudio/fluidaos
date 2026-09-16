use crate::{
    error::ApiError,
    services::{
        app_registry::AppRegistry, file_service::FileService, terminal_service::TerminalService,
    },
};
use chrono::Utc;
use serde_json::{json, Value};
#[derive(Clone)]
pub struct SystemMonitor {
    files: FileService,
    terminal: TerminalService,
    registry: AppRegistry,
}
impl SystemMonitor {
    pub fn new(files: FileService, terminal: TerminalService, registry: AppRegistry) -> Self {
        Self {
            files,
            terminal,
            registry,
        }
    }
    pub async fn snapshot(&self, owner: &str) -> Result<Value, ApiError> {
        let (bytes, files) = self.files.usage().await?;
        let jobs = self.terminal.list(owner).await;
        Ok(
            json!({"timestamp":Utc::now().to_rfc3339(),"uptime":0,"platform":std::env::consts::OS,"cpuUsage":{"user":0,"system":0},"memory":{"rss":0,"heapUsed":0,"heapTotal":0},"processId":std::process::id(),"storage":{"bytes":bytes,"files":files},"jobs":jobs,"apps":self.registry.list().await?.len()}),
        )
    }
}
