use serde::{Deserialize, Serialize};

use crate::card::{CardValue, Slot, VisibleSlot};
use crate::error::{Result, SkyjoError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerBoard {
    pub slots: Vec<Slot>,
    pub num_rows: usize,
    pub num_cols: usize,
}

impl PlayerBoard {
    /// Create a new board with all cards hidden. `cards` must have length `num_rows * num_cols`.
    /// Stored in column-major order: indices [0..num_rows) = column 0, etc.
    pub fn new(cards: &[CardValue], num_rows: usize, num_cols: usize) -> Self {
        assert_eq!(cards.len(), num_rows * num_cols);
        PlayerBoard {
            slots: cards.iter().map(|&v| Slot::Hidden(v)).collect(),
            num_rows,
            num_cols,
        }
    }

    pub fn total_slots(&self) -> usize {
        self.num_rows * self.num_cols
    }

    /// Returns the slot indices for the given column (0-based).
    pub fn column_indices(&self, col: usize) -> Vec<usize> {
        let base = col * self.num_rows;
        (base..base + self.num_rows).collect()
    }

    /// Replace a slot with a new revealed card. Returns the old card value.
    /// Works on Hidden, Revealed, or Cleared slots (Cleared returns error).
    pub fn replace(&mut self, pos: usize, new_val: CardValue) -> Result<CardValue> {
        if pos >= self.total_slots() {
            return Err(SkyjoError::InvalidPosition(pos));
        }
        match self.slots[pos] {
            Slot::Cleared => Err(SkyjoError::SlotAlreadyCleared(pos)),
            Slot::Hidden(old) | Slot::Revealed(old) => {
                self.slots[pos] = Slot::Revealed(new_val);
                Ok(old)
            }
        }
    }

    /// Flip a hidden card to revealed. Returns the card value. Errors if not hidden.
    pub fn flip(&mut self, pos: usize) -> Result<CardValue> {
        if pos >= self.total_slots() {
            return Err(SkyjoError::InvalidPosition(pos));
        }
        match self.slots[pos] {
            Slot::Hidden(v) => {
                self.slots[pos] = Slot::Revealed(v);
                Ok(v)
            }
            Slot::Revealed(_) => Err(SkyjoError::CannotFlipRevealed(pos)),
            Slot::Cleared => Err(SkyjoError::SlotAlreadyCleared(pos)),
        }
    }

    /// Check if all cards in a column are revealed with the same value.
    /// Returns Some(value) if so, None otherwise.
    pub fn check_column_match(&self, col: usize) -> Option<CardValue> {
        let indices = self.column_indices(col);
        let mut first_val = None;
        for &idx in &indices {
            match self.slots[idx] {
                Slot::Revealed(v) => {
                    if let Some(fv) = first_val {
                        if v != fv {
                            return None;
                        }
                    } else {
                        first_val = Some(v);
                    }
                }
                _ => return None,
            }
        }
        first_val
    }

    /// Clear a column. Sets all slots to Cleared. Returns the card values removed.
    pub fn clear_column(&mut self, col: usize) -> Vec<CardValue> {
        let indices = self.column_indices(col);
        let mut values = Vec::with_capacity(self.num_rows);
        for &idx in &indices {
            if let Some(v) = self.slots[idx].value() {
                values.push(v);
            }
            self.slots[idx] = Slot::Cleared;
        }
        values
    }

    /// True if every non-cleared slot is Revealed.
    pub fn all_revealed(&self) -> bool {
        self.slots
            .iter()
            .all(|s| matches!(s, Slot::Revealed(_) | Slot::Cleared))
    }

    /// Score: sum of all card values. Hidden cards are included (for end-of-round scoring).
    /// Cleared slots contribute 0.
    pub fn score(&self) -> i32 {
        self.slots
            .iter()
            .map(|s| s.value().unwrap_or(0) as i32)
            .sum()
    }

    /// Number of hidden slots remaining.
    pub fn hidden_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_hidden()).count()
    }

    /// Returns a view of the board that hides hidden card values.
    pub fn visible_view(&self) -> Vec<VisibleSlot> {
        self.slots
            .iter()
            .map(|s| match s {
                Slot::Hidden(_) => VisibleSlot::Hidden,
                Slot::Revealed(v) => VisibleSlot::Revealed(*v),
                Slot::Cleared => VisibleSlot::Cleared,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_board() -> PlayerBoard {
        // Standard 3 rows x 4 cols, values 1..=12
        let cards: Vec<CardValue> = (1..=12).map(|v| v as CardValue).collect();
        PlayerBoard::new(&cards, 3, 4)
    }

    #[test]
    fn new_board_all_hidden() {
        let board = make_board();
        assert_eq!(board.total_slots(), 12);
        assert!(board.slots.iter().all(|s| s.is_hidden()));
        assert_eq!(board.hidden_count(), 12);
        assert!(!board.all_revealed());
    }

    #[test]
    fn column_indices_column_major() {
        let board = make_board();
        assert_eq!(board.column_indices(0), vec![0, 1, 2]);
        assert_eq!(board.column_indices(1), vec![3, 4, 5]);
        assert_eq!(board.column_indices(2), vec![6, 7, 8]);
        assert_eq!(board.column_indices(3), vec![9, 10, 11]);
    }

    #[test]
    fn flip_hidden_card() {
        let mut board = make_board();
        let val = board.flip(0).unwrap();
        assert_eq!(val, 1);
        assert!(board.slots[0].is_revealed());
        assert_eq!(board.hidden_count(), 11);
    }

    #[test]
    fn flip_revealed_errors() {
        let mut board = make_board();
        board.flip(0).unwrap();
        assert_eq!(board.flip(0), Err(SkyjoError::CannotFlipRevealed(0)));
    }

    #[test]
    fn replace_returns_old_value() {
        let mut board = make_board();
        let old = board.replace(0, 99).unwrap();
        assert_eq!(old, 1); // was Hidden(1)
        assert_eq!(board.slots[0], Slot::Revealed(99));
    }

    #[test]
    fn replace_cleared_errors() {
        let mut board = make_board();
        // Force clear column 0
        for &idx in &[0usize, 1, 2] {
            board.slots[idx] = Slot::Cleared;
        }
        assert_eq!(board.replace(0, 5), Err(SkyjoError::SlotAlreadyCleared(0)));
    }

    #[test]
    fn check_column_match_all_same() {
        let mut board = make_board();
        // Set column 0 (indices 0,1,2) to all Revealed(5)
        board.slots[0] = Slot::Revealed(5);
        board.slots[1] = Slot::Revealed(5);
        board.slots[2] = Slot::Revealed(5);
        assert_eq!(board.check_column_match(0), Some(5));
    }

    #[test]
    fn check_column_match_different_values() {
        let mut board = make_board();
        board.slots[0] = Slot::Revealed(5);
        board.slots[1] = Slot::Revealed(5);
        board.slots[2] = Slot::Revealed(3);
        assert_eq!(board.check_column_match(0), None);
    }

    #[test]
    fn check_column_match_has_hidden() {
        let mut board = make_board();
        board.slots[0] = Slot::Revealed(5);
        board.slots[1] = Slot::Revealed(5);
        // slots[2] is still Hidden
        assert_eq!(board.check_column_match(0), None);
    }

    #[test]
    fn clear_column() {
        let mut board = make_board();
        board.slots[0] = Slot::Revealed(5);
        board.slots[1] = Slot::Revealed(5);
        board.slots[2] = Slot::Revealed(5);
        let cleared = board.clear_column(0);
        assert_eq!(cleared, vec![5, 5, 5]);
        assert!(board.slots[0].is_cleared());
        assert!(board.slots[1].is_cleared());
        assert!(board.slots[2].is_cleared());
    }

    #[test]
    fn all_revealed_with_cleared() {
        let mut board = make_board();
        // Reveal all except column 0, which we clear
        board.slots[0] = Slot::Cleared;
        board.slots[1] = Slot::Cleared;
        board.slots[2] = Slot::Cleared;
        for i in 3..12 {
            board.slots[i] = Slot::Revealed(board.slots[i].value().unwrap());
        }
        assert!(board.all_revealed());
    }

    #[test]
    fn score_includes_hidden() {
        let board = make_board();
        // All hidden, values 1..=12, sum = 78
        assert_eq!(board.score(), 78);
    }

    #[test]
    fn score_cleared_is_zero() {
        let mut board = make_board();
        board.clear_column(0); // clears values 1,2,3
        // Remaining: 4+5+6+7+8+9+10+11+12 = 72
        assert_eq!(board.score(), 72);
    }

    #[test]
    fn visible_view_hides_hidden() {
        let mut board = make_board();
        board.slots[0] = Slot::Revealed(5);
        board.slots[3] = Slot::Cleared;
        let view = board.visible_view();
        assert_eq!(view[0], VisibleSlot::Revealed(5));
        assert_eq!(view[1], VisibleSlot::Hidden);
        assert_eq!(view[3], VisibleSlot::Cleared);
    }

    #[test]
    fn custom_grid_size() {
        let cards: Vec<CardValue> = (1..=8).map(|v| v as CardValue).collect();
        let board = PlayerBoard::new(&cards, 2, 4);
        assert_eq!(board.total_slots(), 8);
        assert_eq!(board.column_indices(0), vec![0, 1]);
        assert_eq!(board.column_indices(3), vec![6, 7]);
    }

    #[test]
    fn score_all_negative() {
        let cards: Vec<CardValue> = vec![-2; 12];
        let board = PlayerBoard::new(&cards, 3, 4);
        assert_eq!(board.score(), -24);
    }

    #[test]
    fn score_all_cleared() {
        let mut board = make_board();
        for col in 0..4 {
            board.clear_column(col);
        }
        assert_eq!(board.score(), 0);
    }

    #[test]
    fn column_match_with_cleared_returns_none() {
        let mut board = make_board();
        // Set one slot in column 0 to Cleared
        board.slots[0] = Slot::Cleared;
        board.slots[1] = Slot::Revealed(5);
        board.slots[2] = Slot::Revealed(5);
        assert_eq!(board.check_column_match(0), None);
    }

    #[test]
    fn flip_cleared_slot_errors() {
        let mut board = make_board();
        board.slots[0] = Slot::Cleared;
        assert_eq!(board.flip(0), Err(SkyjoError::SlotAlreadyCleared(0)));
    }

    #[test]
    fn invalid_position_errors() {
        let mut board = make_board();
        let out_of_bounds = board.total_slots(); // 12
        assert_eq!(
            board.flip(out_of_bounds),
            Err(SkyjoError::InvalidPosition(out_of_bounds))
        );
        assert_eq!(
            board.replace(out_of_bounds, 5),
            Err(SkyjoError::InvalidPosition(out_of_bounds))
        );
    }
}
