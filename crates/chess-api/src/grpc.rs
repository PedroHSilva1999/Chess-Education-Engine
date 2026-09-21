use std::{net::SocketAddr, sync::Arc};

use chess_core::{ChessMove, Color, Piece};
use chess_education::{
    Difficulty, ExerciseConfig, ExerciseRequest, ExerciseRules, ExerciseType, OpponentConfig,
    PositionSource, SessionMode, TrainingConfig,
};
use chess_session::{SessionError, SessionService, SessionSnapshot, SessionStatus};
use tonic::{Request, Response, Status, transport::Server};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("chess.education.v1");
}

use proto::{
    CreateSessionRequest, DeleteSessionRequest, DeleteSessionResponse, GetSessionRequest,
    HintRequest, HintResponse, PlayMoveRequest, PlayMoveResponse, ResetSessionRequest,
    SessionResponse,
    chess_education_server::{ChessEducation, ChessEducationServer},
};

pub struct GrpcChessEducation {
    service: Arc<SessionService>,
}

impl GrpcChessEducation {
    #[must_use]
    pub fn new(service: Arc<SessionService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl ChessEducation for GrpcChessEducation {
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<SessionResponse>, Status> {
        let request = exercise_request(request.into_inner())?;
        let session = crate::rest::with_play_url(
            self.service.create(&request).await.map_err(map_error)?,
        );
        Ok(Response::new(session_response(&session)?))
    }

    async fn play_move(
        &self,
        request: Request<PlayMoveRequest>,
    ) -> Result<Response<PlayMoveResponse>, Status> {
        let request = request.into_inner();
        let id = parse_uuid(&request.session_id)?;
        let promotion = if request.promotion.is_empty() {
            None
        } else {
            Some(parse_piece(&request.promotion)?)
        };
        let chess_move = ChessMove::new(request.from, request.to, promotion)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        let result = self
            .service
            .play_move(id, chess_move)
            .await
            .map_err(map_error)?;
        let json =
            serde_json::to_string(&result).map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(PlayMoveResponse {
            valid: result.valid,
            status: status_name(result.status).to_owned(),
            fen: result.position.fen,
            objective_completed: result.objective.progress >= 1.0,
            feedback_key: result.feedback.message_key,
            json,
        }))
    }

    async fn get_session(
        &self,
        request: Request<GetSessionRequest>,
    ) -> Result<Response<SessionResponse>, Status> {
        let id = parse_uuid(&request.into_inner().session_id)?;
        let session = crate::rest::with_play_url(self.service.get(id).await.map_err(map_error)?);
        Ok(Response::new(session_response(&session)?))
    }

    async fn get_hint(
        &self,
        request: Request<HintRequest>,
    ) -> Result<Response<HintResponse>, Status> {
        let id = parse_uuid(&request.into_inner().session_id)?;
        let result = self.service.hint(id).await.map_err(map_error)?;
        let json =
            serde_json::to_string(&result).map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(HintResponse {
            level: u32::from(result.hint.level),
            hint_type: result.hint.hint_type,
            json,
        }))
    }

    async fn reset_session(
        &self,
        request: Request<ResetSessionRequest>,
    ) -> Result<Response<SessionResponse>, Status> {
        let id = parse_uuid(&request.into_inner().session_id)?;
        let session = crate::rest::with_play_url(self.service.reset(id).await.map_err(map_error)?);
        Ok(Response::new(session_response(&session)?))
    }

    async fn delete_session(
        &self,
        request: Request<DeleteSessionRequest>,
    ) -> Result<Response<DeleteSessionResponse>, Status> {
        let id = parse_uuid(&request.into_inner().session_id)?;
        self.service.delete(id).await.map_err(map_error)?;
        Ok(Response::new(DeleteSessionResponse { deleted: true }))
    }
}

pub async fn serve(
    address: SocketAddr,
    service: Arc<SessionService>,
) -> Result<(), tonic::transport::Error> {
    Server::builder()
        .add_service(ChessEducationServer::new(GrpcChessEducation::new(service)))
        .serve(address)
        .await
}

