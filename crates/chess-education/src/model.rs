use std::collections::BTreeMap;

use chess_core::{ChessMove, Color, Piece, Position};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    Exercise,
    PieceTraining,
    FullGame,
    FreePlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseType {
    PieceMovement,
    Puzzle,
    Checkmate,
    Tactics,
    Opening,
    Endgame,
    FullGame,
    FreePlay,
    CheckEscape,
    CaptureTraining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    #[default]
    Beginner,
    Intermediate,
    Advanced,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseRules {
    #[serde(default)]
    pub allow_undo: bool,
    #[serde(default = "yes")]
    pub show_legal_moves: bool,
    #[serde(default = "yes")]
    pub show_check: bool,
    #[serde(default = "default_move_limit")]
    pub move_limit: u16,
}

const fn yes() -> bool {
    true
}

const fn default_move_limit() -> u16 {
    256
}

impl Default for ExerciseRules {
    fn default() -> Self {
        Self {
            allow_undo: false,
            show_legal_moves: true,
            show_check: true,
            move_limit: default_move_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    #[serde(default = "yes")]
    pub show_legal_moves: bool,
    #[serde(default = "default_task_count")]
    pub number_of_tasks: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

const fn default_task_count() -> u16 {
    1
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            show_legal_moves: true,
            number_of_tasks: default_task_count(),
            target: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpponentConfig {
    Engine {
        #[serde(default = "default_engine_level")]
        difficulty: u8,
    },
    Human,
}

const fn default_engine_level() -> u8 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectiveSpec {
    Checkmate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_moves: Option<u16>,
    },
    Check,
    Capture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        piece: Option<Piece>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        within_moves: Option<u16>,
    },
    MovePiece {
        piece: Piece,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    ReachSquare {
        target: String,
    },
    Promote,
    EscapeCheck,
    WinMaterial,
    Survive {
        moves: u16,
    },
    BestMove {
        uci: String,
    },
    CompleteSequence {
        moves: Vec<String>,
    },
    PlayFullGame,
}

impl ObjectiveSpec {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Checkmate { .. } => "checkmate",
            Self::Check => "check",
            Self::Capture { .. } => "capture",
            Self::MovePiece { .. } => "move_piece",
            Self::ReachSquare { .. } => "reach_square",
            Self::Promote => "promote",
            Self::EscapeCheck => "escape_check",
            Self::WinMaterial => "win_material",
            Self::Survive { .. } => "survive",
            Self::BestMove { .. } => "best_move",
            Self::CompleteSequence { .. } => "complete_sequence",
            Self::PlayFullGame => "play_full_game",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseConfig {
    #[serde(rename = "type")]
    pub exercise_type: ExerciseType,
    #[serde(default)]
    pub difficulty: Difficulty,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_moves: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<PositionSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<ObjectiveSpec>,
    #[serde(default)]
    pub rules: ExerciseRules,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opponent: Option<OpponentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseRequest {
    pub mode: SessionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exercise: Option<ExerciseConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece: Option<Piece>,
    #[serde(default)]
    pub difficulty: Difficulty,
    #[serde(default)]
    pub config: TrainingConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<PositionSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<ObjectiveSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opponent: Option<OpponentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    pub exercise_type: ExerciseType,
    pub title_key: String,
    pub initial_position: Position,
    pub player_color: Color,
    pub objective: ObjectiveSpec,
    pub rules: ExerciseRules,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opponent: Option<OpponentConfig>,
    pub difficulty: Difficulty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveEvaluation {
    pub status: ObjectiveStatus,
    pub progress: f32,
    pub message_key: String,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
}

impl ObjectiveEvaluation {
    #[must_use]
    pub fn in_progress(progress: f32) -> Self {
        Self {
            status: ObjectiveStatus::InProgress,
            progress: progress.clamp(0.0, 0.99),
            message_key: "objective_in_progress".to_owned(),
            variables: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn completed() -> Self {
        Self {
            status: ObjectiveStatus::Completed,
            progress: 1.0,
            message_key: "objective_completed".to_owned(),
            variables: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn failed(message_key: &str) -> Self {
        Self {
            status: ObjectiveStatus::Failed,
            progress: 0.0,
            message_key: message_key.to_owned(),
            variables: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredHint {
    pub level: u8,
    #[serde(rename = "type")]
    pub hint_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub square: Option<String>,
    #[serde(default)]
    pub squares: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chess_move: Option<ChessMove>,
}
