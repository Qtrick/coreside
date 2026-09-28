//! Trusted chess rules capability for the interactive rules engine.
//!
//! The model may describe a chess application and propose moves, but only this
//! module decides legality. Behavior follows FIDE rules as implemented by chess.js:
//! legal move generation with king safety, castling (rights, through-check and
//! in-check rejection), en passant, promotion, checkmate, stalemate, fifty-move
//! rule, threefold repetition and insufficient material.

use serde_json::{json, Value};

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const MAX_HISTORY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    fn other(self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Color::White => "white",
            Color::Black => "black",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub color: Color,
    pub kind: Kind,
}

impl Piece {
    fn to_char(self) -> char {
        let c = match self.kind {
            Kind::Pawn => 'p',
            Kind::Knight => 'n',
            Kind::Bishop => 'b',
            Kind::Rook => 'r',
            Kind::Queen => 'q',
            Kind::King => 'k',
        };
        if self.color == Color::White {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }

    fn from_char(c: char) -> Option<Self> {
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        let kind = match c.to_ascii_lowercase() {
            'p' => Kind::Pawn,
            'n' => Kind::Knight,
            'b' => Kind::Bishop,
            'r' => Kind::Rook,
            'q' => Kind::Queen,
            'k' => Kind::King,
            _ => return None,
        };
        Some(Piece { color, kind })
    }
}

const WK: u8 = 1;
const WQ: u8 = 2;
const BK: u8 = 4;
const BQ: u8 = 8;

/// Square index: 0 = a1, 7 = h1, 56 = a8, 63 = h8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    board: [Option<Piece>; 64],
    side: Color,
    castling: u8,
    ep: Option<u8>,
    halfmove: u32,
    fullmove: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub promotion: Option<Kind>,
}

impl Move {
    pub fn uci(&self) -> String {
        let mut s = format!("{}{}", square_name(self.from), square_name(self.to));
        if let Some(p) = self.promotion {
            s.push(
                Piece {
                    color: Color::Black,
                    kind: p,
                }
                .to_char(),
            );
        }
        s
    }
}

pub fn square_name(sq: u8) -> String {
    let file = (b'a' + sq % 8) as char;
    let rank = (b'1' + sq / 8) as char;
    format!("{file}{rank}")
}

pub fn parse_square(s: &str) -> Result<u8, String> {
    let b = s.as_bytes();
    if b.len() != 2 || !(b'a'..=b'h').contains(&b[0]) || !(b'1'..=b'8').contains(&b[1]) {
        return Err(format!("Invalid square '{s}'"));
    }
    Ok((b[1] - b'1') * 8 + (b[0] - b'a'))
}

fn offset(sq: u8, df: i8, dr: i8) -> Option<u8> {
    let f = (sq % 8) as i8 + df;
    let r = (sq / 8) as i8 + dr;
    if (0..8).contains(&f) && (0..8).contains(&r) {
        Some((r * 8 + f) as u8)
    } else {
        None
    }
}

const KNIGHT_D: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const KING_D: [(i8, i8); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];
const ROOK_D: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_D: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

impl Position {
    pub fn start() -> Self {
        Self::from_fen(START_FEN).expect("start FEN is valid")
    }