fn exercise_request(request: CreateSessionRequest) -> Result<ExerciseRequest, Status> {
    if !request.request_json.is_empty() {
        return serde_json::from_str(&request.request_json)
            .map_err(|error| Status::invalid_argument(error.to_string()));
    }
    let mode = match request.mode.as_str() {
        "piece_training" => SessionMode::PieceTraining,
        "full_game" => SessionMode::FullGame,
        "free_play" => SessionMode::FreePlay,
        "" | "exercise" => SessionMode::Exercise,
        value => return Err(Status::invalid_argument(format!("unknown mode: {value}"))),
    };
    let difficulty = parse_difficulty(&request.difficulty)?;
    let piece = (!request.piece.is_empty())
        .then(|| parse_piece(&request.piece))
        .transpose()?;
    let position = (!request.fen.is_empty()).then_some(PositionSource {
        fen: Some(request.fen),
        generator: None,
    });
    let exercise = if request.exercise_type.is_empty() {
        None
    } else {
        Some(ExerciseConfig {
            exercise_type: parse_exercise_type(&request.exercise_type)?,
            difficulty,
            max_moves: (request.max_moves > 0).then_some(request.max_moves as u16),
            position: position.clone(),
            objective: None,
            rules: ExerciseRules::default(),
            opponent: None,
        })
    };
    let opponent =
        (mode == SessionMode::FullGame).then_some(OpponentConfig::Engine { difficulty: 2 });
    Ok(ExerciseRequest {
        mode,
        exercise,
        piece,
        difficulty,
        config: TrainingConfig {
            show_legal_moves: true,
            number_of_tasks: 1,
            target: (!request.target.is_empty()).then_some(request.target),
        },
        position,
        objective: None,
        player_color: Some(Color::White),
        opponent,
    })
}

fn session_response(session: &SessionSnapshot) -> Result<SessionResponse, Status> {
    Ok(SessionResponse {
        session_id: session.session_id.to_string(),
        status: status_name(session.status).to_owned(),
        fen: session.board.fen.clone(),
        player_color: color_name(session.player_color).to_owned(),
        objective_type: session.exercise.objective.kind().to_owned(),
        ui_url: session.ui_url.clone(),
        json: serde_json::to_string(session)
            .map_err(|error| Status::internal(error.to_string()))?,
    })
}

fn parse_uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value).map_err(|_| Status::invalid_argument("invalid session UUID"))
}

fn parse_piece(value: &str) -> Result<Piece, Status> {
    match value.to_ascii_lowercase().as_str() {
        "pawn" => Ok(Piece::Pawn),
        "knight" => Ok(Piece::Knight),
        "bishop" => Ok(Piece::Bishop),
        "rook" => Ok(Piece::Rook),
        "queen" => Ok(Piece::Queen),
        "king" => Ok(Piece::King),
        _ => Err(Status::invalid_argument("unknown piece")),
    }
}

fn parse_difficulty(value: &str) -> Result<Difficulty, Status> {
    match value.to_ascii_lowercase().as_str() {
        "" | "beginner" => Ok(Difficulty::Beginner),
        "intermediate" => Ok(Difficulty::Intermediate),
        "advanced" => Ok(Difficulty::Advanced),
        _ => Err(Status::invalid_argument("unknown difficulty")),
    }
}

fn parse_exercise_type(value: &str) -> Result<ExerciseType, Status> {
    match value.to_ascii_lowercase().as_str() {
        "piece_movement" => Ok(ExerciseType::PieceMovement),
        "puzzle" => Ok(ExerciseType::Puzzle),
        "checkmate" => Ok(ExerciseType::Checkmate),
        "tactics" => Ok(ExerciseType::Tactics),
        "opening" => Ok(ExerciseType::Opening),
        "endgame" => Ok(ExerciseType::Endgame),
        "full_game" => Ok(ExerciseType::FullGame),
        "free_play" => Ok(ExerciseType::FreePlay),
        "check_escape" => Ok(ExerciseType::CheckEscape),
        "capture_training" => Ok(ExerciseType::CaptureTraining),
        _ => Err(Status::invalid_argument("unknown exercise type")),
    }
}

const fn status_name(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Created => "created",
        SessionStatus::InProgress => "in_progress",
        SessionStatus::Completed => "completed",
        SessionStatus::Failed => "failed",
        SessionStatus::Abandoned => "abandoned",
    }
}

const fn color_name(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Black => "black",
    }
}

fn map_error(error: SessionError) -> Status {
    match error {
        SessionError::NotFound => Status::not_found(error.to_string()),
        SessionError::NotPlayable
        | SessionError::NotPlayerTurn
        | SessionError::MoveLimitReached => Status::failed_precondition(error.to_string()),
        SessionError::Exercise(_) | SessionError::Chess(_) => {
            Status::invalid_argument(error.to_string())
        }
        SessionError::EngineTimeout => Status::deadline_exceeded(error.to_string()),
        SessionError::Repository(_) | SessionError::Engine(_) | SessionError::EngineTask => {
            Status::internal("internal service error")
        }
    }
}
