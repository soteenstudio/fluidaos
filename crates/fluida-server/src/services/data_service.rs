use crate::{error::ApiError, storage::json_repository::JsonRepository};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDefinition {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub folder: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub message: String,
    pub read: bool,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

pub fn default_settings() -> Value {
    json!({
        "theme":"dark","accent":"#7c6cff","wallpaper":"aurora","fontScale":1.0,"dockPosition":"bottom","animation":"full","reducedMotion":false,"chimeVolume":70,"filesView":"grid",
        "notifications":{"enabled":true,"sound":true,"showPreviews":true,"doNotDisturb":false},
        "accessibility":{"highContrast":false,"focusVisible":true,"textScale":1.0},
        "desktop":{"workspaceBehavior":"restore","shortcutLayout":"grid","dockAutoHide":false,"clockFormat":"24h"},
        "defaultApps":{"text":"editor","images":"photos"}
    })
}
fn default_session() -> Value {
    json!({"id":"local","activeWorkspaceId":"workspace-1","pinnedApplications":["files","notes","terminal","clock","calendar","settings"],"updatedAt":"1970-01-01T00:00:00.000Z","workspaces":[{"id":"workspace-1","name":"Main","windows":[],"zIndexSequence":10,"layout":"freeform"}]})
}
fn default_app_state() -> Value {
    json!({"clock":{"worldClocks":["UTC"],"alarms":[],"timerSeconds":300,"stopwatchSeconds":0,"stopwatchRunning":false},"calendar":{"events":[]},"archive":{"records":[]}})
}

fn merge(base: &mut Value, patch: &Value) {
    if let (Some(base), Some(patch)) = (base.as_object_mut(), patch.as_object()) {
        for (k, v) in patch {
            if v.is_object() && base.get(k).is_some_and(Value::is_object) {
                merge(base.get_mut(k).unwrap(), v);
            } else {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

#[derive(Clone)]
pub struct DataService {
    pub settings: JsonRepository<Value>,
    pub notes: JsonRepository<Vec<Note>>,
    pub notifications: JsonRepository<Vec<Notification>>,
    pub session: JsonRepository<Value>,
    pub app_state: JsonRepository<Value>,
    pub apps: Vec<AppDefinition>,
}
impl DataService {
    pub fn new(root: &Path) -> Self {
        let catalog = [
            ("files", "Files", "▰", "Browse and manage local files"),
            ("editor", "Editor", "✎", "Edit plain-text documents"),
            ("notes", "Notes", "◫", "Capture persistent notes"),
            ("terminal", "Terminal", "›_", "Run 20 contained commands"),
            ("monitor", "Monitor", "⌁", "Inspect this session"),
            ("calculator", "Calculator", "＋", "Calculate with history"),
            ("settings", "Settings", "⚙", "Personalize FluidaOS"),
            (
                "wallpapers",
                "Wallpapers",
                "◉",
                "Choose a desktop background",
            ),
            (
                "clock",
                "Clock",
                "◷",
                "World clocks, alarms, timers, and stopwatch",
            ),
            ("calendar", "Calendar", "▦", "Local calendar and agenda"),
            ("photos", "Photos", "▧", "Storage-backed image gallery"),
            (
                "archive",
                "Archive Manager",
                "▣",
                "Organize safe archive records",
            ),
            (
                "app-center",
                "App Center",
                "⬡",
                "Manage installed applications",
            ),
        ];
        Self {
            settings: JsonRepository::new(root.join("settings.json"), default_settings()),
            notes: JsonRepository::new(root.join("notes.json"), vec![]),
            notifications: JsonRepository::new(
                root.join("notifications.json"),
                vec![Notification {
                    id: "welcome".into(),
                    title: "Welcome to FluidaOS".into(),
                    message: "Your workspace is ready.".into(),
                    read: false,
                    created_at: Utc::now().to_rfc3339(),
                    source: None,
                }],
            ),
            session: JsonRepository::new(root.join("session.json"), default_session()),
            app_state: JsonRepository::new(root.join("app-state.json"), default_app_state()),
            apps: catalog
                .into_iter()
                .map(|(id, name, icon, description)| AppDefinition {
                    id: id.into(),
                    name: name.into(),
                    icon: icon.into(),
                    description: description.into(),
                })
                .collect(),
        }
    }
    pub async fn read_settings(&self) -> Result<Value, ApiError> {
        let mut value = default_settings();
        let mut saved = self.settings.read().await?;
        if let Some(object) = saved.as_object_mut() {
            if !object.contains_key("chimeVolume") {
                if let Some(volume) = object.get("volume").cloned() {
                    object.insert("chimeVolume".into(), volume);
                }
            }
            object.remove("volume");
            object.remove("brightness");
        }
        merge(&mut value, &saved);
        self.settings.write(&value).await
    }
    pub async fn save_settings(&self, patch: &Value) -> Result<Value, ApiError> {
        let mut value = self.read_settings().await?;
        merge(&mut value, patch);
        self.settings.write(&value).await
    }
    pub async fn save_session(&self, mut value: Value) -> Result<Value, ApiError> {
        let active = value.get("activeWorkspaceId").and_then(Value::as_str);
        let workspaces = value
            .get("workspaces")
            .and_then(Value::as_array)
            .ok_or_else(|| ApiError::validation("Invalid shell session"))?;
        if workspaces.is_empty()
            || !workspaces
                .iter()
                .any(|w| w.get("id").and_then(Value::as_str) == active)
        {
            return Err(ApiError::validation("Invalid shell session"));
        }
        value["updatedAt"] = json!(Utc::now().to_rfc3339());
        self.session.write(&value).await
    }
    pub async fn save_note(&self, input: &Value) -> Result<Note, ApiError> {
        let title = input
            .get("title")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::validation("title is required"))?;
        let body = input
            .get("body")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::validation("body is required"))?;
        let id = input
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let note = Note {
            id: id.clone(),
            title: title.into(),
            body: body.into(),
            folder: input
                .get("folder")
                .and_then(Value::as_str)
                .unwrap_or("Notes")
                .into(),
            updated_at: Utc::now().to_rfc3339(),
        };
        let saved = note.clone();
        self.notes
            .update(move |mut rows| {
                if let Some(i) = rows.iter().position(|n| n.id == id) {
                    rows[i] = saved;
                } else {
                    rows.insert(0, saved);
                }
                Ok(rows)
            })
            .await?;
        Ok(note)
    }
}

pub fn validate_settings(patch: &Value, current: &Value) -> Result<(), ApiError> {
    let object = patch
        .as_object()
        .ok_or_else(|| ApiError::validation("Invalid settings payload"))?;
    let allowed = [
        "theme",
        "accent",
        "wallpaper",
        "fontScale",
        "dockPosition",
        "animation",
        "reducedMotion",
        "chimeVolume",
        "filesView",
        "notifications",
        "accessibility",
        "desktop",
        "defaultApps",
    ];
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(ApiError::validation("Unknown setting"));
    }
    let nested = [
        (
            "notifications",
            vec!["enabled", "sound", "showPreviews", "doNotDisturb"],
        ),
        (
            "accessibility",
            vec!["highContrast", "focusVisible", "textScale"],
        ),
        (
            "desktop",
            vec![
                "workspaceBehavior",
                "shortcutLayout",
                "dockAutoHide",
                "clockFormat",
            ],
        ),
        ("defaultApps", vec!["text", "images"]),
    ];
    for (key, keys) in nested {
        if let Some(value) = object.get(key) {
            let value = value
                .as_object()
                .ok_or_else(|| ApiError::validation("Invalid nested preferences"))?;
            if value.keys().any(|k| !keys.contains(&k.as_str())) {
                return Err(ApiError::validation("Invalid nested preferences"));
            }
        }
    }
    let mut value = current.clone();
    merge(&mut value, patch);
    let string = |k| value.get(k).and_then(Value::as_str);
    let number = |k| value.get(k).and_then(Value::as_f64);
    if !["light", "dark", "system"].contains(&string("theme").unwrap_or(""))
        || !["bottom", "left", "right"].contains(&string("dockPosition").unwrap_or(""))
        || !["full", "reduced", "none"].contains(&string("animation").unwrap_or(""))
        || !["grid", "list"].contains(&string("filesView").unwrap_or(""))
        || !regex::Regex::new("^#[0-9a-fA-F]{6}$")
            .unwrap()
            .is_match(string("accent").unwrap_or(""))
        || !(0.8..=1.4).contains(&number("fontScale").unwrap_or(-1.0))
        || !(0.0..=100.0).contains(&number("chimeVolume").unwrap_or(-1.0))
        || value
            .get("reducedMotion")
            .and_then(Value::as_bool)
            .is_none()
    {
        return Err(ApiError::validation("Invalid setting value"));
    }
    if string("wallpaper").map_or(true, |v| v.len() > 64)
        || value
            .get("notifications")
            .and_then(Value::as_object)
            .map_or(true, |v| v.values().any(|v| !v.is_boolean()))
    {
        return Err(ApiError::validation("Invalid setting value"));
    }
    let accessibility = value
        .get("accessibility")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::validation("Invalid accessibility preferences"))?;
    if accessibility
        .get("highContrast")
        .and_then(Value::as_bool)
        .is_none()
        || accessibility
            .get("focusVisible")
            .and_then(Value::as_bool)
            .is_none()
        || !(0.8..=2.0).contains(
            &accessibility
                .get("textScale")
                .and_then(Value::as_f64)
                .unwrap_or(-1.0),
        )
    {
        return Err(ApiError::validation("Invalid accessibility preferences"));
    }
    let desktop = value
        .get("desktop")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::validation("Invalid desktop preferences"))?;
    if !["restore", "fresh"].contains(
        &desktop
            .get("workspaceBehavior")
            .and_then(Value::as_str)
            .unwrap_or(""),
    ) || !["grid", "list"].contains(
        &desktop
            .get("shortcutLayout")
            .and_then(Value::as_str)
            .unwrap_or(""),
    ) || desktop
        .get("dockAutoHide")
        .and_then(Value::as_bool)
        .is_none()
        || !["12h", "24h"].contains(
            &desktop
                .get("clockFormat")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )
    {
        return Err(ApiError::validation("Invalid desktop preferences"));
    }
    if value
        .get("defaultApps")
        .and_then(Value::as_object)
        .map_or(true, |v| {
            v.values()
                .any(|v| v.as_str().map_or(true, |v| v.len() > 64))
        })
    {
        return Err(ApiError::validation("Invalid default applications"));
    }
    Ok(())
}
