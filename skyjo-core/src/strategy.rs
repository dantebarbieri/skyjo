use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::card::{CardValue, VisibleSlot};

/// Read-only game snapshot visible to a player during their turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyView {
    pub my_index: usize,
    pub my_board: Vec<VisibleSlot>,
    pub num_rows: usize,
    pub num_cols: usize,
    pub opponent_boards: Vec<Vec<VisibleSlot>>,
    pub opponent_indices: Vec<usize>,
    /// Full contents of each discard pile (enables card counting).
    pub discard_piles: Vec<Vec<CardValue>>,
    pub deck_remaining: usize,
    pub cumulative_scores: Vec<i32>,
    pub is_final_turn: bool,
}

impl StrategyView {
    /// Top card of a discard pile, if it exists.
    pub fn discard_top(&self, pile: usize) -> Option<CardValue> {
        self.discard_piles.get(pile).and_then(|p| p.last().copied())
    }

    /// Column indices for the player's own board.
    pub fn column_indices(&self, col: usize) -> Vec<usize> {
        let base = col * self.num_rows;
        (base..base + self.num_rows).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawChoice {
    DrawFromDeck,
    DrawFromDiscard(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeckDrawAction {
    /// Keep the drawn card, place at this board position (replacing existing card).
    Keep(usize),
    /// Discard the drawn card, flip the hidden card at this position.
    DiscardAndFlip(usize),
}

/// Defines player decision-making at each point in a turn.
/// All methods receive `&mut dyn RngCore` to enable deterministic randomness.
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;

    /// Choose which positions to flip during setup.
    /// Must return exactly `count` distinct positions that are Hidden.
    fn choose_initial_flips(
        &self,
        view: &StrategyView,
        count: usize,
        rng: &mut dyn RngCore,
    ) -> Vec<usize>;

    /// Choose whether to draw from the deck or a discard pile.
    fn choose_draw(&self, view: &StrategyView, rng: &mut dyn RngCore) -> DrawChoice;

    /// After drawing from the deck, decide to keep or discard.
    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> DeckDrawAction;

    /// After drawing from a discard pile (must keep), choose where to place it.
    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> usize;
}
