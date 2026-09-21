use std::sync::Arc;

use chess_core::{ChessRulesEngine, Color, Piece, Position};
use thiserror::Error;

use crate::{
    Exercise, ExerciseRequest, ExerciseRules, ExerciseType, ObjectiveSpec, OpponentConfig,
    PositionSource, SessionMode,
};

#[derive(Debug, Error)]
pub enum ExerciseError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("unsupported exercise configuration: {0}")]
    Unsupported(String),
    #[error("generated an invalid position: {0}")]
    InvalidPosition(String),
}

pub trait PositionGenerator: Send + Sync {
    fn generate(
        &self,
        exercise_type: ExerciseType,
        piece: Option<Piece>,
    ) -> Result<Position, ExerciseError>;
}

pub trait ExerciseFactory: Send + Sync {
    fn create(&self, request: &ExerciseRequest) -> Result<Exercise, ExerciseError>;
}

pub struct DefaultExerciseFactory {
    rules: Arc<dyn ChessRulesEngine>,
}

impl DefaultExerciseFactory {
    #[must_use]
    pub fn new(rules: Arc<dyn ChessRulesEngine>) -> Self {
        Self { rules }
    }

    fn supplied_position(
        source: Option<&PositionSource>,
    ) -> Result<Option<Position>, ExerciseError> {
        source
            .and_then(|source| source.fen.as_ref())
            .map(|fen| {
                Position::from_fen(fen.clone())
                    .map_err(|error| ExerciseError::InvalidPosition(error.to_string()))
            })
            .transpose()
    }
}

impl PositionGenerator for DefaultExerciseFactory {
    fn generate(
        &self,
        exercise_type: ExerciseType,
        piece: Option<Piece>,
    ) -> Result<Position, ExerciseError> {
        let fen = match exercise_type {
            ExerciseType::Checkmate | ExerciseType::Puzzle | ExerciseType::Tactics => {
                "7k/8/5KQ1/8/8/8/8/8 w - - 0 1"
            }
            ExerciseType::CheckEscape => "4r2k/8/8/8/8/8/8/4K3 w - - 0 1",
            ExerciseType::CaptureTraining => "4k3/8/8/8/4r3/8/8/4QK2 w - - 0 1",
            ExerciseType::PieceMovement => match piece.unwrap_or(Piece::Knight) {
                Piece::Knight => "4k3/8/8/8/8/8/8/1N2K3 w - - 0 1",
                Piece::Bishop => "4k3/8/8/8/8/8/3B4/4K3 w - - 0 1",
                Piece::Rook => "4k3/8/8/8/8/8/8/R3K3 w - - 0 1",
                Piece::Queen => "4k3/8/8/8/8/8/8/3QK3 w - - 0 1",
                Piece::King => "7k/8/8/8/8/8/8/4K3 w - - 0 1",
                Piece::Pawn => "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",
            },
            ExerciseType::Opening
            | ExerciseType::Endgame
            | ExerciseType::FullGame
            | ExerciseType::FreePlay => return Ok(Position::initial()),
        };
        Position::from_fen(fen).map_err(|error| ExerciseError::InvalidPosition(error.to_string()))
    }
}

