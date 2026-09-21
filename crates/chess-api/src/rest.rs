use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderName, HeaderValue, Method, StatusCode, header},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use chess_core::{ChessMove, Piece};
use chess_education::ExerciseRequest;
use chess_session::{HintResult, PlayMoveResult, SessionService, SessionSnapshot};
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use uuid::Uuid;

use crate::{
    error::ApiError,
    static_assets::{FAVICON_SVG, PLAY_CSS, PLAY_HTML, PLAY_JS},
};

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<SessionService>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct ServiceInfo {
    service: &'static str,
    health: &'static str,
    ready: &'static str,
    api: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct MoveRequest {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub promotion: Option<Piece>,
}

pub fn router(state: AppState) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");
    Router::new()
        .route("/", get(service_info))
        .route("/play/{session_id}", get(play_page))
        .route("/favicon.svg", get(favicon))
        .route("/assets/play.js", get(play_js))
        .route("/assets/play.css", get(play_css))
        .route("/assets/play-config.js", get(play_config))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/v1/sessions", post(create_session))
        .route(
            "/api/v1/sessions/{id}",
            get(get_session).delete(delete_session),
        )
        .route("/api/v1/sessions/{id}/moves", post(play_move))
        .route("/api/v1/sessions/{id}/hint", post(hint))
        .route("/api/v1/sessions/{id}/reset", post(reset))
        .with_state(state)
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(cors_layer())
}

fn cors_layer() -> CorsLayer {
    let methods = [Method::GET, Method::POST, Method::DELETE, Method::OPTIONS];
    let headers = [
        header::ACCEPT,
        header::CONTENT_TYPE,
        HeaderName::from_static("x-request-id"),
    ];
    CorsLayer::new()
        .allow_origin(allow_origin())
        .allow_methods(methods)
        .allow_headers(headers)
}

fn allow_origin() -> AllowOrigin {
    let configured = configured_origins();
    if !configured.is_empty() {
        return AllowOrigin::list(configured);
    }
    if is_hosted() {
        return AllowOrigin::list(
            ["https://invalid.invalid"]
                .into_iter()
                .filter_map(|origin| origin.parse().ok())
                .collect::<Vec<_>>(),
        );
    }
    AllowOrigin::list(local_dev_origins())
}

fn is_hosted() -> bool {
    std::env::var("RAILWAY_ENVIRONMENT").is_ok()
        || std::env::var("RUST_ENV").ok().as_deref() == Some("production")
}

fn configured_origins() -> Vec<HeaderValue> {
    let raw = std::env::var("FRONTEND_ORIGIN")
        .or_else(|_| std::env::var("CORS_ORIGIN"))
        .unwrap_or_default();
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty() && *origin != "*")
        .filter_map(|origin| origin.parse().ok())
        .collect()
}

fn local_dev_origins() -> Vec<HeaderValue> {
    [
        "http://localhost:4173",
        "http://127.0.0.1:4173",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:8080",
        "http://127.0.0.1:8080",
    ]
    .into_iter()
    .filter_map(|origin| origin.parse().ok())
    .collect()
}

async fn service_info() -> Json<ServiceInfo> {
    Json(ServiceInfo {
        service: "chess-education-engine",
        health: "/health",
        ready: "/ready",
        api: "/api/v1/sessions",
    })
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn ready() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ready" })
}

async fn create_session(
    State(state): State<AppState>,
    Json(request): Json<ExerciseRequest>,
) -> Result<(StatusCode, Json<SessionSnapshot>), ApiError> {
    let session = with_play_url(state.service.create(&request).await?);
    Ok((StatusCode::CREATED, Json(session)))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionSnapshot>, ApiError> {
    Ok(Json(with_play_url(state.service.get(id).await?)))
}

async fn play_move(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MoveRequest>,
) -> Result<Json<PlayMoveResult>, ApiError> {
    let chess_move = ChessMove::new(request.from, request.to, request.promotion)?;
    Ok(Json(state.service.play_move(id, chess_move).await?))
}

async fn hint(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<HintResult>, ApiError> {
    Ok(Json(state.service.hint(id).await?))
}

async fn reset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionSnapshot>, ApiError> {
    Ok(Json(with_play_url(state.service.reset(id).await?)))
}

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn with_play_url(mut session: SessionSnapshot) -> SessionSnapshot {
    session.ui_url = play_url(session.session_id);
    session
}

pub(crate) fn play_url(id: Uuid) -> String {
    match public_api_base() {
        Some(base) => format!("{base}/play/{id}"),
        None => format!("/play/{id}"),
    }
}

fn public_api_base() -> Option<String> {
    std::env::var("PUBLIC_API_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty() && looks_like_http_url(value))
        .or_else(|| {
            std::env::var("RAILWAY_PUBLIC_DOMAIN")
                .ok()
                .map(|domain| {
                    domain
                        .trim()
                        .trim_start_matches("https://")
                        .trim_start_matches("http://")
                        .trim_end_matches('/')
                        .to_owned()
                })
                .filter(|domain| !domain.is_empty() && !domain.contains('/'))
                .map(|domain| format!("https://{domain}"))
        })
}

fn looks_like_http_url(value: &str) -> bool {
    (value.starts_with("http://") || value.starts_with("https://")) && !value.contains('\n')
}

fn launcher_url() -> String {
    configured_origins()
        .into_iter()
        .next()
        .and_then(|value| value.to_str().ok().map(str::to_owned))
        .filter(|origin| looks_like_http_url(origin))
        .unwrap_or_default()
}

async fn play_page(Path(_session_id): Path<Uuid>) -> Html<&'static str> {
    Html(PLAY_HTML)
}

async fn play_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        PLAY_JS,
    )
}

async fn play_css() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        PLAY_CSS,
    )
}

async fn play_config() -> impl IntoResponse {
    let url = serde_json::to_string(&launcher_url()).unwrap_or_else(|_| "\"\"".to_owned());
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        format!("window.CHESS_LAUNCHER_URL = {url};\n"),
    )
}

async fn favicon() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        FAVICON_SVG,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    fn app() -> Router {
        router(AppState {
            service: crate::application_service(),
        })
    }

    #[tokio::test]
    async fn health_endpoint_is_available() {
        let response = app()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn root_describes_the_api_without_serving_the_frontend() {
        let response = app()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["service"], "chess-education-engine");
        assert!(String::from_utf8_lossy(&body).contains("/api/v1/sessions"));
    }

    #[tokio::test]
    async fn local_origin_can_preflight_session_create() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/v1/sessions")
                    .header("Origin", "http://localhost:4173")
                    .header("Access-Control-Request-Method", "POST")
                    .header("Access-Control-Request-Headers", "content-type")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "http://localhost:4173"
        );
    }

    #[tokio::test]
    async fn creates_and_completes_a_session_over_http() {
        let app = app();
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "mode": "exercise",
                            "exercise": {
                                "type": "checkmate",
                                "difficulty": "beginner",
                                "max_moves": 1
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let id = body["session_id"].as_str().unwrap();
        assert!(body["ui_url"].as_str().unwrap().ends_with(&format!("/play/{id}")));

        let response = app
            .oneshot(
                Request::post(format!("/api/v1/sessions/{id}/moves"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"from":"g6","to":"g7"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["status"], "completed");
    }

    #[tokio::test]
    async fn play_page_is_served_by_the_api() {
        let id = Uuid::new_v4();
        let response = app()
            .oneshot(
                Request::get(format!("/play/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("id=\"board\""));
        assert!(html.contains("/assets/play.js"));
    }
}
