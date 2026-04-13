use serde::{Deserialize, Serialize};

use crate::card::CardValue;
use crate::strategy::DeckDrawAction;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameHistory {
    pub seed: u64,
    pub num_players: usize,
    pub strategy_names: Vec<String>,
    pub rules_name: String,
    pub rounds: Vec<RoundHistory>,
    pub final_scores: Vec<i32>,
    pub winners: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundHistory {
    pub round_number: usize,
    pub initial_deck_order: Vec<CardValue>,
    pub dealt_hands: Vec<Vec<CardValue>>,
    pub setup_flips: Vec<Vec<usize>>,
    pub starting_player: usize,
    pub turns: Vec<TurnRecord>,
    pub going_out_player: Option<usize>,
    /// Column clears that happened during end-of-round reveal.
    pub end_of_round_clears: Vec<ColumnClearEvent>,
    pub round_scores: Vec<i32>,
    pub cumulative_scores: Vec<i32>,
    /// True if this round was forcefully ended by the turn limit safety valve.
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub player_index: usize,
    pub action: TurnAction,
    pub column_clears: Vec<ColumnClearEvent>,
    pub went_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnAction {
    DrewFromDeck {
        drawn_card: CardValue,
        action: DeckDrawAction,
        /// The card that was displaced (if Keep was chosen).
        displaced_card: Option<CardValue>,
    },
    DrewFromDiscard {
        pile_index: usize,
        drawn_card: CardValue,
        placement: usize,
        displaced_card: CardValue,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnClearEvent {
    pub player_index: usize,
    pub column: usize,
    pub card_value: CardValue,
    /// The card that was displaced (replaced) by the placement that triggered this clear.
    /// `None` for end-of-round clears and discard-and-flip clears (no card was replaced).
    pub displaced_card: Option<CardValue>,
}
