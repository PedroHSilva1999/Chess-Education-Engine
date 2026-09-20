use std::{collections::BTreeMap, sync::Arc, time::Duration};

use chess_core::{ChessError, ChessMove, ChessRulesEngine};
use chess_education::{
    ExerciseFactory, ExerciseRequest, ExerciseValidator, ObjectiveContext, ObjectiveEvaluation,
    ObjectiveSpec, ObjectiveStatus, OpponentConfig, StructuredHint,
};
use chess_engine::{ChessAi, EngineDifficulty, EngineError};
use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    BoardUiConfig, ChessSession, FeedbackType, HintResult, MoveRecord, PlayMoveResult,
    RepositoryError, SessionMetrics, SessionRepository, SessionSnapshot, SessionStatus,
    StructuredFeedback,
};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session not found")]
    NotFound,
    #[error("session is no longer playable")]
    NotPlayable,
    #[error("it is not the player's turn")]
    NotPlayerTurn,
    #[error("move limit reached")]
    MoveLimitReached,
    #[error("exercise error: {0}")]
    Exercise(String),
    #[error(transparent)]
    Chess(#[from] ChessError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("engine task failed")]
    EngineTask,
    #[error("engine search timed out")]
    EngineTimeout,
}

pub struct SessionService {
    repository: Arc<dyn SessionRepository>,
    rules: Arc<dyn ChessRulesEngine>,
    factory: Arc<dyn ExerciseFactory>,
    evaluator: Arc<dyn ExerciseValidator>,
    engine: Arc<dyn ChessAi>,
}

impl SessionService {
    #[must_use]
    pub fn new(
        repository: Arc<dyn SessionRepository>,
        rules: Arc<dyn ChessRulesEngine>,
        factory: Arc<dyn ExerciseFactory>,
        evaluator: Arc<dyn ExerciseValidator>,
        engine: Arc<dyn ChessAi>,
    ) -> Self {
        Self {
            repository,
            rules,
            factory,
            evaluator,
            engine,
        }
    }

    pub async fn create(&self, request: &ExerciseRequest) -> Result<SessionSnapshot, SessionError> {
        let exercise = self
            .factory
            .create(request)
            .map_err(|error| SessionError::Exercise(error.to_string()))?;
        let now = Utc::now();
        let session = ChessSession {
            id: Uuid::new_v4(),
            position: exercise.initial_position.clone(),
            player_color: exercise.player_color,
            exercise,
            status: SessionStatus::Created,
            history: Vec::new(),
            metrics: SessionMetrics::default(),
            objective: ObjectiveEvaluation::in_progress(0.0),
            hint_level: 0,
            created_at: now,
            updated_at: now,
        };
        self.repository.save(session.clone()).await?;
        self.snapshot(session)
    }

    pub async fn get(&self, id: Uuid) -> Result<SessionSnapshot, SessionError> {
        let session = self.load(id).await?;
        self.snapshot(session)
    }

