use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkyjoError {
    NotEnoughPlayers,
    TooManyPlayers,
    InvalidPosition(usize),
    SlotAlreadyCleared(usize),
    CannotFlipRevealed(usize),
    EmptyDeck,
    EmptyDiscardPile,
    GameAlreadyOver,
    InvalidAction(String),
}

impl fmt::Display for SkyjoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnoughPlayers => write!(f, "not enough players (minimum 2)"),
            Self::TooManyPlayers => write!(f, "too many players (maximum 8)"),
            Self::InvalidPosition(pos) => write!(f, "invalid board position: {pos}"),
            Self::SlotAlreadyCleared(pos) => write!(f, "slot already cleared at position: {pos}"),
            Self::CannotFlipRevealed(pos) => {
                write!(f, "cannot flip already revealed card at position: {pos}")
            }
            Self::EmptyDeck => write!(f, "deck is empty and cannot be reshuffled"),
            Self::EmptyDiscardPile => write!(f, "discard pile is empty"),
            Self::GameAlreadyOver => write!(f, "game is already over"),
            Self::InvalidAction(msg) => write!(f, "invalid action: {msg}"),
        }
    }
}

impl std::error::Error for SkyjoError {}

pub type Result<T> = std::result::Result<T, SkyjoError>;
