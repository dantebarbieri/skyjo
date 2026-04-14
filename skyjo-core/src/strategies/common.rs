use serde::{Deserialize, Serialize};

use crate::card::{CardValue, VisibleSlot, standard_deck};
use crate::strategy::StrategyView;
use std::collections::HashMap;

// --- Common concept descriptions for the strategy guide ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonConcept {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

pub fn common_concepts() -> Vec<CommonConcept> {
    vec![
        CommonConcept {
            id: "card_counting".into(),
            label: "Card Counting".into(),
            description: "The deck has a known composition: 5x(-2), 10x(-1), 15x(0), and 10 copies each of 1-12 (150 total). By tracking which cards are visible — on all players' boards and in the discard pile — the bot can calculate exactly how many copies of each value remain unseen.".into(),
            formula: Some("remaining(value) = total_in_deck(value) - visible_on_boards(value) - visible_in_discard(value)".into()),
        },
        CommonConcept {
            id: "average_unknown".into(),
            label: "Average Unknown Value".into(),
            description: "The expected value of any unseen card (in the deck or hidden on any board). Computed as a weighted average of all remaining card values. For a fresh game this is about 5.07, but it shifts as cards are revealed — if many high cards are already visible, the average of remaining cards drops.".into(),
            formula: Some("avg = sum(value * remaining(value) for each value) / total_remaining".into()),
        },
        CommonConcept {
            id: "expected_score".into(),
            label: "Expected Score".into(),
            description: "An estimate of a player's total board score. Revealed cards contribute their face value, cleared columns contribute 0, and hidden cards are estimated using the average unknown value.".into(),
            formula: Some("expected = sum(revealed_values) + hidden_count * average_unknown_value".into()),
        },
        CommonConcept {
            id: "column_analysis".into(),
            label: "Column Analysis".into(),
            description: "Examining each column on the board to detect 'partial matches' — columns where 2 or more revealed cards share the same value. These columns are candidates for column clears if the remaining hidden slots can be filled with the matching value.".into(),
            formula: None,
        },
        CommonConcept {
            id: "opponent_denial".into(),
            label: "Opponent Denial".into(),
            description: "Evaluating how useful a card would be to the next player before leaving it on the discard pile. Low/negative cards are always useful. Cards matching an opponent's partial column are extremely valuable to them — especially if they only need one more to complete a clear. The bot avoids leaving such cards on the discard pile.".into(),
            formula: None,
        },
    ]
}

/// Build the full deck distribution as a map: value → total count.
pub fn deck_distribution() -> HashMap<CardValue, usize> {
    let mut dist: HashMap<CardValue, usize> = HashMap::new();
    for v in standard_deck() {
        *dist.entry(v).or_insert(0) += 1;
    }
    dist
}

/// Count how many copies of `value` are accounted for (visible on all boards + in discard piles).
pub fn count_visible(view: &StrategyView, value: CardValue) -> usize {
    let mut count = 0;

    // Own board revealed cards
    for slot in &view.my_board {
        if let VisibleSlot::Revealed(v) = slot
            && *v == value
        {
            count += 1;
        }
    }

    // Opponent boards revealed cards
    for board in &view.opponent_boards {
        for slot in board {
            if let VisibleSlot::Revealed(v) = slot
                && *v == value
            {
                count += 1;
            }
        }
    }

    // All discard piles
    for pile in &view.discard_piles {
        for &v in pile {
            if v == value {
                count += 1;
            }
        }
    }

    count
}

/// Count how many copies of `value` remain unseen (in deck + hidden slots).
pub fn count_remaining(view: &StrategyView, value: CardValue) -> usize {
    let dist = deck_distribution();
    let total = dist.get(&value).copied().unwrap_or(0);
    let visible = count_visible(view, value);
    total.saturating_sub(visible)
}

/// Total number of unknown cards (deck + all hidden slots on all boards).
pub fn total_unknown(view: &StrategyView) -> usize {
    let hidden_own = view
        .my_board
        .iter()
        .filter(|s| matches!(s, VisibleSlot::Hidden))
        .count();
    let hidden_opp: usize = view
        .opponent_boards
        .iter()
        .map(|b| b.iter().filter(|s| matches!(s, VisibleSlot::Hidden)).count())
        .sum();
    view.deck_remaining + hidden_own + hidden_opp
}

/// Average value of an unknown card based on the remaining card distribution.
pub fn average_unknown_value(view: &StrategyView) -> f64 {
    let dist = deck_distribution();
    let mut weighted_sum: f64 = 0.0;
    let mut total_remaining: usize = 0;

    for (&value, &total_count) in &dist {
        let visible = count_visible(view, value);
        let remaining = total_count.saturating_sub(visible);
        weighted_sum += value as f64 * remaining as f64;
        total_remaining += remaining;
    }

    if total_remaining == 0 {
        0.0
    } else {
        weighted_sum / total_remaining as f64
    }
}

