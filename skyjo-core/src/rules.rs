use crate::card::{CardValue, standard_deck};

/// Abstracts game rules, allowing hot-swappable rule variants.
/// Implement this trait for variants like "Aunt Janet Rules" (per-player discard piles).
pub trait Rules: Send + Sync {
    fn name(&self) -> &str;

    /// Number of rows in the player grid.
    fn num_rows(&self) -> usize;

    /// Number of columns in the player grid.
    fn num_cols(&self) -> usize;

    /// Total cards per player (rows * cols).
    fn num_cards_per_player(&self) -> usize {
        self.num_rows() * self.num_cols()
    }

    /// Number of cards each player flips at the start of a round.
    fn initial_flips(&self) -> usize;

    /// Cumulative score threshold that ends the game.
    fn end_threshold(&self) -> i32;

    /// Number of discard piles. 1 for standard, num_players for per-player variants.
    fn discard_pile_count(&self, num_players: usize) -> usize;

    /// Which discard pile(s) can this player draw from?
    fn drawable_piles(&self, player_index: usize, num_players: usize) -> Vec<usize>;

    /// Which discard pile does this player's discard go to?
    fn discard_target(&self, player_index: usize) -> usize;

    /// Number of matching revealed cards in a column required to trigger clearing.
    fn column_clear_threshold(&self) -> usize {
        self.num_rows()
    }

    /// Apply going-out penalty.
    /// `is_solo_lowest` is true only if the goer's score is strictly less than all others.
    fn apply_going_out_penalty(
        &self,
        goer_score: i32,
        min_other_score: i32,
        is_solo_lowest: bool,
    ) -> i32;

    /// Determine starting player for round 1.
    /// `revealed_sums` contains the sum of each player's initially revealed cards.
    fn first_round_starting_player(&self, revealed_sums: &[i32]) -> usize;

    /// Whether to reshuffle discards into the deck when the deck is empty.
    fn reshuffle_on_empty_deck(&self) -> bool;

    /// Build the deck for this rule set.
    fn build_deck(&self) -> Vec<CardValue>;

    /// Determine the winner(s) from final cumulative scores.
    /// Returns indices of all winners (may be multiple in case of ties).
    fn resolve_winners(&self, cumulative_scores: &[i32]) -> Vec<usize>;
}

/// Standard Skyjo rules.
#[derive(Debug, Clone)]
pub struct StandardRules;

impl Rules for StandardRules {
    fn name(&self) -> &str {
        "Standard"
    }

    fn num_rows(&self) -> usize {
        3
    }

    fn num_cols(&self) -> usize {
        4
    }

    fn initial_flips(&self) -> usize {
        2
    }

    fn end_threshold(&self) -> i32 {
        100
    }

    fn discard_pile_count(&self, _num_players: usize) -> usize {
        1
    }

    fn drawable_piles(&self, _player_index: usize, _num_players: usize) -> Vec<usize> {
        vec![0]
    }

    fn discard_target(&self, _player_index: usize) -> usize {
        0
    }

    fn apply_going_out_penalty(
        &self,
        goer_score: i32,
        _min_other_score: i32,
        is_solo_lowest: bool,
    ) -> i32 {
        if is_solo_lowest || goer_score <= 0 {
            goer_score
        } else {
            goer_score * 2
        }
    }

    fn first_round_starting_player(&self, revealed_sums: &[i32]) -> usize {
        // Highest sum goes first. Tiebreak: lowest index (emulating "youngest goes first").
        revealed_sums
            .iter()
            .enumerate()
            .max_by(|(i, a), (j, b)| a.cmp(b).then(j.cmp(i)))
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    fn reshuffle_on_empty_deck(&self) -> bool {
        true
    }

    fn build_deck(&self) -> Vec<CardValue> {
        standard_deck()
    }

    fn resolve_winners(&self, cumulative_scores: &[i32]) -> Vec<usize> {
        let min_score = cumulative_scores.iter().copied().min().unwrap_or(0);
        cumulative_scores
            .iter()
            .enumerate()
            .filter(|&(_, &s)| s == min_score)
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_penalty_solo_lowest() {
        let rules = StandardRules;
        // Goer has 10, is solo lowest -> no penalty
        assert_eq!(rules.apply_going_out_penalty(10, 15, true), 10);
    }

    #[test]
    fn standard_penalty_not_lowest() {
        let rules = StandardRules;
        // Goer has 15, not solo lowest -> doubled
        assert_eq!(rules.apply_going_out_penalty(15, 10, false), 30);
    }

    #[test]
    fn standard_penalty_tied_lowest() {
        let rules = StandardRules;
        // Goer has 10, tied (not solo lowest) -> doubled
        assert_eq!(rules.apply_going_out_penalty(10, 10, false), 20);
    }

    #[test]
    fn standard_penalty_negative_not_lowest() {
        let rules = StandardRules;
        // Goer has -5, not solo lowest -> no penalty (score <= 0)
        assert_eq!(rules.apply_going_out_penalty(-5, -10, false), -5);
    }

    #[test]
    fn standard_penalty_zero() {
        let rules = StandardRules;
        // Goer has 0, not solo lowest -> no penalty (score <= 0)
        assert_eq!(rules.apply_going_out_penalty(0, -2, false), 0);
    }

    #[test]
    fn starting_player_highest_sum() {
        let rules = StandardRules;
        assert_eq!(rules.first_round_starting_player(&[3, 7, 5, 2]), 1);
    }

    #[test]
    fn starting_player_tiebreak_lowest_index() {
        let rules = StandardRules;
        // Players 0 and 2 both have 7 -> player 0 wins tiebreak
        assert_eq!(rules.first_round_starting_player(&[7, 3, 7, 2]), 0);
    }

    #[test]
    fn resolve_winners_single() {
        let rules = StandardRules;
        assert_eq!(rules.resolve_winners(&[50, 80, 105, 70]), vec![0]);
    }

    #[test]
    fn resolve_winners_tie() {
        let rules = StandardRules;
        assert_eq!(rules.resolve_winners(&[50, 80, 50, 70]), vec![0, 2]);
    }
}