impl ExerciseFactory for DefaultExerciseFactory {
    fn create(&self, request: &ExerciseRequest) -> Result<Exercise, ExerciseError> {
        let config = request.exercise.as_ref();
        let exercise_type = config
            .map(|value| value.exercise_type)
            .unwrap_or(match request.mode {
                SessionMode::PieceTraining => ExerciseType::PieceMovement,
                SessionMode::FullGame => ExerciseType::FullGame,
                SessionMode::FreePlay => ExerciseType::FreePlay,
                SessionMode::Exercise => ExerciseType::Puzzle,
            });
        let difficulty = config
            .map(|value| value.difficulty)
            .unwrap_or(request.difficulty);
        let position_source = config
            .and_then(|value| value.position.as_ref())
            .or(request.position.as_ref());
        let position = Self::supplied_position(position_source)?
            .unwrap_or(self.generate(exercise_type, request.piece)?);
        self.rules
            .validate_position(&position)
            .map_err(|error| ExerciseError::InvalidPosition(error.to_string()))?;

        let target = request.config.target.clone().unwrap_or_else(|| {
            match request.piece.unwrap_or(Piece::Knight) {
                Piece::Knight => "c3",
                Piece::Bishop => "g5",
                Piece::Rook => "a7",
                Piece::Queen => "h5",
                Piece::King => "f2",
                Piece::Pawn => "e4",
            }
            .to_owned()
        });
        let objective = request
            .objective
            .clone()
            .or_else(|| config.and_then(|value| value.objective.clone()))
            .unwrap_or_else(|| match exercise_type {
                ExerciseType::Checkmate | ExerciseType::Puzzle | ExerciseType::Tactics => {
                    ObjectiveSpec::Checkmate {
                        max_moves: config.and_then(|value| value.max_moves).or(Some(1)),
                    }
                }
                ExerciseType::CheckEscape => ObjectiveSpec::EscapeCheck,
                ExerciseType::CaptureTraining => ObjectiveSpec::Capture {
                    piece: None,
                    within_moves: config.and_then(|value| value.max_moves).or(Some(2)),
                },
                ExerciseType::PieceMovement => ObjectiveSpec::MovePiece {
                    piece: request.piece.unwrap_or(Piece::Knight),
                    target: Some(target),
                },
                ExerciseType::Opening
                | ExerciseType::Endgame
                | ExerciseType::FullGame
                | ExerciseType::FreePlay => ObjectiveSpec::PlayFullGame,
            });

        let opponent = config
            .and_then(|value| value.opponent.clone())
            .or_else(|| request.opponent.clone())
            .or_else(|| {
                (exercise_type == ExerciseType::FullGame)
                    .then_some(OpponentConfig::Engine { difficulty: 2 })
            });
        let rules = config
            .map(|value| value.rules.clone())
            .unwrap_or_else(|| ExerciseRules {
                show_legal_moves: request.config.show_legal_moves,
                ..ExerciseRules::default()
            });

        Ok(Exercise {
            exercise_type,
            title_key: format!("exercise_{}", objective.kind()),
            initial_position: position,
            player_color: request.player_color.unwrap_or(Color::White),
            objective,
            rules,
            opponent,
            difficulty,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Difficulty, TrainingConfig};
    use chess_core::{ChessRulesEngine, ShakmatyRules};

    #[test]
    fn creates_piece_training_from_data() {
        let factory = DefaultExerciseFactory::new(Arc::new(ShakmatyRules));
        let exercise = factory
            .create(&ExerciseRequest {
                mode: SessionMode::PieceTraining,
                exercise: None,
                piece: Some(Piece::Knight),
                difficulty: Difficulty::Beginner,
                config: TrainingConfig::default(),
                position: None,
                objective: None,
                player_color: None,
                opponent: None,
            })
            .unwrap();
        assert_eq!(exercise.exercise_type, ExerciseType::PieceMovement);
        assert!(matches!(
            exercise.objective,
            ObjectiveSpec::MovePiece { .. }
        ));
    }

    #[test]
    fn bishop_training_target_is_reachable_in_one_move() {
        let factory = DefaultExerciseFactory::new(Arc::new(ShakmatyRules));
        let exercise = factory
            .create(&ExerciseRequest {
                mode: SessionMode::PieceTraining,
                exercise: None,
                piece: Some(Piece::Bishop),
                difficulty: Difficulty::Beginner,
                config: TrainingConfig {
                    target: Some("g5".to_owned()),
                    ..TrainingConfig::default()
                },
                position: None,
                objective: None,
                player_color: None,
                opponent: None,
            })
            .unwrap();
        assert_eq!(
            exercise.initial_position.fen,
            "4k3/8/8/8/8/8/3B4/4K3 w - - 0 1"
        );
        let moves = ShakmatyRules
            .legal_moves(&exercise.initial_position)
            .unwrap();
        assert!(
            moves
                .iter()
                .any(|chess_move| chess_move.to.as_str() == "g5")
        );
    }

    #[test]
    fn creates_promotion_from_supplied_position() {
        let factory = DefaultExerciseFactory::new(Arc::new(ShakmatyRules));
        let exercise = factory
            .create(&ExerciseRequest {
                mode: SessionMode::Exercise,
                exercise: None,
                piece: None,
                difficulty: Difficulty::Beginner,
                config: TrainingConfig::default(),
                position: Some(PositionSource {
                    fen: Some("4k3/P7/8/8/8/8/8/4K3 w - - 0 1".to_owned()),
                    generator: None,
                }),
                objective: Some(ObjectiveSpec::Promote),
                player_color: None,
                opponent: None,
            })
            .unwrap();
        assert!(matches!(exercise.objective, ObjectiveSpec::Promote));
    }
}