    pub fn from_fen(fen: &str) -> Result<Self, String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err("FEN must have 6 fields".into());
        }
        let mut board = [None; 64];
        let ranks: Vec<&str> = parts[0].split('/').collect();
        if ranks.len() != 8 {
            return Err("FEN placement must have 8 ranks".into());
        }
        for (i, rank_str) in ranks.iter().enumerate() {
            let rank = 7 - i as u8;
            let mut file = 0u8;
            for c in rank_str.chars() {
                if let Some(d) = c.to_digit(10) {
                    if !(1..=8).contains(&d) {
                        return Err("Invalid FEN digit".into());
                    }
                    file += d as u8;
                } else {
                    let p = Piece::from_char(c).ok_or("Invalid FEN piece")?;
                    if file >= 8 {
                        return Err("FEN rank overflow".into());
                    }
                    board[(rank * 8 + file) as usize] = Some(p);
                    file += 1;
                }
                if file > 8 {
                    return Err("FEN rank overflow".into());
                }
            }
            if file != 8 {
                return Err("FEN rank must describe 8 files".into());
            }
        }
        let side = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err("Invalid FEN side to move".into()),
        };
        let mut castling = 0;
        if parts[2] != "-" {
            for c in parts[2].chars() {
                castling |= match c {
                    'K' => WK,
                    'Q' => WQ,
                    'k' => BK,
                    'q' => BQ,
                    _ => return Err("Invalid FEN castling".into()),
                };
            }
        }
        let ep = if parts[3] == "-" {
            None
        } else {
            let sq = parse_square(parts[3])?;
            if sq / 8 != 2 && sq / 8 != 5 {
                return Err("Invalid FEN en passant rank".into());
            }
            Some(sq)
        };
        let halfmove = parts[4].parse().map_err(|_| "Invalid FEN halfmove")?;
        let fullmove: u32 = parts[5].parse().map_err(|_| "Invalid FEN fullmove")?;
        if fullmove == 0 {
            return Err("Invalid FEN fullmove".into());
        }
        let pos = Position {
            board,
            side,
            castling,
            ep,
            halfmove,
            fullmove,
        };
        for color in [Color::White, Color::Black] {
            let kings = pos
                .board
                .iter()
                .filter(|p| {
                    **p == Some(Piece {
                        color,
                        kind: Kind::King,
                    })
                })
                .count();
            if kings != 1 {
                return Err("FEN must contain exactly one king per side".into());
            }
        }
        if pos.is_attacked(pos.king_square(side.other()), side) {
            return Err("FEN side not to move is in check".into());
        }
        Ok(pos)
    }

    fn placement(&self) -> String {
        let mut s = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match self.board[rank * 8 + file] {
                    Some(p) => {
                        if empty > 0 {
                            s.push_str(&empty.to_string());
                            empty = 0;
                        }
                        s.push(p.to_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                s.push_str(&empty.to_string());
            }
            if rank > 0 {
                s.push('/');
            }
        }
        s
    }

    fn castling_str(&self) -> String {
        let mut s = String::new();
        for (bit, c) in [(WK, 'K'), (WQ, 'Q'), (BK, 'k'), (BQ, 'q')] {
            if self.castling & bit != 0 {
                s.push(c);
            }
        }
        if s.is_empty() {
            s.push('-');
        }
        s
    }

    pub fn to_fen(&self) -> String {
        format!(
            "{} {} {} {} {} {}",
            self.placement(),
            if self.side == Color::White { "w" } else { "b" },
            self.castling_str(),
            self.ep.map(square_name).unwrap_or_else(|| "-".into()),
            self.halfmove,
            self.fullmove
        )
    }

    /// Repetition key: placement, side, castling, and en passant only when an
    /// en passant capture is actually legal (matching chess.js semantics).
    pub fn repetition_key(&self) -> String {
        let ep = match self.ep {
            Some(sq)
                if self
                    .legal_moves()
                    .iter()
                    .any(|m| m.to == sq && self.is_ep(m)) =>
            {
                square_name(sq)
            }
            _ => "-".into(),
        };
        format!(
            "{} {} {} {}",
            self.placement(),
            if self.side == Color::White { "w" } else { "b" },
            self.castling_str(),
            ep
        )
    }

    pub fn side_to_move(&self) -> Color {
        self.side
    }

    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove
    }

    fn is_ep(&self, m: &Move) -> bool {
        Some(m.to) == self.ep
            && matches!(self.board[m.from as usize], Some(p) if p.kind == Kind::Pawn)
            && m.from % 8 != m.to % 8
            && self.board[m.to as usize].is_none()
    }

    fn king_square(&self, color: Color) -> u8 {
        self.board
            .iter()
            .position(|p| {
                *p == Some(Piece {
                    color,
                    kind: Kind::King,
                })
            })
            .map(|i| i as u8)
            .unwrap_or(0)
    }

    /// True when `sq` is attacked by any piece of color `by`.
    pub fn is_attacked(&self, sq: u8, by: Color) -> bool {
        let pawn_dr: i8 = if by == Color::White { -1 } else { 1 };
        for df in [-1, 1] {
            if let Some(s) = offset(sq, df, pawn_dr) {
                if self.board[s as usize]
                    == Some(Piece {
                        color: by,
                        kind: Kind::Pawn,
                    })
                {
                    return true;
                }
            }
        }
        for (df, dr) in KNIGHT_D {
            if let Some(s) = offset(sq, df, dr) {
                if self.board[s as usize]
                    == Some(Piece {
                        color: by,
                        kind: Kind::Knight,
                    })
                {
                    return true;
                }
            }
        }
        for (df, dr) in KING_D {
            if let Some(s) = offset(sq, df, dr) {
                if self.board[s as usize]
                    == Some(Piece {
                        color: by,
                        kind: Kind::King,
                    })
                {
                    return true;
                }
            }
        }
        for (dirs, kinds) in [
            (ROOK_D, [Kind::Rook, Kind::Queen]),
            (BISHOP_D, [Kind::Bishop, Kind::Queen]),
        ] {
            for (df, dr) in dirs {
                let mut cur = sq;
                while let Some(s) = offset(cur, df, dr) {
                    if let Some(p) = self.board[s as usize] {
                        if p.color == by && kinds.contains(&p.kind) {
                            return true;
                        }
                        break;
                    }
                    cur = s;
                }
            }
        }
        false
    }

    pub fn in_check(&self) -> bool {
        self.is_attacked(self.king_square(self.side), self.side.other())
    }

    fn pseudo_legal(&self) -> Vec<Move> {
        let mut moves = Vec::with_capacity(48);
        let us = self.side;
        for from in 0..64u8 {
            let Some(p) = self.board[from as usize] else {
                continue;
            };
            if p.color != us {
                continue;
            }
            match p.kind {
                Kind::Pawn => self.pawn_moves(from, &mut moves),
                Kind::Knight => self.step_moves(from, &KNIGHT_D, &mut moves),
                Kind::King => {
                    self.step_moves(from, &KING_D, &mut moves);
                    self.castle_moves(from, &mut moves);
                }
                Kind::Bishop => self.slide_moves(from, &BISHOP_D, &mut moves),
                Kind::Rook => self.slide_moves(from, &ROOK_D, &mut moves),
                Kind::Queen => {
                    self.slide_moves(from, &BISHOP_D, &mut moves);
                    self.slide_moves(from, &ROOK_D, &mut moves);
                }
            }
        }
        moves
    }

    fn push_pawn(&self, from: u8, to: u8, moves: &mut Vec<Move>) {
        let last_rank = if self.side == Color::White { 7 } else { 0 };
        if to / 8 == last_rank {
            for k in [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight] {
                moves.push(Move {
                    from,
                    to,
                    promotion: Some(k),
                });
            }
        } else {
            moves.push(Move {
                from,
                to,
                promotion: None,
            });
        }
    }

    fn pawn_moves(&self, from: u8, moves: &mut Vec<Move>) {
        let (dr, start_rank) = if self.side == Color::White {
            (1, 1)
        } else {
            (-1, 6)
        };
        if let Some(one) = offset(from, 0, dr) {
            if self.board[one as usize].is_none() {
                self.push_pawn(from, one, moves);
                if from / 8 == start_rank {
                    if let Some(two) = offset(from, 0, 2 * dr) {
                        if self.board[two as usize].is_none() {
                            moves.push(Move {
                                from,
                                to: two,
                                promotion: None,
                            });
                        }
                    }
                }
            }
        }
        for df in [-1, 1] {
            if let Some(t) = offset(from, df, dr) {
                match self.board[t as usize] {
                    Some(q) if q.color != self.side => self.push_pawn(from, t, moves),
                    None if Some(t) == self.ep => moves.push(Move {
                        from,
                        to: t,
                        promotion: None,
                    }),
                    _ => {}
                }
            }
        }
    }

    fn step_moves(&self, from: u8, dirs: &[(i8, i8)], moves: &mut Vec<Move>) {
        for &(df, dr) in dirs {
            if let Some(t) = offset(from, df, dr) {
                match self.board[t as usize] {
                    Some(q) if q.color == self.side => {}
                    _ => moves.push(Move {
                        from,
                        to: t,
                        promotion: None,
                    }),
                }
            }
        }
    }

    fn slide_moves(&self, from: u8, dirs: &[(i8, i8)], moves: &mut Vec<Move>) {
        for &(df, dr) in dirs {
            let mut cur = from;
            while let Some(t) = offset(cur, df, dr) {
                match self.board[t as usize] {
                    Some(q) => {
                        if q.color != self.side {
                            moves.push(Move {
                                from,
                                to: t,
                                promotion: None,
                            });
                        }
                        break;
                    }
                    None => moves.push(Move {
                        from,
                        to: t,
                        promotion: None,
                    }),
                }
                cur = t;
            }
        }
    }

    fn castle_moves(&self, from: u8, moves: &mut Vec<Move>) {
        let them = self.side.other();
        let (home, k_bit, q_bit) = if self.side == Color::White {
            (4u8, WK, WQ)
        } else {
            (60u8, BK, BQ)
        };
        if from != home || self.is_attacked(home, them) {
            return;
        }
        let rook = Some(Piece {
            color: self.side,
            kind: Kind::Rook,
        });
        if self.castling & k_bit != 0
            && self.board[(home + 3) as usize] == rook
            && self.board[(home + 1) as usize].is_none()
            && self.board[(home + 2) as usize].is_none()
            && !self.is_attacked(home + 1, them)
            && !self.is_attacked(home + 2, them)
        {
            moves.push(Move {
                from,
                to: home + 2,
                promotion: None,
            });
        }
        if self.castling & q_bit != 0
            && self.board[(home - 4) as usize] == rook
            && self.board[(home - 1) as usize].is_none()
            && self.board[(home - 2) as usize].is_none()
            && self.board[(home - 3) as usize].is_none()
            && !self.is_attacked(home - 1, them)
            && !self.is_attacked(home - 2, them)
        {
            moves.push(Move {
                from,
                to: home - 2,
                promotion: None,
            });
        }
    }

    /// Apply a pseudo-legal move without legality checks.
    fn make(&self, m: &Move) -> Position {
        let mut n = *self;
        let piece = n.board[m.from as usize].expect("move from occupied square");
        let captured = n.board[m.to as usize];
        let is_ep = self.is_ep(m);
        n.board[m.from as usize] = None;
        n.board[m.to as usize] = Some(match m.promotion {
            Some(kind) => Piece {
                color: piece.color,
                kind,
            },
            None => piece,
        });
        if is_ep {
            let cap_sq = if piece.color == Color::White {
                m.to - 8
            } else {
                m.to + 8
            };
            n.board[cap_sq as usize] = None;
        }
        if piece.kind == Kind::King && (m.to as i8 - m.from as i8).abs() == 2 {
            let (rook_from, rook_to) = if m.to > m.from {
                (m.from + 3, m.from + 1)
            } else {
                (m.from - 4, m.from - 1)
            };
            n.board[rook_to as usize] = n.board[rook_from as usize].take();
        }
        for sq in [m.from, m.to] {
            n.castling &= match sq {
                0 => !WQ,
                7 => !WK,
                4 => !(WK | WQ),
                56 => !BQ,
                63 => !BK,
                60 => !(BK | BQ),
                _ => 0xff,
            };
        }
        n.ep = None;
        if piece.kind == Kind::Pawn && (m.to as i8 - m.from as i8).abs() == 16 {
            n.ep = Some((m.from + m.to) / 2);
        }
        if piece.kind == Kind::Pawn || captured.is_some() || is_ep {
            n.halfmove = 0;
        } else {
            n.halfmove += 1;
        }
        if self.side == Color::Black {
            n.fullmove += 1;
        }
        n.side = self.side.other();
        n
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        self.pseudo_legal()
            .into_iter()
            .filter(|m| {
                let n = self.make(m);
                !n.is_attacked(n.king_square(self.side), n.side)
            })
            .collect()
    }

    /// Apply a move only if it is legal. Promotion moves require an explicit piece.
    pub fn play(&self, from: u8, to: u8, promotion: Option<Kind>) -> Result<Position, String> {
        let legal = self.legal_moves();
        let candidate = Move {
            from,
            to,
            promotion,
        };
        if legal.contains(&candidate) {
            return Ok(self.make(&candidate));
        }
        if promotion.is_none() && legal.iter().any(|m| m.from == from && m.to == to) {
            return Err("Promotion piece required".into());
        }
        Err(format!(
            "Illegal move {}{}",
            square_name(from),
            square_name(to)
        ))
    }

    pub fn insufficient_material(&self) -> bool {
        let mut minors = Vec::new();
        for (sq, p) in self.board.iter().enumerate() {
            match p {
                None => {}
                Some(Piece {
                    kind: Kind::King, ..
                }) => {}
                Some(Piece {
                    kind: Kind::Knight | Kind::Bishop,
                    ..
                }) => minors.push((sq, p.unwrap().kind)),
                Some(_) => return false,
            }
        }
        match minors.len() {
            0 | 1 => true,
            _ => {
                minors.iter().all(|(_, k)| *k == Kind::Bishop) && {
                    let color = |sq: usize| (sq / 8 + sq % 8) % 2;
                    let first = color(minors[0].0);
                    minors.iter().all(|(sq, _)| color(*sq) == first)
                }
            }
        }
    }

    pub fn perft(&self, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let moves = self.legal_moves();
        if depth == 1 {
            return moves.len() as u64;
        }
        moves.iter().map(|m| self.make(m).perft(depth - 1)).sum()
    }
}

