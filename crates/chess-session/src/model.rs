use std::collections::BTreeMap;

use chess_core::{ChessMove, Color, Position};
use chess_education::{Exercise, ObjectiveEvaluation, StructuredHint};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Created,
    InProgress,
    Completed,
    Failed,
    Abandoned,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub moves: u32,
    pub correct_moves: u32,
    pub incorrect_moves: u32,
    pub hints_used: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRecord {
    pub ply: u32,
    pub chess_move: ChessMove,
    pub color: Color,
    pub fen_before: String,
    pub fen_after: String,
    pub played_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChessSession {
    pub id: Uuid,
    pub exercise: Exercise,
    pub position: Position,
    pub player_color: Color,
    pub status: SessionStatus,
    pub history: Vec<MoveRecord>,
    pub metrics: SessionMetrics,
    pub objective: ObjectiveEvaluation,
    pub hint_level: u8,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardInteraction {
    pub drag_enabled: bool,
    pub click_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardUiConfig {
    pub orientation: Color,
    pub interaction: BoardInteraction,
    pub hints_enabled: bool,
    pub show_legal_moves: bool,
    pub show_coordinates: bool,
}

impl From<&ChessSession> for BoardUiConfig {
    fn from(session: &ChessSession) -> Self {
        Self {
            orientation: session.player_color,
            interaction: BoardInteraction {
                drag_enabled: true,
                click_enabled: true,
            },
            hints_enabled: true,
            show_legal_moves: session.exercise.rules.show_legal_moves,
            show_coordinates: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: Uuid,
    pub status: SessionStatus,
    pub board: Position,
    pub player_color: Color,
    pub exercise: Exercise,
    pub objective: ObjectiveEvaluation,
    pub metrics: SessionMetrics,
    pub history: Vec<MoveRecord>,
    pub legal_moves: Vec<ChessMove>,
    pub ui: BoardUiConfig,
    pub ui_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    Correct,
    Incorrect,
    Informational,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredFeedback {
    #[serde(rename = "type")]
    pub feedback_type: FeedbackType,
    pub message_key: String,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayMoveResult {
    pub valid: bool,
    pub position: Position,
    pub status: SessionStatus,
    pub feedback: StructuredFeedback,
    pub objective: ObjectiveEvaluation,
    pub metrics: SessionMetrics,
    pub legal_moves: Vec<ChessMove>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_move: Option<ChessMove>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HintResult {
    pub hint: StructuredHint,
    pub metrics: SessionMetrics,
}
