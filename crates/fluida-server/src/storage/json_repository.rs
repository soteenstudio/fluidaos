use crate::error::ApiError;
use serde::{de::DeserializeOwned, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct JsonRepository<T> {
    file: PathBuf,
    fallback: T,
    lock: Arc<Mutex<()>>,
}
impl<T> JsonRepository<T>
where
    T: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    pub fn new(file: PathBuf, fallback: T) -> Self {
        Self {
            file,
            fallback,
            lock: Arc::new(Mutex::new(())),
        }
    }
    async fn read_unlocked(&self) -> Result<T, ApiError> {
        match tokio::fs::read(&self.file).await {
            Ok(data) => Ok(serde_json::from_slice(&data)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.write_unlocked(&self.fallback).await?;
                Ok(self.fallback.clone())
            }
            Err(e) => Err(e.into()),
        }
    }
    async fn write_unlocked(&self, value: &T) -> Result<T, ApiError> {
        if let Some(parent) = self.file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = self
            .file
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        tokio::fs::write(&temporary, serde_json::to_vec_pretty(value)?).await?;
        tokio::fs::rename(temporary, &self.file).await?;
        Ok(value.clone())
    }
    pub async fn read(&self) -> Result<T, ApiError> {
        let _guard = self.lock.lock().await;
        self.read_unlocked().await
    }
    pub async fn write(&self, value: &T) -> Result<T, ApiError> {
        let _guard = self.lock.lock().await;
        self.write_unlocked(value).await
    }
    pub async fn update<F>(&self, transform: F) -> Result<T, ApiError>
    where
        F: FnOnce(T) -> Result<T, ApiError>,
    {
        let _guard = self.lock.lock().await;
        let next = transform(self.read_unlocked().await?)?;
        self.write_unlocked(&next).await
    }
}
