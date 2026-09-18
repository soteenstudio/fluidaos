use crate::{
    error::ApiError, services::data_service::AppDefinition,
    storage::json_repository::JsonRepository,
};
use axum::http::StatusCode;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;
use zip::ZipArchive;

pub const MAX_ARCHIVE_SIZE: u64 = 8 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 256;
pub const MAX_EXTRACTED_SIZE: u64 = 32 * 1024 * 1024;
const REQUIRED_FILES: [&str; 3] = ["manifest.json", "index.html", "app.js"];
const CAPABILITIES: [&str; 5] = [
    "files:read",
    "files:write",
    "terminal",
    "notifications",
    "system:read",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub version: String,
    pub entry_point: String,
    pub capabilities: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Source {
    #[serde(rename = "builtin")]
    Builtin,
    #[serde(rename = "local")]
    Local { package_path: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub manifest: Manifest,
    pub source: Source,
    pub enabled: bool,
    pub installed_at: String,
}
#[derive(Clone)]
pub struct AppRegistry {
    root: PathBuf,
    installed: PathBuf,
    repo: JsonRepository<Vec<InstalledApp>>,
    builtins: Vec<InstalledApp>,
}

fn invalid_archive(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "INVALID_ARCHIVE", message)
}
fn invalid_manifest(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "INVALID_MANIFEST", message)
}
fn relative_path(value: &str) -> Result<PathBuf, ApiError> {
    if value.is_empty()
        || value.contains('\0')
        || value.contains('\\')
        || Path::new(value).is_absolute()
    {
        return Err(invalid_archive("Invalid archive path"));
    }
    let mut clean = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(value) => clean.push(value),
            _ => return Err(invalid_archive("Archive path traversal is not allowed")),
        }
    }
    Ok(clean)
}
fn record_name(names: &mut HashSet<String>, name: String) -> Result<(), ApiError> {
    if !names.insert(name) {
        return Err(invalid_archive("Duplicate archive entry"));
    }
    Ok(())
}
fn validate_manifest(value: &[u8]) -> Result<Manifest, ApiError> {
    let manifest: Manifest =
        serde_json::from_slice(value).map_err(|_| invalid_manifest("Invalid manifest.json"))?;
    let id_ok = regex::Regex::new("^[a-z0-9][a-z0-9-]{1,63}$")
        .unwrap()
        .is_match(&manifest.id);
    if !id_ok
        || manifest.name.trim().is_empty()
        || manifest.name.len() > 128
        || manifest.icon.len() > 32
        || manifest.description.len() > 1024
        || manifest.version.trim().is_empty()
        || manifest
            .capabilities
            .iter()
            .any(|value| !CAPABILITIES.contains(&value.as_str()))
    {
        return Err(invalid_manifest("Manifest fields are invalid"));
    }
    if manifest.entry_point != "app.js" {
        return Err(invalid_manifest(
            "entryPoint must identify the root-level app.js file",
        ));
    }
    Ok(manifest)
}

