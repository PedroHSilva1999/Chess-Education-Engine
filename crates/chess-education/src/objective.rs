use chess_core::{ChessMove, ChessRulesEngine, Color, Piece, Position, Square};

use crate::{ObjectiveEvaluation, ObjectiveSpec, ObjectiveStatus};

pub struct ObjectiveContext<'a> {
    pub objective: &'a ObjectiveSpec,
    pub before: &'a Position,
    pub after: &'a Position,
    pub chess_move: &'a ChessMove,
    pub history: &'a [ChessMove],
    pub player_color: Color,
}

pub trait ExerciseValidator: Send + Sync {
    fn validate(
        &self,
        rules: &dyn ChessRulesEngine,
        context: &ObjectiveContext<'_>,
    ) -> ObjectiveEvaluation;
}

#[derive(Debug, Default)]
pub struct DefaultObjectiveEvaluator;

impl DefaultObjectiveEvaluator {
    fn moved_piece_matches(
        rules: &dyn ChessRulesEngine,
        context: &ObjectiveContext<'_>,
        expected: Piece,
    ) -> bool {
        rules
            .piece_at(context.before, &context.chess_move.from)
            .ok()
            .flatten()
            .is_some_and(|piece| piece.piece == expected)
    }

    fn failed_if_limit_reached(
        evaluation: ObjectiveEvaluation,
        history_len: usize,
        max_moves: Option<u16>,
    ) -> ObjectiveEvaluation {
        if evaluation.status != ObjectiveStatus::Completed
            && max_moves.is_some_and(|limit| history_len >= usize::from(limit))
        {
            ObjectiveEvaluation::failed("move_limit_reached")
        } else {
            evaluation
        }
    }
}

impl ExerciseValidator for DefaultObjectiveEvaluator {
    fn validate(
        &self,
        rules: &dyn ChessRulesEngine,
        context: &ObjectiveContext<'_>,
    ) -> ObjectiveEvaluation {
        let status = match rules.status(context.after) {
            Ok(status) => status,
            Err(_) => return ObjectiveEvaluation::failed("position_validation_failed"),
        };
        match context.objective {
            ObjectiveSpec::Checkmate { max_moves } => Self::failed_if_limit_reached(
                if status.checkmate {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.5)
                },
                context.history.len(),
                *max_moves,
            ),
            ObjectiveSpec::Check => {
                if status.in_check {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.0)
                }
            }
            ObjectiveSpec::Capture {
                piece,
                within_moves,
            } => {
                let captured = rules
                    .piece_at(context.before, &context.chess_move.to)
                    .ok()
                    .flatten();
                let completed = captured.is_some_and(|captured| {
                    captured.color != context.player_color
                        && piece.is_none_or(|expected| expected == captured.piece)
                });
                Self::failed_if_limit_reached(
                    if completed {
                        ObjectiveEvaluation::completed()
                    } else {
                        ObjectiveEvaluation::in_progress(0.0)
                    },
                    context.history.len(),
                    *within_moves,
                )
            }
            ObjectiveSpec::MovePiece { piece, target } => {
                if Self::moved_piece_matches(rules, context, *piece)
                    && target
                        .as_ref()
                        .is_none_or(|target| target == context.chess_move.to.as_str())
                {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.0)
                }
            }
            ObjectiveSpec::ReachSquare { target } => {
                if context.chess_move.to.as_str() == target {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.0)
                }
            }
            ObjectiveSpec::Promote => {
                if context.chess_move.promotion.is_some() {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.0)
                }
            }
            ObjectiveSpec::EscapeCheck => {
                let was_in_check = rules
                    .status(context.before)
                    .is_ok_and(|status| status.in_check);
                if was_in_check && !status.in_check {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.0)
                }
            }
            ObjectiveSpec::WinMaterial => {
                if rules
                    .material_balance(context.after, context.player_color)
                    .is_ok_and(|balance| balance > 0)
                {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.0)
                }
            }
            ObjectiveSpec::Survive { moves } => {
                if context.history.len() >= usize::from(*moves) && !status.checkmate {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(
                        context.history.len() as f32 / f32::from((*moves).max(1)),
                    )
                }
            }
            ObjectiveSpec::BestMove { uci } => {
                if context.chess_move.to_uci() == *uci {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::failed("better_move_available")
                }
            }
            ObjectiveSpec::CompleteSequence { moves } => {
                let played = context
                    .history
                    .iter()
                    .map(ChessMove::to_uci)
                    .collect::<Vec<_>>();
                if !moves.starts_with(&played) {
                    ObjectiveEvaluation::failed("sequence_mismatch")
                } else if played.len() == moves.len() {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(
                        played.len() as f32 / moves.len().max(1) as f32,
                    )
                }
            }
            ObjectiveSpec::PlayFullGame => {
                if status.game_over {
                    ObjectiveEvaluation::completed()
                } else {
                    ObjectiveEvaluation::in_progress(0.0)
                }
            }
        }
    }
}

#[allow(dead_code)]
fn valid_square(value: &str) -> bool {
    Square::new(value).is_ok()
}
