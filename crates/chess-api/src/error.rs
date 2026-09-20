use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chess_core::ChessError;
use chess_session::SessionError;
use serde_json::json;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_input",
            message: message.into(),
        }
    }
}

impl From<SessionError> for ApiError {
    fn from(error: SessionError) -> Self {
        let (status, code) = match &error {
            SessionError::NotFound => (StatusCode::NOT_FOUND, "session_not_found"),
            SessionError::NotPlayable => (StatusCode::CONFLICT, "session_not_playable"),
            SessionError::NotPlayerTurn => (StatusCode::CONFLICT, "not_player_turn"),
            SessionError::MoveLimitReached => (StatusCode::CONFLICT, "move_limit_reached"),
            SessionError::Exercise(_) | SessionError::Chess(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "invalid_chess_data")
            }
            SessionError::EngineTimeout => (StatusCode::GATEWAY_TIMEOUT, "engine_timeout"),
            SessionError::Repository(_) | SessionError::Engine(_) | SessionError::EngineTask => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        };
        Self {
            status,
            code,
            message: error.to_string(),
        }
    }
}

impl From<ChessError> for ApiError {
    fn from(error: ChessError) -> Self {
        Self::invalid_input(error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": {
                    "code": self.code,
                    "message": self.message,
                }
            })),
        )
            .into_response()
    }
}