    pub async fn play_move(
        &self,
        id: Uuid,
        chess_move: ChessMove,
    ) -> Result<PlayMoveResult, SessionError> {
        let mut session = self.load(id).await?;
        if matches!(
            session.status,
            SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Abandoned
        ) {
            return Err(SessionError::NotPlayable);
        }
        if session.history.len() >= usize::from(session.exercise.rules.move_limit) {
            return Err(SessionError::MoveLimitReached);
        }
        let status = self.rules.status(&session.position)?;
        if session.exercise.opponent.is_some() && status.side_to_move != session.player_color {
            return Err(SessionError::NotPlayerTurn);
        }

        session.metrics.moves += 1;
        let before = session.position.clone();
        let after = match self.rules.apply_move(&before, &chess_move) {
            Ok(after) => after,
            Err(ChessError::IllegalMove(_)) => {
                session.metrics.incorrect_moves += 1;
                session.updated_at = Utc::now();
                self.update_duration(&mut session);
                self.repository.save(session.clone()).await?;
                return Ok(PlayMoveResult {
                    valid: false,
                    position: session.position.clone(),
                    status: session.status,
                    feedback: feedback(FeedbackType::Incorrect, "illegal_move"),
                    objective: session.objective.clone(),
                    metrics: session.metrics.clone(),
                    legal_moves: self.rules.legal_moves(&session.position)?,
                    engine_move: None,
                });
            }
            Err(error) => return Err(error.into()),
        };
        session.status = SessionStatus::InProgress;
        session.position = after.clone();
        session.history.push(MoveRecord {
            ply: session.history.len() as u32 + 1,
            chess_move: chess_move.clone(),
            color: status.side_to_move,
            fen_before: before.fen.clone(),
            fen_after: after.fen.clone(),
            played_at: Utc::now(),
        });
        let played = session
            .history
            .iter()
            .map(|record| record.chess_move.clone())
            .collect::<Vec<_>>();
        session.objective = self.evaluator.validate(
            self.rules.as_ref(),
            &ObjectiveContext {
                objective: &session.exercise.objective,
                before: &before,
                after: &after,
                chess_move: &chess_move,
                history: &played,
                player_color: session.player_color,
            },
        );
        if session.objective.status == ObjectiveStatus::Completed {
            session.metrics.correct_moves += 1;
            session.status = SessionStatus::Completed;
        } else if session.objective.status == ObjectiveStatus::Failed {
            session.metrics.incorrect_moves += 1;
            session.status = SessionStatus::Failed;
        } else {
            session.metrics.correct_moves += 1;
        }

        let engine_move = if session.status == SessionStatus::InProgress {
            self.maybe_play_engine(&mut session).await?
        } else {
            None
        };
        if session.status == SessionStatus::InProgress
            && matches!(session.exercise.objective, ObjectiveSpec::PlayFullGame)
            && self.is_claimable_draw(&session)
        {
            session.status = SessionStatus::Completed;
            session.objective = ObjectiveEvaluation::completed();
            session.objective.message_key = "draw_completed".to_owned();
        }
        session.updated_at = Utc::now();
        self.update_duration(&mut session);
        self.repository.save(session.clone()).await?;
        let legal_moves = if session.status == SessionStatus::InProgress {
            self.rules.legal_moves(&session.position)?
        } else {
            Vec::new()
        };
        let feedback_type = if session.status == SessionStatus::Failed {
            FeedbackType::Incorrect
        } else {
            FeedbackType::Correct
        };
        let message_key = session.objective.message_key.clone();
        Ok(PlayMoveResult {
            valid: true,
            position: session.position,
            status: session.status,
            feedback: feedback(feedback_type, &message_key),
            objective: session.objective,
            metrics: session.metrics,
            legal_moves,
            engine_move,
        })
    }

    pub async fn hint(&self, id: Uuid) -> Result<HintResult, SessionError> {
        let mut session = self.load(id).await?;
        if matches!(
            session.status,
            SessionStatus::Completed | SessionStatus::Abandoned
        ) {
            return Err(SessionError::NotPlayable);
        }
        session.hint_level = (session.hint_level + 1).min(3);
        session.metrics.hints_used += 1;
        let suggested = self.suggested_move(&session)?;
        let hint = match session.hint_level {
            1 => StructuredHint {
                level: 1,
                hint_type: "piece".to_owned(),
                square: suggested
                    .as_ref()
                    .map(|value| value.from.as_str().to_owned()),
                squares: Vec::new(),
                chess_move: None,
            },
            2 => StructuredHint {
                level: 2,
                hint_type: "candidate_squares".to_owned(),
                square: suggested
                    .as_ref()
                    .map(|value| value.from.as_str().to_owned()),
                squares: self
                    .rules
                    .legal_moves(&session.position)?
                    .into_iter()
                    .filter(|value| {
                        suggested
                            .as_ref()
                            .is_none_or(|suggested| value.from == suggested.from)
                    })
                    .map(|value| value.to.as_str().to_owned())
                    .collect(),
                chess_move: None,
            },
            _ => StructuredHint {
                level: 3,
                hint_type: "move".to_owned(),
                square: None,
                squares: Vec::new(),
                chess_move: suggested,
            },
        };
        session.updated_at = Utc::now();
        self.update_duration(&mut session);
        self.repository.save(session.clone()).await?;
        Ok(HintResult {
            hint,
            metrics: session.metrics,
        })
    }

