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
    /// Pre-penalty scores for each player (before going-out penalty).
    #[serde(default)]
    pub raw_round_scores: Vec<i32>,
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

#[cfg(test)]
mod tests {
    use crate::game::Game;
    use crate::rules::StandardRules;
    use crate::strategies::{GreedyStrategy, RandomStrategy};
    use crate::strategy::Strategy;

    fn play_game(num_players: usize, seed: u64) -> super::GameHistory {
        let strategies: Vec<Box<dyn Strategy>> = (0..num_players)
            .map(|_| Box::new(RandomStrategy) as _)
            .collect();
        Game::new(Box::new(StandardRules), strategies, seed)
            .unwrap()
            .play()
            .unwrap()
    }

    #[test]
    fn history_has_correct_num_players() {
        let history = play_game(3, 42);
        assert_eq!(history.num_players, 3);
        assert_eq!(history.strategy_names.len(), 3);
    }

    #[test]
    fn history_has_at_least_one_round() {
        let history = play_game(2, 42);
        assert!(!history.rounds.is_empty());
    }

    #[test]
    fn each_round_has_turns() {
        let history = play_game(2, 42);
        for round in &history.rounds {
            assert!(
                !round.turns.is_empty(),
                "Round {} has no turns",
                round.round_number
            );
        }
    }

    #[test]
    fn winners_list_is_non_empty() {
        let history = play_game(2, 42);
        assert!(!history.winners.is_empty());
        for &w in &history.winners {
            assert!(w < history.num_players);
        }
    }

    #[test]
    fn final_scores_length_matches_num_players() {
        for n in 2..=5 {
            let history = play_game(n, 42);
            assert_eq!(history.final_scores.len(), n);
        }
    }

    #[test]
    fn round_scores_length_matches_num_players() {
        let history = play_game(4, 42);
        for round in &history.rounds {
            assert_eq!(round.round_scores.len(), 4);
            assert_eq!(round.raw_round_scores.len(), 4);
            assert_eq!(round.cumulative_scores.len(), 4);
            assert_eq!(round.dealt_hands.len(), 4);
            assert_eq!(round.setup_flips.len(), 4);
        }
    }

    #[test]
    fn strategy_names_are_recorded() {
        let strategies: Vec<Box<dyn crate::strategy::Strategy>> =
            vec![Box::new(GreedyStrategy), Box::new(RandomStrategy)];
        let history = Game::new(Box::new(StandardRules), strategies, 42)
            .unwrap()
            .play()
            .unwrap();
        assert_eq!(history.strategy_names, vec!["Greedy", "Random"]);
    }

    #[test]
    fn rules_name_is_recorded() {
        let history = play_game(2, 42);
        assert_eq!(history.rules_name, "Standard");
    }

    #[test]
    fn round_numbers_are_sequential() {
        let history = play_game(2, 42);
        for (i, round) in history.rounds.iter().enumerate() {
            assert_eq!(round.round_number, i);
        }
    }
}
