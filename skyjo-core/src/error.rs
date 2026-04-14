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
    NotYourTurn { expected: usize, got: usize },
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
            Self::NotYourTurn { expected, got } => {
                write!(f, "not your turn: expected player {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for SkyjoError {}

pub type Result<T> = std::result::Result<T, SkyjoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_variant_can_be_created() {
        let _ = SkyjoError::NotEnoughPlayers;
        let _ = SkyjoError::TooManyPlayers;
        let _ = SkyjoError::InvalidPosition(5);
        let _ = SkyjoError::SlotAlreadyCleared(3);
        let _ = SkyjoError::CannotFlipRevealed(7);
        let _ = SkyjoError::EmptyDeck;
        let _ = SkyjoError::EmptyDiscardPile;
        let _ = SkyjoError::GameAlreadyOver;
        let _ = SkyjoError::InvalidAction("test".into());
        let _ = SkyjoError::NotYourTurn {
            expected: 0,
            got: 1,
        };
    }

    #[test]
    fn display_messages_are_meaningful() {
        assert_eq!(
            SkyjoError::NotEnoughPlayers.to_string(),
            "not enough players (minimum 2)"
        );
        assert_eq!(
            SkyjoError::TooManyPlayers.to_string(),
            "too many players (maximum 8)"
        );
        assert_eq!(
            SkyjoError::InvalidPosition(5).to_string(),
            "invalid board position: 5"
        );
        assert_eq!(
            SkyjoError::SlotAlreadyCleared(3).to_string(),
            "slot already cleared at position: 3"
        );
        assert_eq!(
            SkyjoError::CannotFlipRevealed(7).to_string(),
            "cannot flip already revealed card at position: 7"
        );
        assert_eq!(
            SkyjoError::EmptyDeck.to_string(),
            "deck is empty and cannot be reshuffled"
        );
        assert_eq!(
            SkyjoError::EmptyDiscardPile.to_string(),
            "discard pile is empty"
        );
        assert_eq!(
            SkyjoError::GameAlreadyOver.to_string(),
            "game is already over"
        );
        assert_eq!(
            SkyjoError::InvalidAction("bad move".into()).to_string(),
            "invalid action: bad move"
        );
        assert_eq!(
            SkyjoError::NotYourTurn {
                expected: 2,
                got: 5
            }
            .to_string(),
            "not your turn: expected player 2, got 5"
        );
    }

    #[test]
    fn display_messages_are_distinct() {
        let variants: Vec<String> = vec![
            SkyjoError::NotEnoughPlayers.to_string(),
            SkyjoError::TooManyPlayers.to_string(),
            SkyjoError::InvalidPosition(0).to_string(),
            SkyjoError::SlotAlreadyCleared(0).to_string(),
            SkyjoError::CannotFlipRevealed(0).to_string(),
            SkyjoError::EmptyDeck.to_string(),
            SkyjoError::EmptyDiscardPile.to_string(),
            SkyjoError::GameAlreadyOver.to_string(),
            SkyjoError::InvalidAction("x".into()).to_string(),
            SkyjoError::NotYourTurn {
                expected: 0,
                got: 1,
            }
            .to_string(),
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "Variants {i} and {j} have the same display message");
                }
            }
        }
    }

    #[test]
    fn implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(SkyjoError::EmptyDeck);
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn equality_and_clone() {
        let a = SkyjoError::InvalidPosition(3);
        let b = a.clone();
        assert_eq!(a, b);
        assert_ne!(SkyjoError::EmptyDeck, SkyjoError::EmptyDiscardPile);
    }
}