    pub async fn reset(&self, id: Uuid) -> Result<SessionSnapshot, SessionError> {
        let mut session = self.load(id).await?;
        session.position = session.exercise.initial_position.clone();
        session.status = SessionStatus::Created;
        session.history.clear();
        session.metrics = SessionMetrics::default();
        session.objective = ObjectiveEvaluation::in_progress(0.0);
        session.hint_level = 0;
        session.created_at = Utc::now();
        session.updated_at = session.created_at;
        self.repository.save(session.clone()).await?;
        self.snapshot(session)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), SessionError> {
        if self.repository.delete(id).await? {
            Ok(())
        } else {
            Err(SessionError::NotFound)
        }
    }

    async fn load(&self, id: Uuid) -> Result<ChessSession, SessionError> {
        self.repository.get(id).await?.ok_or(SessionError::NotFound)
    }

    fn snapshot(&self, mut session: ChessSession) -> Result<SessionSnapshot, SessionError> {
        self.update_duration(&mut session);
        let legal_moves = if matches!(
            session.status,
            SessionStatus::Completed | SessionStatus::Failed
        ) {
            Vec::new()
        } else {
            self.rules.legal_moves(&session.position)?
        };
        Ok(SessionSnapshot {
            session_id: session.id,
            status: session.status,
            board: session.position.clone(),
            player_color: session.player_color,
            exercise: session.exercise.clone(),
            objective: session.objective.clone(),
            metrics: session.metrics.clone(),
            history: session.history.clone(),
            legal_moves,
            ui: BoardUiConfig::from(&session),
            ui_url: format!("/play/{}", session.id),
        })
    }

    fn update_duration(&self, session: &mut ChessSession) {
        session.metrics.duration_ms =
            (Utc::now() - session.created_at).num_milliseconds().max(0) as u64;
    }

    async fn maybe_play_engine(
        &self,
        session: &mut ChessSession,
    ) -> Result<Option<ChessMove>, SessionError> {
        let level = match session.exercise.opponent {
            Some(OpponentConfig::Engine { difficulty }) => difficulty,
            _ => return Ok(None),
        };
        let status = self.rules.status(&session.position)?;
        if status.game_over || status.side_to_move == session.player_color {
            return Ok(None);
        }
        let engine = Arc::clone(&self.engine);
        let position = session.position.clone();
        let task = tokio::task::spawn_blocking(move || {
            engine.select_move(&position, EngineDifficulty::educational_level(level))
        });
        let chess_move = tokio::time::timeout(Duration::from_millis(2_200), task)
            .await
            .map_err(|_| SessionError::EngineTimeout)?
            .map_err(|_| SessionError::EngineTask)??;
        let before = session.position.clone();
        let color = self.rules.status(&before)?.side_to_move;
        let after = self.rules.apply_move(&before, &chess_move)?;
        session.history.push(MoveRecord {
            ply: session.history.len() as u32 + 1,
            chess_move: chess_move.clone(),
            color,
            fen_before: before.fen,
            fen_after: after.fen.clone(),
            played_at: Utc::now(),
        });
        session.position = after;
        if self.rules.status(&session.position)?.game_over {
            session.status = SessionStatus::Completed;
            session.objective = ObjectiveEvaluation::completed();
        }
        Ok(Some(chess_move))
    }

    fn suggested_move(&self, session: &ChessSession) -> Result<Option<ChessMove>, SessionError> {
        let legal = self.rules.legal_moves(&session.position)?;
        let suggested = match &session.exercise.objective {
            ObjectiveSpec::BestMove { uci } => ChessMove::from_uci(uci).ok(),
            ObjectiveSpec::CompleteSequence { moves } => moves
                .get(session.history.len())
                .and_then(|value| ChessMove::from_uci(value).ok()),
            ObjectiveSpec::MovePiece { piece, target } => legal
                .iter()
                .find(|chess_move| {
                    self.rules
                        .piece_at(&session.position, &chess_move.from)
                        .ok()
                        .flatten()
                        .is_some_and(|found| found.piece == *piece)
                        && target
                            .as_ref()
                            .is_none_or(|target| chess_move.to.as_str() == target)
                })
                .cloned(),
            _ => self
                .engine
                .select_move(&session.position, EngineDifficulty::educational_level(3))
                .ok(),
        };
        Ok(suggested.or_else(|| legal.into_iter().next()))
    }