/// Expected score for a board given the average unknown value.
pub fn expected_score(board: &[VisibleSlot], avg: f64) -> f64 {
    board
        .iter()
        .map(|s| match s {
            VisibleSlot::Revealed(v) => *v as f64,
            VisibleSlot::Hidden => avg,
            VisibleSlot::Cleared => 0.0,
        })
        .sum()
}

/// Info about a column on the player's own board.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub col: usize,
    pub indices: Vec<usize>,
    pub revealed_values: Vec<(usize, CardValue)>,
    pub hidden_indices: Vec<usize>,
    pub cleared_count: usize,
    /// If 2+ revealed cards share the same value, that value and its count.
    pub partial_match: Option<(CardValue, usize)>,
}

/// Analyze all columns on a board.
pub fn column_analysis(view: &StrategyView) -> Vec<ColumnInfo> {
    let mut result = Vec::with_capacity(view.num_cols);
    for col in 0..view.num_cols {
        let indices = view.column_indices(col);
        let mut revealed_values = Vec::new();
        let mut hidden_indices = Vec::new();
        let mut cleared_count = 0;

        for &idx in &indices {
            match &view.my_board[idx] {
                VisibleSlot::Revealed(v) => revealed_values.push((idx, *v)),
                VisibleSlot::Hidden => hidden_indices.push(idx),
                VisibleSlot::Cleared => cleared_count += 1,
            }
        }

        // Find partial match: most common revealed value with count >= 2
        let partial_match = if revealed_values.len() >= 2 {
            let mut counts: HashMap<CardValue, usize> = HashMap::new();
            for &(_, v) in &revealed_values {
                *counts.entry(v).or_insert(0) += 1;
            }
            counts
                .into_iter()
                .filter(|&(_, count)| count >= 2)
                .max_by_key(|&(_, count)| count)
        } else {
            None
        };

        result.push(ColumnInfo {
            col,
            indices,
            revealed_values,
            hidden_indices,
            cleared_count,
            partial_match,
        });
    }
    result
}

/// Analyze columns on an opponent's board using their VisibleSlot data.
pub fn opponent_column_analysis(
    board: &[VisibleSlot],
    num_rows: usize,
    num_cols: usize,
) -> Vec<ColumnInfo> {
    let mut result = Vec::with_capacity(num_cols);
    for col in 0..num_cols {
        let base = col * num_rows;
        let indices: Vec<usize> = (base..base + num_rows).collect();
        let mut revealed_values = Vec::new();
        let mut hidden_indices = Vec::new();
        let mut cleared_count = 0;

        for &idx in &indices {
            match &board[idx] {
                VisibleSlot::Revealed(v) => revealed_values.push((idx, *v)),
                VisibleSlot::Hidden => hidden_indices.push(idx),
                VisibleSlot::Cleared => cleared_count += 1,
            }
        }

        let partial_match = if revealed_values.len() >= 2 {
            let mut counts: HashMap<CardValue, usize> = HashMap::new();
            for &(_, v) in &revealed_values {
                *counts.entry(v).or_insert(0) += 1;
            }
            counts
                .into_iter()
                .filter(|&(_, count)| count >= 2)
                .max_by_key(|&(_, count)| count)
        } else {
            None
        };

        result.push(ColumnInfo {
            col,
            indices,
            revealed_values,
            hidden_indices,
            cleared_count,
            partial_match,
        });
    }
    result
}

/// Get the next player's board from the view.
/// The next player is the one immediately after us in turn order.
pub fn next_player_board(view: &StrategyView) -> Option<&Vec<VisibleSlot>> {
    // In a game with N players, the next player after my_index is (my_index + 1) % N.
    // opponent_indices lists all players except my_index in order.
    // The next player is (my_index + 1) % total_players.
    let total_players = view.opponent_boards.len() + 1;
    let next_idx = (view.my_index + 1) % total_players;

    // Find position of next_idx in opponent_indices
    view.opponent_indices
        .iter()
        .position(|&i| i == next_idx)
        .map(|pos| &view.opponent_boards[pos])
}

