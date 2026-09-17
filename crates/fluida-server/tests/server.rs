use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use fluida_server::{
    app,
    config::Config,
    services::{file_service::FileService, terminal_service::TerminalService},
};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

fn config(root: &TempDir) -> Config {
    let project = root.path().to_path_buf();
    std::fs::create_dir_all(project.join("src/public")).unwrap();
    std::fs::create_dir_all(project.join("src/client/styles")).unwrap();
    std::fs::create_dir_all(project.join("dist")).unwrap();
    std::fs::write(
        project.join("src/public/index.html"),
        "<!doctype html><title>FluidaOS</title>",
    )
    .unwrap();
    Config {
        host: "127.0.0.1".into(),
        port: 3000,
        session_secret: "test-secret".into(),
        trusted_origins: vec!["https://trusted.example".into()],
        storage_root: project.join("os_storage"),
        registry_root: project.join("app_packages"),
        project_root: project,
    }
}
async fn json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn file_service_contains_paths_and_terminal_is_restricted() {
    let root = TempDir::new().unwrap();
    let files = FileService::new(root.path().join("storage"));
    assert!(files.resolve("../escape").is_err());
    assert!(files.resolve("/absolute").is_err());
    files.write("docs/a.txt", b"hello").await.unwrap();
    assert_eq!(files.read("docs/a.txt").await.unwrap(), "hello");
    let terminal = TerminalService::new(files);
    let result = terminal
        .execute("owner", "cat a.txt", "docs")
        .await
        .unwrap();
    assert_eq!(result["stdout"], "hello");
    assert!(terminal.execute("owner", "sh -c whoami", "").await.is_err());
}

#[tokio::test]
async fn api_preserves_envelopes_cookies_static_assets_and_cors() {
    let root = TempDir::new().unwrap();
    let server = app(config(&root)).await;
    let response = server
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        cookie.contains("Path=/api")
            && cookie.contains("HttpOnly")
            && cookie.contains("SameSite=Strict")
            && cookie.contains("Max-Age=31536000")
    );
    assert!(json(response).await["success"].as_bool().unwrap());
    let system = server
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/system")
                .header(header::COOKIE, cookie.split(';').next().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(system.status(), StatusCode::OK);
    assert!(json(system).await["success"].as_bool().unwrap());
    let invalid = server
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/missing")
                .header(header::COOKIE, "fluida-session=bad.signature")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::NOT_FOUND);
    let body = json(invalid).await;
    assert_eq!(body["error"]["code"], "NOT_FOUND");
    let index = server
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(index.status(), StatusCode::OK);
    assert!(index.headers().get(header::SET_COOKIE).is_none());
    let cors = server
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/settings")
                .header(header::ORIGIN, "https://trusted.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        cors.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "https://trusted.example"
    );
}

#[tokio::test]
async fn oversized_json_is_rejected() {
    let root = TempDir::new().unwrap();
    let server = app(config(&root)).await;
    let huge = format!(
        "{{\"path\":\"a\",\"content\":\"{}\"}}",
        "x".repeat(8 * 1024 * 1024)
    );
    let response = server
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fs/write")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(huge))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[test]
fn cargo_profiles_disable_incremental_compilation() {
    let manifest =
        std::fs::read_to_string(format!("{}/../../Cargo.toml", env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    assert!(manifest.contains("[profile.dev]\nincremental = false"));
    assert!(manifest.contains("[profile.release]\nincremental = false"));
}
