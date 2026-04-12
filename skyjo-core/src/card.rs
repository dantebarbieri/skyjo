use serde::{Deserialize, Serialize};

pub type CardValue = i8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slot {
    Hidden(CardValue),
    Revealed(CardValue),
    Cleared,
}

impl Slot {
    pub fn value(&self) -> Option<CardValue> {
        match self {
            Slot::Hidden(v) | Slot::Revealed(v) => Some(*v),
            Slot::Cleared => None,
        }
    }

    pub fn visible_value(&self) -> Option<CardValue> {
        match self {
            Slot::Revealed(v) => Some(*v),
            _ => None,
        }
    }

    pub fn is_hidden(&self) -> bool {
        matches!(self, Slot::Hidden(_))
    }

    pub fn is_revealed(&self) -> bool {
        matches!(self, Slot::Revealed(_))
    }

    pub fn is_cleared(&self) -> bool {
        matches!(self, Slot::Cleared)
    }
}

/// Build the standard 150-card Skyjo deck.
pub fn standard_deck() -> Vec<CardValue> {
    let mut deck = Vec::with_capacity(150);
    deck.extend(std::iter::repeat_n(-2, 5));
    deck.extend(std::iter::repeat_n(-1, 10));
    deck.extend(std::iter::repeat_n(0, 15));
    for v in 1..=12 {
        deck.extend(std::iter::repeat_n(v, 10));
    }
    debug_assert_eq!(deck.len(), 150);
    deck
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisibleSlot {
    Hidden,
    Revealed(CardValue),
    Cleared,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_deck_has_150_cards() {
        let deck = standard_deck();
        assert_eq!(deck.len(), 150);
    }

    #[test]
    fn standard_deck_distribution() {
        let deck = standard_deck();
        assert_eq!(deck.iter().filter(|&&v| v == -2).count(), 5);
        assert_eq!(deck.iter().filter(|&&v| v == -1).count(), 10);
        assert_eq!(deck.iter().filter(|&&v| v == 0).count(), 15);
        for v in 1..=12 {
            assert_eq!(deck.iter().filter(|&&c| c == v).count(), 10);
        }
    }

    #[test]
    fn slot_visibility() {
        let hidden = Slot::Hidden(5);
        assert_eq!(hidden.value(), Some(5));
        assert_eq!(hidden.visible_value(), None);
        assert!(hidden.is_hidden());

        let revealed = Slot::Revealed(3);
        assert_eq!(revealed.value(), Some(3));
        assert_eq!(revealed.visible_value(), Some(3));
        assert!(revealed.is_revealed());

        let cleared = Slot::Cleared;
        assert_eq!(cleared.value(), None);
        assert_eq!(cleared.visible_value(), None);
        assert!(cleared.is_cleared());
    }
}
