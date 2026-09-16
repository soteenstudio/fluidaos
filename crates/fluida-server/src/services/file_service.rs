use crate::error::ApiError;
use axum::http::StatusCode;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{
    path::{Component, Path, PathBuf},
    time::SystemTime,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use walkdir::WalkDir;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
    pub modified_at: String,
    pub mime_type: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchResult {
    #[serde(flatten)]
    pub entry: FileEntry,
    pub parent: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub size: u64,
    pub mime_type: String,
}

#[derive(Clone)]
pub struct FileService {
    pub root: PathBuf,
}
impl FileService {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn resolve(&self, user: &str) -> Result<PathBuf, ApiError> {
        if user.contains('\0') || Path::new(user).is_absolute() {
            return Err(ApiError::invalid_path("Invalid storage path"));
        }
        let mut clean = PathBuf::new();
        for part in Path::new(&user.replace('\\', "/")).components() {
            match part {
                Component::Normal(v) => clean.push(v),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !clean.pop() {
                        return Err(ApiError::invalid_path("Path escapes OS storage"));
                    }
                }
                _ => return Err(ApiError::invalid_path("Invalid storage path")),
            }
        }
        Ok(self.root.join(clean))
    }
    pub fn relative(&self, path: &Path) -> Result<String, ApiError> {
        Ok(path
            .strip_prefix(&self.root)
            .map_err(|_| ApiError::invalid_path("Path escapes OS storage"))?
            .to_string_lossy()
            .replace('\\', "/"))
    }
    async fn entry(&self, path: &Path) -> Result<FileEntry, ApiError> {
        let meta = tokio::fs::metadata(path).await?;
        let directory = meta.is_dir();
        let modified: DateTime<Utc> = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH).into();
        Ok(FileEntry {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            path: self.relative(path)?,
            is_directory: directory,
            size: if directory { 0 } else { meta.len() },
            modified_at: modified.to_rfc3339(),
            mime_type: if directory {
                "inode/directory".into()
            } else {
                mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string()
            },
        })
    }
    pub async fn list(&self, path: &str) -> Result<Vec<FileEntry>, ApiError> {
        let target = self.resolve(path)?;
        tokio::fs::create_dir_all(&target).await?;
        let mut read = tokio::fs::read_dir(target).await?;
        let mut rows = vec![];
        while let Some(item) = read.next_entry().await? {
            rows.push(self.entry(&item.path()).await?);
        }
        rows.sort_by(|a, b| {
            b.is_directory
                .cmp(&a.is_directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(rows)
    }
    pub async fn metadata(&self, path: &str) -> Result<FileEntry, ApiError> {
        self.entry(&self.resolve(path)?).await
    }
    pub async fn read(&self, path: &str) -> Result<String, ApiError> {
        Ok(tokio::fs::read_to_string(self.resolve(path)?).await?)
    }
    pub async fn write(&self, path: &str, content: &[u8]) -> Result<(), ApiError> {
        let target = self.resolve(path)?;
        if target == self.root {
            return Err(ApiError::invalid_path("Storage root cannot be overwritten"));
        }
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(target, content).await?;
        Ok(())
    }
    pub async fn append(&self, path: &str, content: &str) -> Result<(), ApiError> {
        let target = self.resolve(path)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(target)
            .await?
            .write_all(content.as_bytes())
            .await?;
        Ok(())
    }
    pub async fn touch(&self, path: &str) -> Result<(), ApiError> {
        let target = self.resolve(path)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(target)
            .await?;
        Ok(())
    }
    pub async fn mkdir(&self, path: &str) -> Result<(), ApiError> {
        let target = self.resolve(path)?;
        if target == self.root {
            return Err(ApiError::invalid_path("Directory path is required"));
        }
        tokio::fs::create_dir(target).await?;
        Ok(())
    }
    pub async fn rename(&self, from: &str, to: &str) -> Result<(), ApiError> {
        let source = self.resolve(from)?;
        let target = self.resolve(to)?;
        if source == self.root || target == self.root {
            return Err(ApiError::invalid_path("Storage root cannot be moved"));
        }
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::rename(source, target).await?;
        Ok(())
    }
    pub async fn copy(&self, from: &str, to: &str) -> Result<(), ApiError> {
        let source = self.resolve(from)?;
        let target = self.resolve(to)?;
        if source == self.root || target == self.root || target.starts_with(&source) {
            return Err(ApiError::invalid_path("Invalid copy destination"));
        }
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if source.is_dir() {
            let s = source.clone();
            let t = target.clone();
            tokio::task::spawn_blocking(move || {
                for item in WalkDir::new(&s) {
                    let item = item.map_err(std::io::Error::other)?;
                    let dest = t.join(item.path().strip_prefix(&s).unwrap());
                    if item.file_type().is_dir() {
                        std::fs::create_dir_all(dest)?
                    } else {
                        if let Some(p) = dest.parent() {
                            std::fs::create_dir_all(p)?;
                        }
                        std::fs::copy(item.path(), dest)?;
                    }
                }
                Ok::<_, std::io::Error>(())
            })
            .await
            .map_err(|e| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    e.to_string(),
                )
            })??;
        } else {
            tokio::fs::copy(source, target).await?;
        }
        Ok(())
    }
    pub async fn delete(&self, path: &str, recursive: bool) -> Result<(), ApiError> {
        let target = self.resolve(path)?;
        if target == self.root {
            return Err(ApiError::invalid_path("Storage root cannot be deleted"));
        }
        if target.is_dir() {
            if recursive {
                tokio::fs::remove_dir_all(target).await?
            } else {
                let mut read = tokio::fs::read_dir(&target).await?;
                if read.next_entry().await?.is_some() {
                    return Err(ApiError::new(
                        StatusCode::CONFLICT,
                        "DIRECTORY_NOT_EMPTY",
                        "Directory is not empty",
                    ));
                }
                tokio::fs::remove_dir(target).await?
            }
        } else {
            tokio::fs::remove_file(target).await?
        }
        Ok(())
    }
    pub async fn preview(&self, path: &str, max: usize) -> Result<FilePreview, ApiError> {
        if !(1..=262144).contains(&max) {
            return Err(ApiError::invalid_path("Invalid preview limit"));
        }
        let target = self.resolve(path)?;
        let meta = tokio::fs::metadata(&target).await?;
        if !meta.is_file() {
            return Err(ApiError::invalid_path("Preview path must be a file"));
        }
        let mut bytes = vec![0; std::cmp::min(meta.len() as usize, max)];
        let mut file = tokio::fs::File::open(&target).await?;
        file.read_exact(&mut bytes).await?;
        Ok(FilePreview {
            path: self.relative(&target)?,
            content: String::from_utf8_lossy(&bytes).into(),
            truncated: meta.len() > bytes.len() as u64,
            size: meta.len(),
            mime_type: mime_guess::from_path(target)
                .first_or_octet_stream()
                .to_string(),
        })
    }
    pub async fn search(
        &self,
        directory: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchResult>, ApiError> {
        if query.trim().is_empty() || query.len() > 128 || !(1..=500).contains(&limit) {
            return Err(ApiError::invalid_path("Invalid search"));
        }
        let root = self.resolve(directory)?;
        let needle = query.to_lowercase();
        let paths = tokio::task::spawn_blocking(move || {
            let mut paths = vec![];
            for item in WalkDir::new(root)
                .min_depth(1)
                .into_iter()
                .filter_map(Result::ok)
            {
                if item
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&needle)
                {
                    paths.push(item.path().to_owned());
                    if paths.len() >= limit {
                        break;
                    }
                }
            }
            paths
        })
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                e.to_string(),
            )
        })?;
        let mut rows = vec![];
        for path in paths {
            let entry = self.entry(&path).await?;
            let parent = Path::new(&entry.path)
                .parent()
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .replace('\\', "/");
            rows.push(FileSearchResult { entry, parent });
        }
        Ok(rows)
    }
    pub async fn import_base64(
        &self,
        directory: &str,
        name: &str,
        value: &str,
    ) -> Result<FileEntry, ApiError> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value)
            .map_err(|_| ApiError::invalid_path("Invalid base64 content"))?;
        if bytes.len() > 5 * 1024 * 1024 {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "UPLOAD_TOO_LARGE",
                "Upload exceeds size limit",
            ));
        }
        let name = self.normalize_filename(name)?;
        let path = if directory.is_empty() {
            name
        } else {
            format!("{directory}/{name}")
        };
        self.write(&path, &bytes).await?;
        self.metadata(&path).await
    }
    pub fn normalize_filename(&self, name: &str) -> Result<String, ApiError> {
        let name = Path::new(name)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let clean = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || "._ -".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim()
            .to_owned();
        if clean.is_empty() || clean == "." || clean == ".." {
            Err(ApiError::invalid_path("Invalid filename"))
        } else {
            Ok(clean)
        }
    }
    pub async fn usage(&self) -> Result<(u64, u64), ApiError> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            let mut bytes = 0;
            let mut files = 0;
            for item in WalkDir::new(root)
                .into_iter()
                .filter_entry(|e| e.file_name() != ".fluida")
                .filter_map(Result::ok)
            {
                if item.file_type().is_file() {
                    bytes += item.metadata().map(|m| m.len()).unwrap_or(0);
                    files += 1;
                }
            }
            (bytes, files)
        })
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                e.to_string(),
            )
        })
    }
}
