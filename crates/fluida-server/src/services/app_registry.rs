use crate::{
    error::ApiError, services::data_service::AppDefinition,
    storage::json_repository::JsonRepository,
};
use axum::http::StatusCode;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Source {
    #[serde(rename = "builtin")]
    Builtin,
    #[serde(rename = "local")]
    Local { package_path: String },
}
#[derive(Clone, Serialize, Deserialize)]
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
    repo: JsonRepository<Vec<InstalledApp>>,
    builtins: Vec<InstalledApp>,
}
impl AppRegistry {
    pub fn new(root: PathBuf, data: &Path, apps: &[AppDefinition]) -> Self {
        let builtins = apps
            .iter()
            .map(|a| InstalledApp {
                manifest: Manifest {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    icon: a.icon.clone(),
                    description: a.description.clone(),
                    version: "1.0.0".into(),
                    entry_point: format!("builtin:{}", a.id),
                    capabilities: vec![],
                },
                source: Source::Builtin,
                enabled: true,
                installed_at: "1970-01-01T00:00:00.000Z".into(),
            })
            .collect();
        Self {
            root,
            repo: JsonRepository::new(data.join("apps.json"), vec![]),
            builtins,
        }
    }
    fn package(&self, value: &str) -> Result<PathBuf, ApiError> {
        if value.contains('\0') || Path::new(value).is_absolute() {
            return Err(ApiError::validation("Invalid package path"));
        }
        let mut path = self.root.clone();
        for component in Path::new(value).components() {
            match component {
                std::path::Component::Normal(value) => path.push(value),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if path == self.root {
                        return Err(ApiError::validation("Package escapes registry root"));
                    }
                    path.pop();
                }
                _ => return Err(ApiError::validation("Invalid package path")),
            }
        }
        Ok(path)
    }
    fn validate(value: serde_json::Value) -> Result<Manifest, ApiError> {
        let manifest: Manifest = serde_json::from_value(value).map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_MANIFEST",
                "Invalid manifest",
            )
        })?;
        if !regex::Regex::new("^[a-z0-9][a-z0-9-]{1,63}$")
            .unwrap()
            .is_match(&manifest.id)
            || manifest.name.is_empty()
            || manifest.capabilities.iter().any(|c| {
                ![
                    "files:read",
                    "files:write",
                    "terminal",
                    "notifications",
                    "system:read",
                ]
                .contains(&c.as_str())
            })
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_MANIFEST",
                "Invalid manifest",
            ));
        }
        Ok(manifest)
    }
    pub async fn list(&self) -> Result<Vec<InstalledApp>, ApiError> {
        let stored = self.repo.read().await?;
        let mut rows = self
            .builtins
            .iter()
            .map(|b| {
                stored
                    .iter()
                    .find(|s| matches!(s.source, Source::Builtin) && s.manifest.id == b.manifest.id)
                    .cloned()
                    .unwrap_or_else(|| b.clone())
            })
            .collect::<Vec<_>>();
        rows.extend(
            stored
                .into_iter()
                .filter(|a| matches!(a.source, Source::Local { .. })),
        );
        Ok(rows)
    }
    pub async fn install(&self, package: &str) -> Result<InstalledApp, ApiError> {
        let dir = self.package(package)?;
        let manifest = Self::validate(serde_json::from_slice(
            &tokio::fs::read(dir.join("manifest.json")).await?,
        )?)?;
        if Path::new(&manifest.entry_point).is_absolute()
            || Path::new(&manifest.entry_point)
                .components()
                .any(|v| matches!(v, std::path::Component::ParentDir))
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_MANIFEST",
                "Invalid entry point",
            ));
        }
        let entry = dir.join(&manifest.entry_point);
        if !tokio::fs::metadata(&entry).await?.is_file() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_MANIFEST",
                "Invalid entry point",
            ));
        }
        if self
            .list()
            .await?
            .iter()
            .any(|a| a.manifest.id == manifest.id)
        {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "DUPLICATE_APP",
                "Application ID already installed",
            ));
        }
        let app = InstalledApp {
            manifest,
            source: Source::Local {
                package_path: package.into(),
            },
            enabled: true,
            installed_at: Utc::now().to_rfc3339(),
        };
        let saved = app.clone();
        self.repo
            .update(move |mut rows| {
                rows.push(saved);
                Ok(rows)
            })
            .await?;
        Ok(app)
    }
    pub async fn enable(&self, id: &str, enabled: bool) -> Result<InstalledApp, ApiError> {
        let builtin = self.builtins.iter().find(|a| a.manifest.id == id).cloned();
        let id = id.to_owned();
        let mut result = None;
        self.repo
            .update(|mut rows| {
                if let Some(app) = rows.iter_mut().find(|a| a.manifest.id == id) {
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
        if self.builtins.iter().any(|a| a.manifest.id == id) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "BUILTIN_APP",
                "Built-in applications cannot be uninstalled",
            ));
        }
        let id = id.to_owned();
        self.repo
            .update(move |mut rows| {
                let before = rows.len();
                rows.retain(|a| a.manifest.id != id);
                if rows.len() == before {
                    return Err(ApiError::not_found());
                }
                Ok(rows)
            })
            .await?;
        Ok(())
    }
}
