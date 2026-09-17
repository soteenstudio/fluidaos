use crate::{
    error::ApiError,
    services::{
        app_registry::AppRegistry,
        data_service::{validate_settings, DataService},
        device_control::DeviceControl,
        file_service::FileService,
        system_monitor::SystemMonitor,
        terminal_service::TerminalService,
    },
    SessionOwner,
};
use axum::{
    body::Body,
    extract::{Extension, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
#[derive(Clone)]
pub struct AppState {
    pub data: DataService,
    pub files: FileService,
    pub terminal: TerminalService,
    pub registry: AppRegistry,
    pub monitor: SystemMonitor,
    pub secret: Vec<u8>,
    pub devices: DeviceControl,
}
type Result<T> = std::result::Result<T, ApiError>;
fn ok(value: Value) -> Json<Value> {
    Json(json!({"success":true,"data":value}))
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::validation(format!("{key} must be a string")))
}
fn owner(extension: Option<&SessionOwner>, headers: &axum::http::HeaderMap) -> Result<String> {
    if let Some(owner) = extension {
        return Ok(owner.0.clone());
    }
    let id = headers
        .get("x-fluida-session")
        .and_then(|v| v.to_str().ok())
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 128
                && v.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "A valid x-fluida-session header is required",
            )
        })?;
    Ok(id.into())
}
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/fs", get(fs_list).delete(fs_delete))
        .route("/fs/search", get(fs_search))
        .route("/fs/meta", get(fs_meta))
        .route("/fs/preview", post(fs_preview))
        .route("/fs/read", post(fs_read))
        .route("/fs/write", post(fs_write))
        .route("/fs/mkdir", post(fs_mkdir))
        .route("/fs/rename", post(fs_rename))
        .route("/fs/copy", post(fs_copy))
        .route("/fs/import", post(fs_import))
        .route("/fs/download", get(fs_download))
        .route("/apps", get(apps))
        .route("/apps/all", get(apps_all))
        .route("/apps/install", post(app_install))
        .route("/apps/{id}", patch(app_enable).delete(app_uninstall))
        .route("/shell/session", get(session_get).put(session_put))
        .route("/settings", get(settings_get).put(settings_put))
        .route("/settings/reset", post(settings_reset))
        .route("/terminal/jobs", get(jobs).post(job_create))
        .route("/terminal/jobs/{id}", delete(job_delete))
        .route("/system/jobs/{id}", delete(job_delete))
        .route("/app-state/{app}", get(app_state_get).put(app_state_put))
        .route("/photos", get(photos))
        .route("/notes", get(notes).post(note_save))
        .route("/notes/{id}", delete(note_delete))
        .route("/notifications", get(notifications))
        .route(
            "/notifications/{id}",
            patch(notification_read).delete(notification_delete),
        )
        .route("/system", get(system))
        .route("/device-control", get(device_state).put(device_set))
        .route("/system/resources", get(system))
        .fallback(|| async {
            ApiError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Endpoint not found")
        })
}
async fn device_state(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(s.devices.state())?))
}
async fn device_set(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    let kind = string(&v, "control")?;
    let value = v
        .get("value")
        .and_then(Value::as_u64)
        .ok_or_else(|| ApiError::validation("value must be an integer"))?;
    if value > 100 {
        return Err(ApiError::validation(
            "Device values must be between 0 and 100",
        ));
    }
    Ok(ok(serde_json::to_value(s.devices.set(kind, value as u8)?)?))
}
#[derive(Deserialize)]
struct FsQuery {
    #[serde(default)]
    path: String,
    q: Option<String>,
}
async fn fs_list(State(s): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(s.files.list(&q.path).await?)?))
}
async fn fs_search(
    State(s): State<Arc<AppState>>,
    Query(q): Query<FsQuery>,
) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(
        s.files
            .search(
                &q.path,
                q.q.as_deref()
                    .ok_or_else(|| ApiError::validation("q must be a string"))?,
                200,
            )
            .await?,
    )?))
}
async fn fs_meta(State(s): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(s.files.metadata(&q.path).await?)?))
}
async fn fs_preview(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    let path = string(&v, "path")?;
    let max = v
        .get("maxBytes")
        .map(|v| {
            v.as_u64()
                .ok_or_else(|| ApiError::validation("maxBytes must be a number"))
        })
        .transpose()?
        .unwrap_or(131072) as usize;
    Ok(ok(serde_json::to_value(s.files.preview(path, max).await?)?))
}
async fn fs_read(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    Ok(ok(
        json!({"content":s.files.read(string(&v,"path")?).await?}),
    ))
}
async fn fs_write(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    s.files
        .write(string(&v, "path")?, string(&v, "content")?.as_bytes())
        .await?;
    Ok(ok(Value::Null))
}
async fn fs_mkdir(
    State(s): State<Arc<AppState>>,
    Json(v): Json<Value>,
) -> Result<impl IntoResponse> {
    s.files.mkdir(string(&v, "path")?).await?;
    Ok((StatusCode::CREATED, ok(Value::Null)))
}
async fn fs_rename(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    s.files
        .rename(string(&v, "from")?, string(&v, "to")?)
        .await?;
    Ok(ok(Value::Null))
}
async fn fs_copy(
    State(s): State<Arc<AppState>>,
    Json(v): Json<Value>,
) -> Result<impl IntoResponse> {
    s.files.copy(string(&v, "from")?, string(&v, "to")?).await?;
    Ok((StatusCode::CREATED, ok(Value::Null)))
}
async fn fs_import(
    State(s): State<Arc<AppState>>,
    Json(v): Json<Value>,
) -> Result<impl IntoResponse> {
    let entry = s
        .files
        .import_base64(
            string(&v, "directory")?,
            string(&v, "name")?,
            string(&v, "content")?,
        )
        .await?;
    Ok((StatusCode::CREATED, ok(serde_json::to_value(entry)?)))
}
async fn fs_delete(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    let recursive = v
        .get("recursive")
        .map(|v| {
            v.as_bool()
                .ok_or_else(|| ApiError::validation("recursive must be a boolean"))
        })
        .transpose()?
        .unwrap_or(false);
    s.files.delete(string(&v, "path")?, recursive).await?;
    Ok(ok(Value::Null))
}
async fn fs_download(State(s): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> Result<Response> {
    let path = s.files.resolve(&q.path)?;
    if !tokio::fs::metadata(&path).await?.is_file() {
        return Err(ApiError::invalid_path("Download path must be a file"));
    }
    let filename = s.files.normalize_filename(
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    )?;
    let body = Body::from(tokio::fs::read(path).await?);
    Ok((
        [
            (
                header::CONTENT_TYPE,
                mime_guess::from_path(&q.path)
                    .first_or_octet_stream()
                    .to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response())
}
async fn apps(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(
        s.registry
            .list()
            .await?
            .into_iter()
            .filter(|a| a.enabled)
            .collect::<Vec<_>>(),
    )?))
}
async fn apps_all(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(s.registry.list().await?)?))
}
async fn app_install(
    State(s): State<Arc<AppState>>,
    Json(v): Json<Value>,
) -> Result<impl IntoResponse> {
    Ok((
        StatusCode::CREATED,
        ok(serde_json::to_value(
            s.registry.install(string(&v, "packagePath")?).await?,
        )?),
    ))
}
async fn app_enable(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let enabled = v
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| ApiError::validation("enabled must be a boolean"))?;
    Ok(ok(serde_json::to_value(
        s.registry.enable(&id, enabled).await?,
    )?))
}
async fn app_uninstall(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    s.registry.uninstall(&id).await?;
    Ok(ok(Value::Null))
}
async fn session_get(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    Ok(ok(s.data.session.read().await?))
}
async fn session_put(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    Ok(ok(s.data.save_session(v).await?))
}
async fn settings_get(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    Ok(ok(s.data.read_settings().await?))
}
async fn settings_put(State(s): State<Arc<AppState>>, Json(v): Json<Value>) -> Result<Json<Value>> {
    let current = s.data.read_settings().await?;
    validate_settings(&v, &current)?;
    Ok(ok(s.data.save_settings(&v).await?))
}
async fn settings_reset(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    Ok(ok(s
        .data
        .settings
        .write(&crate::services::data_service::default_settings())
        .await?))
}
async fn job_create(
    State(s): State<Arc<AppState>>,
    extension: Option<Extension<SessionOwner>>,
    headers: axum::http::HeaderMap,
    Json(v): Json<Value>,
) -> Result<impl IntoResponse> {
    let owner = owner(extension.as_deref(), &headers)?;
    let result = s
        .terminal
        .execute(&owner, string(&v, "command")?, string(&v, "cwd")?)
        .await?;
    Ok((StatusCode::CREATED, ok(result)))
}
async fn jobs(
    State(s): State<Arc<AppState>>,
    extension: Option<Extension<SessionOwner>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(
        s.terminal
            .list(&owner(extension.as_deref(), &headers)?)
            .await,
    )?))
}
async fn job_delete(
    State(s): State<Arc<AppState>>,
    extension: Option<Extension<SessionOwner>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(
        s.terminal
            .terminate(&owner(extension.as_deref(), &headers)?, &id)
            .await?,
    )?))
}
async fn app_state_get(
    State(s): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> Result<Json<Value>> {
    if !["clock", "calendar", "archive"].contains(&app.as_str()) {
        return Err(ApiError::not_found());
    }
    Ok(ok(s
        .data
        .app_state
        .read()
        .await?
        .get(&app)
        .cloned()
        .unwrap_or(Value::Null)))
}
async fn app_state_put(
    State(s): State<Arc<AppState>>,
    Path(app): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    validate_app_state(&app, &v, &s.files)?;
    let key = app.clone();
    let saved = v.clone();
    s.data
        .app_state
        .update(move |mut state| {
            state[&key] = saved;
            Ok(state)
        })
        .await?;
    Ok(ok(v))
}
fn validate_app_state(app: &str, v: &Value, files: &FileService) -> Result<()> {
    match app {
        "clock" => {
            if !v.get("worldClocks").is_some_and(Value::is_array)
                || !v.get("alarms").is_some_and(Value::is_array)
                || v.get("timerSeconds").and_then(Value::as_f64).is_none()
                || v.get("stopwatchSeconds").and_then(Value::as_f64).is_none()
                || v.get("stopwatchRunning").and_then(Value::as_bool).is_none()
            {
                return Err(ApiError::validation("Invalid clock state"));
            }
        }
        "calendar" => {
            if !v.get("events").is_some_and(Value::is_array) {
                return Err(ApiError::validation("Invalid calendar state"));
            }
        }
        "archive" => {
            let rows = v
                .get("records")
                .and_then(Value::as_array)
                .ok_or_else(|| ApiError::validation("Invalid archive state"))?;
            for row in rows {
                for path in row
                    .get("paths")
                    .and_then(Value::as_array)
                    .ok_or_else(|| ApiError::validation("Invalid archive state"))?
                {
                    files.resolve(
                        path.as_str()
                            .ok_or_else(|| ApiError::validation("Invalid archive path"))?,
                    )?;
                }
            }
        }
        _ => return Err(ApiError::not_found()),
    }
    Ok(())
}
async fn photos(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    let rows = s.files.search("", ".", 500).await?;
    Ok(ok(serde_json::to_value(
        rows.into_iter()
            .filter(|r| r.entry.mime_type.starts_with("image/"))
            .map(|r| r.entry)
            .collect::<Vec<_>>(),
    )?))
}
async fn notes(
    State(s): State<Arc<AppState>>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    let needle = q.get("q").map(|v| v.to_lowercase()).unwrap_or_default();
    Ok(ok(serde_json::to_value(
        s.data
            .notes
            .read()
            .await?
            .into_iter()
            .filter(|n| {
                format!("{} {}", n.title, n.body)
                    .to_lowercase()
                    .contains(&needle)
            })
            .collect::<Vec<_>>(),
    )?))
}
async fn note_save(
    State(s): State<Arc<AppState>>,
    Json(v): Json<Value>,
) -> Result<impl IntoResponse> {
    Ok((
        StatusCode::CREATED,
        ok(serde_json::to_value(s.data.save_note(&v).await?)?),
    ))
}
async fn note_delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    s.data
        .notes
        .update(move |mut rows| {
            rows.retain(|n| n.id != id);
            Ok(rows)
        })
        .await?;
    Ok(ok(Value::Null))
}
async fn notifications(State(s): State<Arc<AppState>>) -> Result<Json<Value>> {
    Ok(ok(serde_json::to_value(
        s.data.notifications.read().await?,
    )?))
}
async fn notification_read(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let read = v
        .get("read")
        .and_then(Value::as_bool)
        .ok_or_else(|| ApiError::validation("read must be a boolean"))?;
    let mut found = None;
    s.data
        .notifications
        .update(|mut rows| {
            let row = rows
                .iter_mut()
                .find(|n| n.id == id)
                .ok_or_else(ApiError::not_found)?;
            row.read = read;
            found = Some(row.clone());
            Ok(rows)
        })
        .await?;
    Ok(ok(serde_json::to_value(found.unwrap())?))
}
async fn notification_delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    s.data
        .notifications
        .update(move |mut rows| {
            rows.retain(|n| n.id != id);
            Ok(rows)
        })
        .await?;
    Ok(ok(Value::Null))
}
async fn system(
    State(s): State<Arc<AppState>>,
    extension: Option<Extension<SessionOwner>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>> {
    Ok(ok(s
        .monitor
        .snapshot(&owner(extension.as_deref(), &headers)?)
        .await?))
}
