//! Stable domain contracts around the chess rules implementation.
//!
//! Other crates depend on [`ChessRulesEngine`], never directly on `shakmaty`.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use shakmaty::{
    CastlingMode, Chess, Color as ShakColor, EnPassantMode, Position as ShakPosition, Role,
    Square as ShakSquare, fen::Fen, uci::UciMove,
};
use thiserror::Error;

pub const INITIAL_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    White,
    Black,
}

impl Color {
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }
}

impl From<ShakColor> for Color {
    fn from(value: ShakColor) -> Self {
        match value {
            ShakColor::White => Self::White,
            ShakColor::Black => Self::Black,
        }
    }
}

impl From<Color> for ShakColor {
    fn from(value: Color) -> Self {
        match value {
            Color::White => Self::White,
            Color::Black => Self::Black,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Piece {
    fn from_promotion(character: char) -> Option<Self> {
        match character.to_ascii_lowercase() {
            'q' => Some(Self::Queen),
            'r' => Some(Self::Rook),
            'b' => Some(Self::Bishop),
            'n' => Some(Self::Knight),
            _ => None,
        }
    }

    fn promotion_char(self) -> Option<char> {
        match self {
            Self::Queen => Some('q'),
            Self::Rook => Some('r'),
            Self::Bishop => Some('b'),
            Self::Knight => Some('n'),
            Self::Pawn | Self::King => None,
        }
    }
}

impl From<Role> for Piece {
    fn from(value: Role) -> Self {
        match value {
            Role::Pawn => Self::Pawn,
            Role::Knight => Self::Knight,
            Role::Bishop => Self::Bishop,
            Role::Rook => Self::Rook,
            Role::Queen => Self::Queen,
            Role::King => Self::King,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Square(String);

impl Square {
    pub fn new(value: impl Into<String>) -> Result<Self, ChessError> {
        let value = value.into().to_ascii_lowercase();
        ShakSquare::from_str(&value).map_err(|_| ChessError::InvalidSquare(value.clone()))?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChessMove {
    pub from: Square,
    pub to: Square,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<Piece>,
}

impl ChessMove {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        promotion: Option<Piece>,
    ) -> Result<Self, ChessError> {
        if matches!(promotion, Some(Piece::Pawn | Piece::King)) {
            return Err(ChessError::InvalidPromotion);
        }
        Ok(Self {
            from: Square::new(from)?,
            to: Square::new(to)?,
            promotion,
        })
    }

    #[must_use]
    pub fn to_uci(&self) -> String {
        let mut uci = format!("{}{}", self.from.as_str(), self.to.as_str());
        if let Some(character) = self.promotion.and_then(Piece::promotion_char) {
            uci.push(character);
        }
        uci
    }

    pub fn from_uci(uci: &str) -> Result<Self, ChessError> {
        if !(uci.len() == 4 || uci.len() == 5) {
            return Err(ChessError::InvalidMove(uci.to_owned()));
        }
        let promotion = uci.chars().nth(4).and_then(Piece::from_promotion);
        if uci.len() == 5 && promotion.is_none() {
            return Err(ChessError::InvalidPromotion);
        }
        Self::new(&uci[0..2], &uci[2..4], promotion)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub fen: String,
}

impl Position {
    pub fn from_fen(fen: impl Into<String>) -> Result<Self, ChessError> {
        let fen = fen.into();
        parse_position(&fen)?;
        Ok(Self { fen })
    }

    #[must_use]
    pub fn initial() -> Self {
        Self {
            fen: INITIAL_FEN.to_owned(),
        }
    }

    #[must_use]
    pub fn halfmove_clock(&self) -> Option<u16> {
        self.fen.split_whitespace().nth(4)?.parse().ok()
    }

    #[must_use]
    pub fn repetition_key(&self) -> Option<String> {
        let fields = self.fen.split_whitespace().take(4).collect::<Vec<_>>();
        (fields.len() == 4).then(|| fields.join(" "))
    }

    #[must_use]
    pub fn is_fifty_move_draw(&self) -> bool {
        self.halfmove_clock().is_some_and(|clock| clock >= 100)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionStatus {
    pub side_to_move: Color,
    pub in_check: bool,
    pub checkmate: bool,
    pub stalemate: bool,
    pub game_over: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceOnSquare {
    pub color: Color,
    pub piece: Piece,
}

#[derive(Debug, Error)]
pub enum ChessError {
    #[error("invalid FEN: {0}")]
    InvalidFen(String),
    #[error("invalid square: {0}")]
    InvalidSquare(String),
    #[error("invalid move: {0}")]
    InvalidMove(String),
    #[error("illegal move: {0}")]
    IllegalMove(String),
    #[error("invalid promotion piece")]
    InvalidPromotion,
}

pub trait ChessRulesEngine: Send + Sync {
    fn validate_position(&self, position: &Position) -> Result<(), ChessError>;
    fn legal_moves(&self, position: &Position) -> Result<Vec<ChessMove>, ChessError>;
    fn apply_move(
        &self,
        position: &Position,
        chess_move: &ChessMove,
    ) -> Result<Position, ChessError>;
    fn status(&self, position: &Position) -> Result<PositionStatus, ChessError>;
    fn piece_at(
        &self,
        position: &Position,
        square: &Square,
    ) -> Result<Option<PieceOnSquare>, ChessError>;
    fn material_balance(&self, position: &Position, perspective: Color) -> Result<i32, ChessError>;
}

#[derive(Debug, Default)]
pub struct ShakmatyRules;

impl ChessRulesEngine for ShakmatyRules {
    fn validate_position(&self, position: &Position) -> Result<(), ChessError> {
        parse_position(&position.fen).map(|_| ())
    }

    fn legal_moves(&self, position: &Position) -> Result<Vec<ChessMove>, ChessError> {
        let chess = parse_position(&position.fen)?;
        chess
            .legal_moves()
            .iter()
            .map(|chess_move| {
                ChessMove::from_uci(
                    &UciMove::from_move(*chess_move, CastlingMode::Standard).to_string(),
                )
            })
            .collect()
    }

    fn apply_move(
        &self,
        position: &Position,
        chess_move: &ChessMove,
    ) -> Result<Position, ChessError> {
        let chess = parse_position(&position.fen)?;
        let uci = UciMove::from_str(&chess_move.to_uci())
            .map_err(|_| ChessError::InvalidMove(chess_move.to_uci()))?;
        let legal_move = uci
            .to_move(&chess)
            .map_err(|_| ChessError::IllegalMove(chess_move.to_uci()))?;
        let next = chess
            .play(legal_move)
            .map_err(|_| ChessError::IllegalMove(chess_move.to_uci()))?;
        Ok(Position {
            fen: Fen::from_position(&next, EnPassantMode::Legal).to_string(),
        })
    }

    fn status(&self, position: &Position) -> Result<PositionStatus, ChessError> {
        let chess = parse_position(&position.fen)?;
        Ok(PositionStatus {
            side_to_move: chess.turn().into(),
            in_check: chess.is_check(),
            checkmate: chess.is_checkmate(),
            stalemate: chess.is_stalemate(),
            game_over: chess.is_game_over(),
        })
    }

    fn piece_at(
        &self,
        position: &Position,
        square: &Square,
    ) -> Result<Option<PieceOnSquare>, ChessError> {
        let chess = parse_position(&position.fen)?;
        let square = ShakSquare::from_str(square.as_str())
            .map_err(|_| ChessError::InvalidSquare(square.as_str().to_owned()))?;
        Ok(chess.board().piece_at(square).map(|piece| PieceOnSquare {
            color: piece.color.into(),
            piece: piece.role.into(),
        }))
    }

    fn material_balance(&self, position: &Position, perspective: Color) -> Result<i32, ChessError> {
        let chess = parse_position(&position.fen)?;
        let board = chess.board();
        let score = |color: ShakColor| {
            [
                (Role::Pawn, 100),
                (Role::Knight, 320),
                (Role::Bishop, 330),
                (Role::Rook, 500),
                (Role::Queen, 900),
            ]
            .into_iter()
            .map(|(role, value)| {
                (board.by_color(color) & board.by_role(role)).count() as i32 * value
            })
            .sum::<i32>()
        };
        let own = score(perspective.into());
        let opponent = score(perspective.opposite().into());
        Ok(own - opponent)
    }
}

fn parse_position(fen: &str) -> Result<Chess, ChessError> {
    let parsed = Fen::from_str(fen).map_err(|error| ChessError::InvalidFen(error.to_string()))?;
    parsed
        .into_position(CastlingMode::Standard)
        .map_err(|error| ChessError::InvalidFen(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rules() -> ShakmatyRules {
        ShakmatyRules
    }

    #[test]
    fn initial_position_has_twenty_moves() {
        assert_eq!(rules().legal_moves(&Position::initial()).unwrap().len(), 20);
    }

    #[test]
    fn applies_legal_move_and_rejects_illegal_move() {
        let next = rules()
            .apply_move(
                &Position::initial(),
                &ChessMove::new("e2", "e4", None).unwrap(),
            )
            .unwrap();
        assert!(next.fen.contains(" b KQkq"));

        let error = rules()
            .apply_move(
                &Position::initial(),
                &ChessMove::new("e2", "e5", None).unwrap(),
            )
            .unwrap_err();
        assert!(matches!(error, ChessError::IllegalMove(_)));
    }

    #[test]
    fn recognizes_checkmate_and_stalemate() {
        let mate = Position::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        assert!(rules().status(&mate).unwrap().checkmate);

        let stalemate = Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        assert!(rules().status(&stalemate).unwrap().stalemate);
    }

    #[test]
    fn supports_castling_en_passant_and_promotion() {
        let castle = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let moves = rules().legal_moves(&castle).unwrap();
        assert!(moves.iter().any(|chess_move| chess_move.to_uci() == "e1g1"));

        let en_passant = Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2").unwrap();
        let moves = rules().legal_moves(&en_passant).unwrap();
        assert!(moves.iter().any(|chess_move| chess_move.to_uci() == "e5d6"));

        let promotion = Position::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let moves = rules().legal_moves(&promotion).unwrap();
        assert!(
            moves
                .iter()
                .any(|chess_move| chess_move.to_uci() == "a7a8q")
        );
    }

    fn perft(rules: &ShakmatyRules, position: &Position, depth: u8) -> u64 {
        if depth == 0 {
            return 1;
        }
        rules
            .legal_moves(position)
            .unwrap()
            .into_iter()
            .map(|chess_move| {
                let next = rules.apply_move(position, &chess_move).unwrap();
                perft(rules, &next, depth - 1)
            })
            .sum()
    }

    #[test]
    fn standard_position_matches_perft_reference() {
        assert_eq!(perft(&rules(), &Position::initial(), 1), 20);
        assert_eq!(perft(&rules(), &Position::initial(), 2), 400);
        assert_eq!(perft(&rules(), &Position::initial(), 3), 8_902);
    }

    #[test]
    fn exposes_fifty_move_and_repetition_data() {
        let position = Position::from_fen("8/8/8/8/8/8/4K3/7k w - - 100 51").unwrap();
        assert!(position.is_fifty_move_draw());
        assert_eq!(
            position.repetition_key().as_deref(),
            Some("8/8/8/8/8/8/4K3/7k w - -")
        );
    }

    proptest! {
        #[test]
        fn every_reported_initial_move_produces_a_valid_position(index in 0usize..20) {
            let rules = rules();
            let position = Position::initial();
            let chess_move = rules.legal_moves(&position).unwrap()[index].clone();
            let next = rules.apply_move(&position, &chess_move).unwrap();
            prop_assert!(rules.validate_position(&next).is_ok());
        }
    }
}