fn parse_promotion(v: &Value) -> Result<Option<Kind>, String> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "" => Ok(None),
            "q" | "queen" => Ok(Some(Kind::Queen)),
            "r" | "rook" => Ok(Some(Kind::Rook)),
            "b" | "bishop" => Ok(Some(Kind::Bishop)),
            "n" | "knight" => Ok(Some(Kind::Knight)),
            other => Err(format!("Invalid promotion piece '{other}'")),
        },
        _ => Err("Promotion must be a string".into()),
    }
}

/// Outcome of the position for the side to move.
pub fn game_status(pos: &Position, history: &[String]) -> (&'static str, Option<&'static str>) {
    let legal = pos.legal_moves();
    if legal.is_empty() {
        if pos.in_check() {
            return ("checkmate", Some(pos.side.other().name()));
        }
        return ("stalemate", None);
    }
    if pos.halfmove >= 100 {
        return ("draw_fifty_move", None);
    }
    if pos.insufficient_material() {
        return ("draw_insufficient_material", None);
    }
    let key = pos.repetition_key();
    if history.iter().filter(|k| **k == key).count() >= 3 {
        return ("draw_threefold", None);
    }
    ("playing", None)
}

/// Render the trusted, authoritative chess object stored in interactive state.
pub fn state_object(pos: &Position, history: Vec<String>, moves: Vec<String>) -> Value {
    let (status, winner) = game_status(pos, &history);
    let board: Vec<Value> = (0..8)
        .rev()
        .map(|rank| {
            Value::Array(
                (0..8)
                    .map(|file| {
                        json!(pos.board[rank * 8 + file]
                            .map(|p| p.to_char().to_string())
                            .unwrap_or_default())
                    })
                    .collect(),
            )
        })
        .collect();
    let legal: Vec<String> = if status == "playing" {
        pos.legal_moves().iter().map(Move::uci).collect()
    } else {
        Vec::new()
    };
    json!({
        "fen": pos.to_fen(),
        "board": board,
        "sideToMove": pos.side.name(),
        "inCheck": pos.in_check(),
        "legalMoves": legal,
        "moves": moves,
        "positionHistory": history,
        "status": status,
        "winner": winner,
    })
}