/// Evaluate how useful a card value is to a player based on their board.
/// Returns a score: higher = more useful to them.
/// Considers: low/negative values are always useful; values matching partial columns are very useful.
pub fn card_usefulness_to_player(
    board: &[VisibleSlot],
    num_rows: usize,
    num_cols: usize,
    value: CardValue,
) -> f64 {
    let mut usefulness: f64 = 0.0;

    // Low/negative values are inherently useful
    if value <= 0 {
        usefulness += (1 - value) as f64 * 2.0; // -2 → 6.0, -1 → 4.0, 0 → 2.0
    }

    // Check if this value matches any partial column
    let cols = opponent_column_analysis(board, num_rows, num_cols);
    for col_info in &cols {
        if col_info.cleared_count > 0 {
            continue;
        }
        if let Some((match_val, match_count)) = col_info.partial_match
            && match_val == value
        {
            let needed = num_rows - match_count;
            // Very useful if they only need 1 more to complete
            usefulness += if needed == 1 {
                20.0
            } else {
                5.0 * match_count as f64
            };
        }
        // Also check if value matches a single revealed card in a column with hidden slots
        if col_info.revealed_values.len() == 1
            && !col_info.hidden_indices.is_empty()
            && col_info.revealed_values[0].1 == value
        {
            usefulness += 2.0;
        }
    }

    // Higher-value cards are less inherently useful (opponent wouldn't want them)
    if value > 0 {
        usefulness -= value as f64 * 0.3;
    }

    usefulness
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view_with_board(board: Vec<VisibleSlot>) -> StrategyView {
        StrategyView {
            my_index: 0,
            my_board: board,
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![],
            opponent_indices: vec![],
            discard_piles: vec![vec![]],
            deck_remaining: 100,
            cumulative_scores: vec![0],
            is_final_turn: false,
        }
    }

    #[test]
    fn test_deck_distribution() {
        let dist = deck_distribution();
        assert_eq!(dist[&-2], 5);
        assert_eq!(dist[&-1], 10);
        assert_eq!(dist[&0], 15);
        for v in 1..=12 {
            assert_eq!(dist[&v], 10);
        }
        let total: usize = dist.values().sum();
        assert_eq!(total, 150);
    }

    #[test]
    fn test_count_remaining_all_hidden() {
        let board = vec![VisibleSlot::Hidden; 12];
        let view = make_view_with_board(board);
        // Nothing visible, so all copies remain
        assert_eq!(count_remaining(&view, 5), 10);
        assert_eq!(count_remaining(&view, -2), 5);
        assert_eq!(count_remaining(&view, 0), 15);
    }

    #[test]
    fn test_count_remaining_with_visible() {
        let mut board = vec![VisibleSlot::Hidden; 12];
        board[0] = VisibleSlot::Revealed(5);
        board[1] = VisibleSlot::Revealed(5);
        let mut view = make_view_with_board(board);
        view.discard_piles = vec![vec![5, 5, 5]];
        // 10 total 5s, 2 on board + 3 in discard = 5 visible → 5 remaining
        assert_eq!(count_remaining(&view, 5), 5);
    }

    #[test]
    fn test_average_unknown_value_full_deck() {
        let board = vec![VisibleSlot::Hidden; 12];
        let view = make_view_with_board(board);
        let avg = average_unknown_value(&view);
        // Full deck average: (5*-2 + 10*-1 + 15*0 + 10*(1+2+...+12)) / 150
        // = (-10 + -10 + 0 + 10*78) / 150 = 760 / 150 ≈ 5.0667
        assert!((avg - 5.0667).abs() < 0.01);
    }

    #[test]
    fn test_expected_score() {
        let board = vec![
            VisibleSlot::Revealed(3),
            VisibleSlot::Hidden,
            VisibleSlot::Cleared,
            VisibleSlot::Revealed(7),
        ];
        let avg = 5.0;
        let score = expected_score(&board, avg);
        // 3 + 5.0 + 0 + 7 = 15.0
        assert!((score - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_column_analysis_partial_match() {
        let board = vec![
            // col 0: two 5s and a hidden
            VisibleSlot::Revealed(5),
            VisibleSlot::Revealed(5),
            VisibleSlot::Hidden,
            // col 1: all different
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            // col 2-3: hidden
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view_with_board(board);
        let analysis = column_analysis(&view);

        assert_eq!(analysis[0].partial_match, Some((5, 2)));
        assert!(analysis[1].partial_match.is_none());
    }

    #[test]
    fn test_next_player_board() {
        let view = StrategyView {
            my_index: 0,
            my_board: vec![VisibleSlot::Hidden; 12],
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![
                vec![VisibleSlot::Revealed(1); 12], // player 1
                vec![VisibleSlot::Revealed(2); 12], // player 2
            ],
            opponent_indices: vec![1, 2],
            discard_piles: vec![vec![]],
            deck_remaining: 100,
            cumulative_scores: vec![0, 0, 0],
            is_final_turn: false,
        };

        let next = next_player_board(&view).unwrap();
        assert_eq!(next[0], VisibleSlot::Revealed(1)); // player 1's board
    }
}
