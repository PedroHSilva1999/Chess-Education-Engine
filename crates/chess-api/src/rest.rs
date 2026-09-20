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
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use uuid::Uuid;

use crate::{
    error::ApiError,
    static_assets::{APP_JS, INDEX_HTML, STYLES_CSS},
};

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<SessionService>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
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
        .route("/", get(index))
        .route("/play/{session_id}", get(index))
        .route("/assets/app.js", get(app_js))
        .route("/assets/styles.css", get(styles_css))
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
    let methods = [Method::GET, Method::POST, Method::DELETE];
    match std::env::var("CORS_ORIGIN") {
        Ok(origin) if origin != "*" => origin
            .parse::<HeaderValue>()
            .map(|origin| {
                CorsLayer::new()
                    .allow_origin(origin)
                    .allow_methods(methods.clone())
            })
            .unwrap_or_else(|_| CorsLayer::new().allow_origin(Any).allow_methods(methods)),
        _ => CorsLayer::new().allow_origin(Any).allow_methods(methods),
    }
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
    let session = state.service.create(&request).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionSnapshot>, ApiError> {
    Ok(Json(state.service.get(id).await?))
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
    Ok(Json(state.service.reset(id).await?))
}

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        APP_JS,
    )
}

async fn styles_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLES_CSS,
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
}