pub fn initial_state_object(fen: &str) -> Result<Value, String> {
    let pos = Position::from_fen(fen)?;
    Ok(state_object(&pos, vec![pos.repetition_key()], Vec::new()))
}

fn string_list(obj: &Value, key: &str) -> Result<Vec<String>, String> {
    obj.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("chess state '{key}' missing"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("chess state '{key}' corrupted"))
        })
        .collect()
}

/// Apply a move to the persisted chess object. Fails closed on corrupt state,
/// illegal moves, or moves after the game has ended.
pub fn apply_move(chess: &Value, from: &str, to: &str, promotion: &Value) -> Result<Value, String> {
    let fen = chess
        .get("fen")
        .and_then(Value::as_str)
        .ok_or("chess state 'fen' missing")?;
    let pos = Position::from_fen(fen)?;
    let mut history = string_list(chess, "positionHistory")?;
    let mut moves = string_list(chess, "moves")?;
    if game_status(&pos, &history).0 != "playing" {
        return Err("Game is over".into());
    }
    if history.len() >= MAX_HISTORY {
        return Err("Chess history limit reached".into());
    }
    let promo = parse_promotion(promotion)?;
    let next = pos.play(parse_square(from)?, parse_square(to)?, promo)?;
    moves.push(
        Move {
            from: parse_square(from)?,
            to: parse_square(to)?,
            promotion: promo,
        }
        .uci(),
    );
    history.push(next.repetition_key());
    Ok(state_object(&next, history, moves))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(s: &str) -> u8 {
        parse_square(s).unwrap()
    }

    fn play_uci(obj: Value, moves: &[&str]) -> Result<Value, String> {
        let mut cur = obj;
        for m in moves {
            let promo = if m.len() == 5 {
                json!(&m[4..5])
            } else {
                Value::Null
            };
            cur = apply_move(&cur, &m[0..2], &m[2..4], &promo)?;
        }
        Ok(cur)
    }

    fn from_fen(fen: &str) -> Value {
        initial_state_object(fen).unwrap()
    }

    #[test]
    fn perft_start_position() {
        let p = Position::start();
        assert_eq!(p.perft(1), 20);
        assert_eq!(p.perft(2), 400);
        assert_eq!(p.perft(3), 8902);
        assert_eq!(p.perft(4), 197_281);
    }

    #[test]
    fn perft_kiwipete_castling_ep_promotion() {
        let p = Position::from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        )
        .unwrap();
        assert_eq!(p.perft(1), 48);
        assert_eq!(p.perft(2), 2039);
        assert_eq!(p.perft(3), 97_862);
    }

    #[test]
    fn perft_reference_positions_3_4_5() {
        let p3 = Position::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(p3.perft(4), 43_238);
        let p4 =
            Position::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1")
                .unwrap();
        assert_eq!(p4.perft(3), 9467);
        let p5 = Position::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8")
            .unwrap();
        assert_eq!(p5.perft(3), 62_379);
    }

    #[test]
    fn initial_position_and_fen_roundtrip() {
        let obj = from_fen(START_FEN);
        assert_eq!(obj["fen"], START_FEN);
        assert_eq!(obj["sideToMove"], "white");
        assert_eq!(obj["legalMoves"].as_array().unwrap().len(), 20);
        assert_eq!(obj["board"][0][4], "k");
        assert_eq!(obj["board"][7][4], "K");
    }

    #[test]
    fn pawn_single_double_and_illegal_moves() {
        let start = from_fen(START_FEN);
        assert!(play_uci(start.clone(), &["e2e3"]).is_ok());
        let after = play_uci(start.clone(), &["e2e4"]).unwrap();
        assert!(after["fen"].as_str().unwrap().contains(" b KQkq e3 0 1"));
        assert!(play_uci(start.clone(), &["e2e5"]).is_err(), "triple push");
        assert!(
            play_uci(start.clone(), &["e2d3"]).is_err(),
            "diagonal w/o capture"
        );
        // Double move blocked by piece on e3.
        let blocked = from_fen("rnbqkbnr/pppppppp/8/8/8/4N3/PPPPPPPP/RNBQKB1R w KQkq - 0 1");
        assert!(play_uci(blocked, &["e2e4"]).is_err());
        // Double move from non-start rank.
        let moved = play_uci(start, &["e2e3", "a7a6"]).unwrap();
        assert!(play_uci(moved, &["e3e5"]).is_err());
    }

    #[test]
    fn piece_movement_rules() {
        let start = from_fen(START_FEN);
        assert!(play_uci(start.clone(), &["g1f3"]).is_ok(), "knight");
        assert!(
            play_uci(start.clone(), &["g1g3"]).is_err(),
            "knight not straight"
        );
        assert!(
            play_uci(start.clone(), &["f1c4"]).is_err(),
            "bishop blocked"
        );
        assert!(
            play_uci(start.clone(), &["e2e4", "e7e5", "f1c4"]).is_ok(),
            "bishop"
        );
        assert!(play_uci(start.clone(), &["a1a3"]).is_err(), "rook blocked");
        assert!(
            play_uci(start.clone(), &["a2a4", "a7a6", "a1a3"]).is_ok(),
            "rook"
        );
        assert!(
            play_uci(start.clone(), &["e2e4", "e7e5", "d1h5"]).is_ok(),
            "queen"
        );
        assert!(
            play_uci(start.clone(), &["e2e4", "e7e5", "e1e2"]).is_ok(),
            "king"
        );
        assert!(
            play_uci(start, &["e2e4", "e7e5", "e1e3"]).is_err(),
            "king two squares"
        );
    }

    #[test]
    fn capture_and_illegal_own_capture() {
        let obj = play_uci(from_fen(START_FEN), &["e2e4", "d7d5"]).unwrap();
        let cap = play_uci(obj.clone(), &["e4d5"]).unwrap();
        assert!(cap["fen"].as_str().unwrap().ends_with(" 0 2"));
        assert!(
            play_uci(obj, &["d1d2"]).is_err(),
            "cannot capture own piece"
        );
    }

    #[test]
    fn check_and_pinned_piece_cannot_expose_king() {
        let obj = play_uci(from_fen(START_FEN), &["e2e4", "f7f6", "d1h5"]).unwrap();
        assert_eq!(obj["inCheck"], true);
        // Only g7g6 blocks; e.g. a7a6 leaves the king in check.
        assert!(play_uci(obj.clone(), &["a7a6"]).is_err());
        assert!(play_uci(obj, &["g7g6"]).is_ok());
        // Pinned knight on e7 (bishop b4? use rook pin on file).
        let pinned = from_fen("4k3/4n3/8/8/8/8/8/4R1K1 b - - 0 1");
        assert!(play_uci(pinned, &["e7c6"]).is_err());
    }

    #[test]
    fn checkmate_and_stalemate() {
        let mate = play_uci(from_fen(START_FEN), &["f2f3", "e7e5", "g2g4", "d8h4"]).unwrap();
        assert_eq!(mate["status"], "checkmate");
        assert_eq!(mate["winner"], "black");
        assert_eq!(mate["legalMoves"].as_array().unwrap().len(), 0);
        assert!(play_uci(mate, &["a2a3"]).unwrap_err().contains("over"));

        let stale = play_uci(from_fen("7k/8/6Q1/8/8/8/8/K7 w - - 0 1"), &["g6f7"]).unwrap();
        assert_eq!(stale["status"], "stalemate");
        assert!(stale["winner"].is_null());
    }

    #[test]
    fn castling_both_sides_and_rights() {
        let fen = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1";
        let ks = play_uci(from_fen(fen), &["e1g1"]).unwrap();
        assert!(ks["fen"]
            .as_str()
            .unwrap()
            .starts_with("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R4RK1 b kq"));
        let qs = play_uci(from_fen(fen), &["e1c1"]).unwrap();
        assert!(qs["fen"]
            .as_str()
            .unwrap()
            .starts_with("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/2KR3R b kq"));

        // King move loses both rights.
        let km = play_uci(from_fen(fen), &["e1f1", "a7a6", "f1e1", "a6a5"]).unwrap();
        assert!(km["fen"].as_str().unwrap().contains(" w kq "));
        assert!(play_uci(km, &["e1g1"]).is_err());

        // Rook move loses that side's right only.
        let rm = play_uci(from_fen(fen), &["h1g1", "a7a6", "g1h1", "a6a5"]).unwrap();
        assert!(rm["fen"].as_str().unwrap().contains(" w Qkq "));
        assert!(play_uci(rm.clone(), &["e1g1"]).is_err());
        assert!(play_uci(rm, &["e1c1"]).is_ok());

        // Captured rook removes the opponent's right.
        let cap = from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
        let after = play_uci(cap, &["a1a8"]).unwrap();
        assert!(after["fen"].as_str().unwrap().contains(" b Kk "));
    }

    #[test]
    fn castling_through_or_out_of_check_rejected() {
        // f1 attacked by bishop on c4.
        let through = from_fen("4k3/8/8/8/2b5/8/8/4K2R w K - 0 1");
        assert!(play_uci(through, &["e1g1"]).is_err());
        // King currently in check from rook on e8.
        let in_check = from_fen("4r1k1/8/8/8/8/8/8/R3K2R w KQ - 0 1");
        assert!(play_uci(in_check.clone(), &["e1g1"]).is_err());
        assert!(play_uci(in_check, &["e1c1"]).is_err());
        // Queenside b1 may be attacked; only c1/d1 matter.
        let b1 = from_fen("1r2k3/8/8/8/8/8/8/R3K3 w Q - 0 1");
        assert!(play_uci(b1, &["e1c1"]).is_ok());
    }

    #[test]
    fn en_passant_capture_and_expiry() {
        let obj = play_uci(from_fen(START_FEN), &["e2e4", "a7a6", "e4e5", "d7d5"]).unwrap();
        let ep = play_uci(obj.clone(), &["e5d6"]).unwrap();
        let fen = ep["fen"].as_str().unwrap();
        assert!(fen.starts_with("rnbqkbnr/1pp1pppp/p2P4/8/8/8/PPPP1PPP/RNBQKBNR b"));
        // Delayed en passant is illegal.
        let late = play_uci(obj, &["a2a3", "a6a5"]).unwrap();
        assert!(play_uci(late, &["e5d6"]).is_err());
    }

    #[test]
    fn promotion_to_each_piece_and_required_choice() {
        let fen = "8/P6k/8/8/8/8/8/K7 w - - 0 1";
        for (p, c) in [("q", 'Q'), ("r", 'R'), ("b", 'B'), ("n", 'N')] {
            let obj = apply_move(&from_fen(fen), "a7", "a8", &json!(p)).unwrap();
            assert_eq!(obj["board"][0][0], c.to_string());
        }
        let err = apply_move(&from_fen(fen), "a7", "a8", &Value::Null).unwrap_err();
        assert!(err.contains("Promotion"));
        assert!(apply_move(&from_fen(fen), "a7", "a8", &json!("k")).is_err());
        assert!(apply_move(&from_fen(START_FEN), "e2", "e4", &json!("q")).is_err());
    }

    #[test]
    fn fifty_move_rule() {
        let obj = from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 99 60");
        let after = play_uci(obj, &["a1a2"]).unwrap();
        assert_eq!(after["status"], "draw_fifty_move");
        // A pawn move resets the clock.
        let reset = play_uci(from_fen("4k3/8/8/8/8/8/P7/4K3 w - - 99 60"), &["a2a3"]).unwrap();
        assert_eq!(reset["status"], "playing");
    }

    #[test]
    fn threefold_repetition() {
        let obj = play_uci(
            from_fen(START_FEN),
            &["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1"],
        )
        .unwrap();
        assert_eq!(obj["status"], "playing");
        let drawn = play_uci(obj, &["f6g8"]).unwrap();
        assert_eq!(drawn["status"], "draw_threefold");
    }

    #[test]
    fn insufficient_material_cases() {
        let ins = |fen: &str| Position::from_fen(fen).unwrap().insufficient_material();
        assert!(ins("4k3/8/8/8/8/8/8/4K3 w - - 0 1"));
        assert!(ins("4k3/8/8/8/8/8/8/4KN2 w - - 0 1"));
        assert!(ins("4k3/8/8/8/8/8/8/4KB2 w - - 0 1"));
        assert!(
            ins("4kb2/8/8/8/8/8/8/2B1K3 w - - 0 1"),
            "same-colored bishops"
        );
        assert!(
            !ins("4k1b1/8/8/8/8/8/8/2B1K3 w - - 0 1"),
            "opposite bishops"
        );
        assert!(!ins("4k3/8/8/8/8/8/8/3NKN2 w - - 0 1"), "two knights");
        assert!(!ins("4k3/8/8/8/8/8/P7/4K3 w - - 0 1"));
        let obj = play_uci(from_fen("4k3/8/8/8/8/8/3r4/4K3 w - - 0 1"), &["e1d2"]).unwrap();
        assert_eq!(obj["status"], "draw_insufficient_material");
    }

    #[test]
    fn move_history_and_corrupt_state_fail_closed() {
        let obj = play_uci(from_fen(START_FEN), &["e2e4", "e7e5"]).unwrap();
        assert_eq!(obj["moves"], json!(["e2e4", "e7e5"]));
        assert_eq!(obj["positionHistory"].as_array().unwrap().len(), 3);
        let mut bad = obj.clone();
        bad["fen"] = json!("not a fen");
        assert!(apply_move(&bad, "g1", "f3", &Value::Null).is_err());
        let mut bad_hist = obj;
        bad_hist["positionHistory"] = json!([1, 2]);
        assert!(apply_move(&bad_hist, "g1", "f3", &Value::Null).is_err());
        assert!(
            Position::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").is_err(),
            "no kings"
        );
        assert!(parse_square("i9").is_err());
        let _ = sq("a1");
    }
}