impl AppRegistry {
    pub fn new(root: PathBuf, data: &Path, apps: &[AppDefinition]) -> Self {
        let installed = root.join("installed");
        let builtins = apps
            .iter()
            .map(|app| InstalledApp {
                manifest: Manifest {
                    id: app.id.clone(),
                    name: app.name.clone(),
                    icon: app.icon.clone(),
                    description: app.description.clone(),
                    version: "1.0.0".into(),
                    entry_point: "app.js".into(),
                    capabilities: match app.id.as_str() {
                        "files" | "editor" => vec!["files:read".into(), "files:write".into()],
                        "notes" => vec!["files:read".into(), "files:write".into()],
                        "terminal" => vec!["terminal".into()],
                        "monitor" | "settings" => vec!["system:read".into()],
                        "photos" | "archive" => vec!["files:read".into()],
                        _ => vec![],
                    },
                },
                source: Source::Builtin,
                enabled: true,
                installed_at: "1970-01-01T00:00:00.000Z".into(),
            })
            .collect();
        Self {
            root,
            installed,
            repo: JsonRepository::new(data.join("apps.json"), vec![]),
            builtins,
        }
    }
    fn package_path(&self, value: &str) -> Result<PathBuf, ApiError> {
        if !value.to_ascii_lowercase().ends_with(".zip") {
            return Err(invalid_archive(
                "Only .zip application packages are accepted",
            ));
        }
        Ok(self.root.join(relative_path(value)?))
    }
    fn extract_archive(&self, archive_path: &Path) -> Result<(Manifest, PathBuf), ApiError> {
        let metadata = fs::metadata(archive_path).map_err(ApiError::from)?;
        if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_SIZE {
            return Err(invalid_archive("Archive exceeds the size limit"));
        }
        let mut archive = ZipArchive::new(fs::File::open(archive_path).map_err(ApiError::from)?)
            .map_err(|_| invalid_archive("Invalid ZIP archive"))?;
        if archive.len() == 0 || archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err(invalid_archive("Archive entry limit exceeded"));
        }
        let mut names = HashSet::new();
        let mut total = 0u64;
        let mut manifest_bytes = None;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|_| invalid_archive("Invalid ZIP entry"))?;
            let name = entry.name().trim_end_matches('/');
            if name.is_empty() {
                continue;
            }
            let clean = relative_path(name)?;
            let normalized = clean.to_string_lossy().replace('\\', "/");
            record_name(&mut names, normalized.clone())?;
            let kind = entry.unix_mode().unwrap_or(0) & 0o170000;
            if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                return Err(invalid_archive("Links and special files are not allowed"));
            }
            total = total
                .checked_add(entry.size())
                .ok_or_else(|| invalid_archive("Extracted size limit exceeded"))?;
            if total > MAX_EXTRACTED_SIZE {
                return Err(invalid_archive("Extracted size limit exceeded"));
            }
            if normalized == "manifest.json" {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).map_err(ApiError::from)?;
                manifest_bytes = Some(bytes)
            }
        }
        for required in REQUIRED_FILES {
            if !names.contains(required) {
                return Err(invalid_archive(format!(
                    "Missing required file: {required}"
                )));
            }
        }
        let manifest = validate_manifest(
            &manifest_bytes.ok_or_else(|| invalid_archive("Missing manifest.json"))?,
        )?;
        let temporary = self.installed.join(format!(".tmp-{}", Uuid::new_v4()));
        fs::create_dir_all(&temporary).map_err(ApiError::from)?;
        let extraction = (|| -> Result<(), ApiError> {
            for index in 0..archive.len() {
                let mut entry = archive
                    .by_index(index)
                    .map_err(|_| invalid_archive("Invalid ZIP entry"))?;
                let name = entry.name().trim_end_matches('/');
                if name.is_empty() {
                    continue;
                }
                let target = temporary.join(relative_path(name)?);
                if entry.is_dir() {
                    fs::create_dir_all(&target).map_err(ApiError::from)?
                } else {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).map_err(ApiError::from)?
                    }
                    let mut output = fs::File::create(target).map_err(ApiError::from)?;
                    std::io::copy(&mut entry, &mut output).map_err(ApiError::from)?;
                    output.flush().map_err(ApiError::from)?
                }
            }
            Ok(())
        })();
        if let Err(error) = extraction {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
        Ok((manifest, temporary))
    }
    pub async fn list(&self) -> Result<Vec<InstalledApp>, ApiError> {
        let stored = self.repo.read().await?;
        let mut rows = self
            .builtins
            .iter()
            .map(|builtin| {
                stored
                    .iter()
                    .find(|saved| {
                        matches!(saved.source, Source::Builtin)
                            && saved.manifest.id == builtin.manifest.id
                    })
                    .cloned()
                    .unwrap_or_else(|| builtin.clone())
            })
            .collect::<Vec<_>>();
        rows.extend(
            stored
                .into_iter()
                .filter(|app| matches!(app.source, Source::Local { .. })),
        );
        Ok(rows)
    }
    pub async fn install(&self, package: &str) -> Result<InstalledApp, ApiError> {
        let archive = self.package_path(package)?;
        fs::create_dir_all(&self.installed).map_err(ApiError::from)?;
        let (manifest, temporary) = self.extract_archive(&archive)?;
        if self
            .list()
            .await?
            .iter()
            .any(|app| app.manifest.id == manifest.id)
        {
            let _ = fs::remove_dir_all(temporary);
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "DUPLICATE_APP",
                "Application ID already installed",
            ));
        }
        let destination = self.installed.join(&manifest.id);
        if destination.exists() {
            let _ = fs::remove_dir_all(temporary);
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "DUPLICATE_APP",
                "Application ID already installed",
            ));
        }
        fs::rename(&temporary, &destination).map_err(|error| {
            let _ = fs::remove_dir_all(&temporary);
            ApiError::from(error)
        })?;
        let app = InstalledApp {
            manifest,
            source: Source::Local {
                package_path: package.into(),
            },
            enabled: true,
            installed_at: Utc::now().to_rfc3339(),
        };
        let saved = app.clone();
        if let Err(error) = self
            .repo
            .update(move |mut rows| {
                rows.push(saved);
                Ok(rows)
            })
            .await
        {
            let _ = fs::remove_dir_all(destination);
            return Err(error);
        }
        Ok(app)
    }
    pub async fn asset(&self, id: &str, path: &str) -> Result<PathBuf, ApiError> {
        let app = self
            .list()
            .await?
            .into_iter()
            .find(|app| app.manifest.id == id && app.enabled)
            .ok_or_else(ApiError::not_found)?;
        let relative = relative_path(path).map_err(|_| ApiError::not_found())?;
        let base = match app.source {
            Source::Builtin => self.root.join("builtin").join(id),
            Source::Local { .. } => self.installed.join(id),
        };
        let base = fs::canonicalize(base).map_err(|_| ApiError::not_found())?;
        let target = fs::canonicalize(base.join(relative)).map_err(|_| ApiError::not_found())?;
        if !target.starts_with(&base) || !target.is_file() {
            return Err(ApiError::not_found());
        }
        Ok(target)
    }
    pub async fn enable(&self, id: &str, enabled: bool) -> Result<InstalledApp, ApiError> {
        let builtin = self
            .builtins
            .iter()
            .find(|app| app.manifest.id == id)
            .cloned();
        let id = id.to_owned();
        let mut result = None;
        self.repo
            .update(|mut rows| {
                if let Some(app) = rows.iter_mut().find(|app| app.manifest.id == id) {
                    app.enabled = enabled;
                    result = Some(app.clone())
                } else if let Some(mut app) = builtin {
                    app.enabled = enabled;
                    result = Some(app.clone());
                    rows.push(app)
                } else {
                    return Err(ApiError::not_found());
                }
                Ok(rows)
            })
            .await?;
        Ok(result.unwrap())
    }
    pub async fn uninstall(&self, id: &str) -> Result<(), ApiError> {
        if self.builtins.iter().any(|app| app.manifest.id == id) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "BUILTIN_APP",
                "Built-in applications cannot be uninstalled",
            ));
        }
        let owned = id.to_owned();
        self.repo
            .update(move |mut rows| {
                let before = rows.len();
                rows.retain(|app| app.manifest.id != owned);
                if rows.len() == before {
                    return Err(ApiError::not_found());
                }
                Ok(rows)
            })
            .await?;
        let directory = self.installed.join(id);
        if directory.exists() {
            fs::remove_dir_all(directory).map_err(ApiError::from)?
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::{write::SimpleFileOptions, ZipWriter};

    fn manifest(id: &str, entry: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"id":id,"name":"Test","icon":"T","description":"test","version":"1.0.0","entryPoint":entry,"capabilities":[]})).unwrap()
    }
    fn package(root: &Path, name: &str, entries: Vec<(&str, Vec<u8>)>) {
        fs::create_dir_all(root).unwrap();
        let file = fs::File::create(root.join(name)).unwrap();
        let mut zip = ZipWriter::new(file);
        for (entry, data) in entries {
            zip.start_file(entry, SimpleFileOptions::default()).unwrap();
            zip.write_all(&data).unwrap()
        }
        zip.finish().unwrap();
    }
    fn registry(root: &TempDir) -> AppRegistry {
        AppRegistry::new(root.path().join("packages"), &root.path().join("data"), &[])
    }
    fn valid(id: &str) -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("manifest.json", manifest(id, "app.js")),
            ("index.html", b"<script src=app.js></script>".to_vec()),
            ("app.js", b"true".to_vec()),
        ]
    }

    #[tokio::test]
    async fn installs_serves_and_uninstalls_a_valid_archive() {
        let root = TempDir::new().unwrap();
        let registry = registry(&root);
        package(
            &root.path().join("packages"),
            "valid.zip",
            valid("valid-app"),
        );
        let app = registry.install("valid.zip").await.unwrap();
        assert_eq!(app.manifest.entry_point, "app.js");
        assert_eq!(
            fs::read_to_string(registry.asset("valid-app", "app.js").await.unwrap()).unwrap(),
            "true"
        );
        registry.uninstall("valid-app").await.unwrap();
        assert!(!root.path().join("packages/installed/valid-app").exists());
    }
    #[tokio::test]
    async fn rejects_missing_files_and_invalid_entry_points() {
        let root = TempDir::new().unwrap();
        let registry = registry(&root);
        package(
            &root.path().join("packages"),
            "missing.zip",
            vec![
                ("manifest.json", manifest("missing-app", "app.js")),
                ("index.html", vec![]),
            ],
        );
        assert!(registry.install("missing.zip").await.is_err());
        package(
            &root.path().join("packages"),
            "entry.zip",
            vec![
                ("manifest.json", manifest("entry-app", "../app.js")),
                ("index.html", vec![]),
                ("app.js", vec![]),
            ],
        );
        assert!(registry.install("entry.zip").await.is_err());
    }
    #[tokio::test]
    async fn rejects_duplicates_traversal_and_duplicate_ids() {
        let root = TempDir::new().unwrap();
        let registry = registry(&root);
        let mut names = HashSet::new();
        record_name(&mut names, "app.js".into()).unwrap();
        assert!(record_name(&mut names, "app.js".into()).is_err());
        let mut traversal = valid("traversal-app");
        traversal.push(("../escape", vec![]));
        package(&root.path().join("packages"), "traversal.zip", traversal);
        assert!(registry.install("traversal.zip").await.is_err());
        package(
            &root.path().join("packages"),
            "first.zip",
            valid("same-app"),
        );
        package(
            &root.path().join("packages"),
            "second.zip",
            valid("same-app"),
        );
        registry.install("first.zip").await.unwrap();
        assert!(registry.install("second.zip").await.is_err());
    }
    #[tokio::test]
    async fn enforces_entry_limit_and_asset_containment() {
        let root = TempDir::new().unwrap();
        let registry = registry(&root);
        let mut entries = valid("large-app");
        for index in 0..MAX_ARCHIVE_ENTRIES {
            entries.push((Box::leak(format!("files/{index}").into_boxed_str()), vec![]))
        }
        package(&root.path().join("packages"), "large.zip", entries);
        assert!(registry.install("large.zip").await.is_err());
        package(
            &root.path().join("packages"),
            "contained.zip",
            valid("contained-app"),
        );
        registry.install("contained.zip").await.unwrap();
        assert!(registry
            .asset("contained-app", "../manifest.json")
            .await
            .is_err());
    }
}