    fn is_claimable_draw(&self, session: &ChessSession) -> bool {
        if session.position.is_fifty_move_draw() {
            return true;
        }
        let Some(current_key) = session.position.repetition_key() else {
            return false;
        };
        let initial_matches = usize::from(
            session.exercise.initial_position.repetition_key().as_ref() == Some(&current_key),
        );
        let repeated = session
            .history
            .iter()
            .filter(|record| repetition_key(&record.fen_after).as_ref() == Some(&current_key))
            .count();
        initial_matches + repeated >= 3
    }
}

fn feedback(feedback_type: FeedbackType, message_key: &str) -> StructuredFeedback {
    StructuredFeedback {
        feedback_type,
        message_key: message_key.to_owned(),
        variables: BTreeMap::new(),
    }
}

fn repetition_key(fen: &str) -> Option<String> {
    let fields = fen.split_whitespace().take(4).collect::<Vec<_>>();
    (fields.len() == 4).then(|| fields.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_core::{Piece, ShakmatyRules};
    use chess_education::{
        DefaultExerciseFactory, DefaultObjectiveEvaluator, Difficulty, ExerciseConfig,
        ExerciseRules, ExerciseType, SessionMode, TrainingConfig,
    };
    use chess_engine::SimpleEngine;

    fn service() -> SessionService {
        let rules: Arc<dyn ChessRulesEngine> = Arc::new(ShakmatyRules);
        SessionService::new(
            Arc::new(crate::InMemorySessionRepository::default()),
            Arc::clone(&rules),
            Arc::new(DefaultExerciseFactory::new(Arc::clone(&rules))),
            Arc::new(DefaultObjectiveEvaluator),
            Arc::new(SimpleEngine::new(rules)),
        )
    }

    fn mate_request() -> ExerciseRequest {
        ExerciseRequest {
            mode: SessionMode::Exercise,
            exercise: Some(ExerciseConfig {
                exercise_type: ExerciseType::Checkmate,
                difficulty: Difficulty::Beginner,
                max_moves: Some(1),
                position: None,
                objective: None,
                rules: ExerciseRules::default(),
                opponent: None,
            }),
            piece: None,
            difficulty: Difficulty::Beginner,
            config: TrainingConfig::default(),
            position: None,
            objective: None,
            player_color: None,
            opponent: None,
        }
    }

    #[tokio::test]
    async fn completes_a_data_driven_mate_in_one() {
        let service = service();
        let session = service.create(&mate_request()).await.unwrap();
        let result = service
            .play_move(
                session.session_id,
                ChessMove::new("g6", "g7", None).unwrap(),
            )
            .await
            .unwrap();
        assert!(result.valid);
        assert_eq!(result.status, SessionStatus::Completed);
    }

    #[tokio::test]
    async fn keeps_sessions_isolated_and_tracks_errors() {
        let service = service();
        let first = service.create(&mate_request()).await.unwrap();
        let second = service.create(&mate_request()).await.unwrap();
        let result = service
            .play_move(first.session_id, ChessMove::new("g6", "f4", None).unwrap())
            .await
            .unwrap();
        assert!(!result.valid);
        assert_eq!(result.metrics.incorrect_moves, 1);
        assert!(
            service
                .get(second.session_id)
                .await
                .unwrap()
                .history
                .is_empty()
        );
    }

    #[tokio::test]
    async fn piece_training_produces_structured_hints() {
        let service = service();
        let request = ExerciseRequest {
            mode: SessionMode::PieceTraining,
            exercise: None,
            piece: Some(Piece::Knight),
            difficulty: Difficulty::Beginner,
            config: TrainingConfig::default(),
            position: None,
            objective: None,
            player_color: None,
            opponent: None,
        };
        let session = service.create(&request).await.unwrap();
        let hint = service.hint(session.session_id).await.unwrap();
        assert_eq!(hint.hint.level, 1);
        assert_eq!(hint.hint.square.as_deref(), Some("b1"));
    }
}
