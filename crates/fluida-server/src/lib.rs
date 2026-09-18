pub mod config;
pub mod error;
pub mod http;
pub mod services;
pub mod storage;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderValue, Method},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use base64::Engine;
use config::Config;
use hmac::{Hmac, Mac};
use http::AppState;
use sha2::Sha256;
use std::sync::Arc;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

#[derive(Clone)]
pub struct SessionOwner(pub String);

pub async fn app(config: Config) -> Router {
    let data = services::data_service::DataService::new(&config.storage_root.join(".fluida"));
    let files = services::file_service::FileService::new(config.storage_root.clone());
    let terminal = services::terminal_service::TerminalService::new(files.clone());
    let registry = services::app_registry::AppRegistry::new(
        config.registry_root.clone(),
        &config.storage_root.join(".fluida"),
        &data.apps,
    );
    let monitor = services::system_monitor::SystemMonitor::new(
        files.clone(),
        terminal.clone(),
        registry.clone(),
    );
    let devices = services::device_control::native(config.device_control_enabled);
    let state = Arc::new(AppState {
        data,
        files,
        terminal,
        registry,
        monitor,
        secret: config.session_secret.as_bytes().to_vec(),
        devices,
    });
    let mut cors = CorsLayer::new()
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            axum::http::HeaderName::from_static("x-fluida-session"),
        ]);
    for origin in &config.trusted_origins {
        if let Ok(value) = origin.parse::<HeaderValue>() {
            cors = cors.allow_origin(value)
        }
    }
    let public = config.project_root.join("src/public");
    let api = http::router()
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), session));
    Router::new()
        .nest("/api", api)
        .nest_service("/assets", ServeDir::new(config.project_root.join("dist")))
        .nest_service(
            "/styles",
            ServeDir::new(config.project_root.join("src/client/styles")),
        )
        .fallback_service(
            ServeDir::new(&public).fallback(ServeFile::new(public.join("index.html"))),
        )
        .layer(cors)
        .with_state(state)
}
async fn session(
    State(state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .map(str::trim)
                .find_map(|v| v.strip_prefix("fluida-session="))
        });
    let owner = cookie
        .and_then(|v| verify(v, &state.secret))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let fresh = cookie.and_then(|v| verify(v, &state.secret)).is_none();
    let secure = request.uri().scheme_str() == Some("https")
        || request
            .headers()
            .get("x-forwarded-proto")
            .is_some_and(|v| v == "https");
    request.extensions_mut().insert(SessionOwner(owner.clone()));
    let mut response = next.run(request).await;
    if matches!(
        response.status(),
        axum::http::StatusCode::BAD_REQUEST
            | axum::http::StatusCode::UNPROCESSABLE_ENTITY
            | axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE
            | axum::http::StatusCode::PAYLOAD_TOO_LARGE
    ) && !response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|v| {
            v.to_str()
                .unwrap_or_default()
                .starts_with("application/json")
        })
    {
        let status = if response.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
            axum::http::StatusCode::PAYLOAD_TOO_LARGE
        } else {
            axum::http::StatusCode::BAD_REQUEST
        };
        response = crate::error::ApiError::new(
            status,
            if status == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
                "PAYLOAD_TOO_LARGE"
            } else {
                "VALIDATION_ERROR"
            },
            "Invalid request",
        )
        .into_response();
    }
    if fresh {
        let value = format!(
            "fluida-session={}.{}; Path=/api; HttpOnly; SameSite=Strict; Max-Age=31536000{}",
            owner,
            sign(&owner, &state.secret),
            if secure { "; Secure" } else { "" }
        );
        if let Ok(v) = value.parse() {
            response.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    response
}
fn sign(id: &str, secret: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key");
    mac.update(id.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}
fn verify(value: &str, secret: &[u8]) -> Option<String> {
    let (id, sig) = value.rsplit_once('.')?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        || sign(id, secret) != sig
    {
        return None;
    }
    Some(id.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_cookie_round_trip() {
        let value = format!("abc.{}", sign("abc", b"secret"));
        assert_eq!(verify(&value, b"secret").as_deref(), Some("abc"));
        assert!(verify("abc.invalid", b"secret").is_none())
    }
}
