//! Lightweight bounded chess search used by full-game sessions.

use std::{
    cmp::Reverse,
    sync::Arc,
    time::{Duration, Instant},
};

use chess_core::{ChessError, ChessMove, ChessRulesEngine, Position};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MATE_SCORE: i32 = 100_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EngineDifficulty {
    pub search_depth: Option<u8>,
    pub time_limit_ms: Option<u64>,
    pub randomness: f32,
    pub evaluation_noise: f32,
}

impl Default for EngineDifficulty {
    fn default() -> Self {
        Self {
            search_depth: Some(2),
            time_limit_ms: Some(250),
            randomness: 0.1,
            evaluation_noise: 0.0,
        }
    }
}

impl EngineDifficulty {
    #[must_use]
    pub fn educational_level(level: u8) -> Self {
        match level.clamp(1, 5) {
            1 => Self {
                search_depth: Some(1),
                time_limit_ms: Some(100),
                randomness: 0.45,
                evaluation_noise: 0.25,
            },
            2 => Self {
                search_depth: Some(2),
                time_limit_ms: Some(150),
                randomness: 0.25,
                evaluation_noise: 0.15,
            },
            3 => Self::default(),
            4 => Self {
                search_depth: Some(3),
                time_limit_ms: Some(500),
                randomness: 0.03,
                evaluation_noise: 0.0,
            },
            _ => Self {
                search_depth: Some(4),
                time_limit_ms: Some(1_000),
                randomness: 0.0,
                evaluation_noise: 0.0,
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Chess(#[from] ChessError),
    #[error("the position has no legal moves")]
    NoLegalMoves,
}

pub trait ChessAi: Send + Sync {
    fn select_move(
        &self,
        position: &Position,
        difficulty: EngineDifficulty,
    ) -> Result<ChessMove, EngineError>;
}

pub struct SimpleEngine {
    rules: Arc<dyn ChessRulesEngine>,
}

impl SimpleEngine {
    #[must_use]
    pub fn new(rules: Arc<dyn ChessRulesEngine>) -> Self {
        Self { rules }
    }

    fn negamax(
        &self,
        position: &Position,
        depth: u8,
        mut alpha: i32,
        beta: i32,
        deadline: Instant,
    ) -> Result<i32, EngineError> {
        let status = self.rules.status(position)?;
        if status.checkmate {
            return Ok(-MATE_SCORE - i32::from(depth));
        }
        if status.stalemate || status.game_over {
            return Ok(0);
        }
        if depth == 0 || Instant::now() >= deadline {
            return Ok(self.rules.material_balance(position, status.side_to_move)?);
        }

        let mut best = -MATE_SCORE;
        for chess_move in self.rules.legal_moves(position)? {
            if Instant::now() >= deadline {
                break;
            }
            let next = self.rules.apply_move(position, &chess_move)?;
            let score = -self.negamax(&next, depth - 1, -beta, -alpha, deadline)?;
            best = best.max(score);
            alpha = alpha.max(score);
            if alpha >= beta {
                break;
            }
        }
        Ok(best)
    }
}

impl ChessAi for SimpleEngine {
    fn select_move(
        &self,
        position: &Position,
        difficulty: EngineDifficulty,
    ) -> Result<ChessMove, EngineError> {
        let depth = difficulty.search_depth.unwrap_or(2).clamp(1, 5);
        let time_limit = difficulty.time_limit_ms.unwrap_or(250).clamp(25, 2_000);
        let deadline = Instant::now() + Duration::from_millis(time_limit);
        let mut scored = self
            .rules
            .legal_moves(position)?
            .into_iter()
            .map(|chess_move| {
                let score = self
                    .rules
                    .apply_move(position, &chess_move)
                    .and_then(|next| {
                        self.negamax(
                            &next,
                            depth.saturating_sub(1),
                            -MATE_SCORE,
                            MATE_SCORE,
                            deadline,
                        )
                        .map(|score| -score)
                        .map_err(|error| match error {
                            EngineError::Chess(error) => error,
                            EngineError::NoLegalMoves => unreachable!(),
                        })
                    });
                score.map(|score| (chess_move, score))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if scored.is_empty() {
            return Err(EngineError::NoLegalMoves);
        }
        scored.sort_by_key(|(_, score)| Reverse(*score));

        let alternatives = ((scored.len() as f32 * difficulty.randomness.clamp(0.0, 1.0)).round()
            as usize)
            .clamp(1, scored.len());
        let deterministic_seed = position.fen.bytes().fold(0_usize, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(byte as usize)
        });
        Ok(scored.swap_remove(deterministic_seed % alternatives).0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_core::{ChessRulesEngine, ShakmatyRules};

    #[test]
    fn always_returns_a_legal_move() {
        let rules: Arc<dyn ChessRulesEngine> = Arc::new(ShakmatyRules);
        let engine = SimpleEngine::new(Arc::clone(&rules));
        let position = Position::initial();
        let selected = engine
            .select_move(&position, EngineDifficulty::educational_level(1))
            .unwrap();
        assert!(rules.legal_moves(&position).unwrap().contains(&selected));
    }
}
