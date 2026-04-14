use rand::seq::IndexedRandom;
use rand::prelude::SliceRandom;
use rand::RngCore;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, ConceptReference, DecisionLogic, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, PriorityRule, Strategy, StrategyDescription, StrategyView,
};

use super::common::{average_unknown_value, expected_score, opponent_column_analysis};

/// A strategy that copies the leader. Finds the opponent with the lowest expected
/// score and tries to replicate their board patterns (card values, column structures).
pub struct MimicStrategy;

/// Find the opponent board with the lowest expected score (the "leader").
fn find_leader(view: &StrategyView) -> Option<&Vec<VisibleSlot>> {
    if view.opponent_boards.is_empty() {
        return None;
    }
    let avg = average_unknown_value(view);
    view.opponent_boards
        .iter()
        .min_by(|a, b| {
            let sa = expected_score(a, avg);
            let sb = expected_score(b, avg);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Check if any revealed slot on a board matches the given value.
fn leader_has_value(board: &[VisibleSlot], value: CardValue) -> bool {
    board.iter().any(|s| matches!(s, VisibleSlot::Revealed(v) if *v == value))
}

fn highest_revealed_value(board: &[VisibleSlot]) -> Option<CardValue> {
    board
        .iter()
        .filter_map(|s| match s {
            VisibleSlot::Revealed(v) => Some(*v),
            _ => None,
        })
        .max()
}

fn position_of_highest_revealed(board: &[VisibleSlot]) -> usize {
    board
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            VisibleSlot::Revealed(v) => Some((i, *v)),
            _ => None,
        })
        .max_by_key(|(_, v)| *v)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

impl Strategy for MimicStrategy {
    fn name(&self) -> &str {
        "Mimic"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Mimic".into(),
            summary: "Copy the leader. Identifies the opponent with the lowest expected score and tries to replicate their board patterns — matching their revealed card values and mirroring their column structures.".into(),
            complexity: Complexity::Medium,
            strengths: vec![
                "Adapts to the current game state by following proven patterns".into(),
                "Benefits from opponents' good decisions without computing them independently".into(),
                "Naturally gravitates toward low-scoring board configurations".into(),
            ],
            weaknesses: vec![
                "Reactive — always one step behind the leader".into(),
                "Cannot exploit opportunities the leader hasn't found".into(),
                "Falls back to greedy logic when no leader pattern applies".into(),
            ],
            phases: vec![
                PhaseDescription {
                    phase: Phase::InitialFlips,
                    label: "Initial Flips".into(),
                    logic: DecisionLogic::Simple {
                        text: "Spread flips across different columns (one per column) to maximize information about the board. Unlike Clearer which clusters flips, Mimic wants visibility across all columns.".into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::ChooseDraw,
                    label: "Draw Decision".into(),
                    logic: DecisionLogic::PriorityList {
                        rules: vec![
                            PriorityRule {
                                condition: "Discard top matches a value the leader has revealed".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Mimics the leader's board by acquiring their card values.".into()),
                            },
                            PriorityRule {
                                condition: "Otherwise".into(),
                                action: "Draw from deck".into(),
                                detail: None,
                            },
                        ],
                    },
                },
                PhaseDescription {
                    phase: Phase::DeckDrawAction,
                    label: "After Drawing from Deck".into(),
                    logic: DecisionLogic::PriorityList {
                        rules: vec![
                            PriorityRule {
                                condition: "Card matches a value on the leader's board".into(),
                                action: "Keep it — replace highest revealed or a hidden slot".into(),
                                detail: Some("Replicates leader's values on our own board.".into()),
                            },
                            PriorityRule {
                                condition: "Leader has a partial column match and card matches that value".into(),
                                action: "Keep it — place in a column that could build toward a clear".into(),
                                detail: Some("Mirrors leader's column-clearing patterns.".into()),
                            },
                            PriorityRule {
                                condition: "Card < highest revealed card (greedy fallback)".into(),
                                action: "Keep it — replace the highest revealed card".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "Otherwise".into(),
                                action: "Discard and flip a random hidden card".into(),
                                detail: None,
                            },
                        ],
                    },
                },
                PhaseDescription {
                    phase: Phase::DiscardDrawPlacement,
                    label: "After Drawing from Discard".into(),
                    logic: DecisionLogic::PriorityList {
                        rules: vec![
                            PriorityRule {
                                condition: "Card matches a leader's value and we can mirror column structure".into(),
                                action: "Place in a column similar to the leader's layout".into(),
                                detail: Some("Attempts to build matching column patterns.".into()),
                            },
                            PriorityRule {
                                condition: "A revealed card is higher than drawn card".into(),
                                action: "Replace the highest such revealed card".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "A hidden slot is available".into(),
                                action: "Replace a hidden card".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "Otherwise".into(),
                                action: "Replace the highest revealed card".into(),
                                detail: None,
                            },
                        ],
                    },
                },
            ],
            concepts: vec![
                ConceptReference {
                    id: "expected_score".into(),
                    label: "Expected Score".into(),
                    used_for: "Identifying which opponent is the leader (lowest expected score).".into(),
                },
                ConceptReference {
                    id: "average_unknown".into(),
                    label: "Average Unknown Value".into(),
                    used_for: "Estimating hidden card values when computing expected scores.".into(),
                },
            ],
        }
    }

    fn choose_initial_flips(
        &self,
        view: &StrategyView,
        count: usize,
        rng: &mut dyn RngCore,
    ) -> Vec<usize> {
        // Spread flips across different columns — one per column until count is met.
        let mut cols: Vec<usize> = (0..view.num_cols).collect();
        cols.shuffle(rng);

        let mut result = Vec::with_capacity(count);
        for &col in &cols {
            if result.len() >= count {
                break;
            }
            let base = col * view.num_rows;
            // Pick one hidden slot from this column
            for row in 0..view.num_rows {
                let idx = base + row;
                if matches!(view.my_board[idx], VisibleSlot::Hidden) {
                    result.push(idx);
                    break;
                }
            }
        }

        // If we still need more (count > num_cols), fill from remaining hidden slots
        if result.len() < count {
            for (idx, slot) in view.my_board.iter().enumerate() {
                if result.len() >= count {
                    break;
                }
                if matches!(slot, VisibleSlot::Hidden) && !result.contains(&idx) {
                    result.push(idx);
                }
            }
        }

        result
    }

    fn choose_draw(&self, view: &StrategyView, _rng: &mut dyn RngCore) -> DrawChoice {
        if let Some(discard_val) = view.discard_top(0)
            && let Some(leader_board) = find_leader(view)
            && leader_has_value(leader_board, discard_val)
        {
            return DrawChoice::DrawFromDiscard(0);
        }
        DrawChoice::DrawFromDeck
    }

    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> DeckDrawAction {
        if let Some(leader) = find_leader(view) {
            // Priority 1: Card matches a value on the leader's board
            if leader_has_value(leader, drawn_card) {
                // Try to place: replace highest revealed, or a hidden slot
                let highest = highest_revealed_value(&view.my_board);
                if highest.is_some_and(|h| drawn_card < h) {
                    return DeckDrawAction::Keep(position_of_highest_revealed(&view.my_board));
                }
                // Place on a hidden slot
                if let Some(pos) = view
                    .my_board
                    .iter()
                    .position(|s| matches!(s, VisibleSlot::Hidden))
                {
                    return DeckDrawAction::Keep(pos);
                }
            }

            // Priority 2: Leader has a partial column match and drawn card matches that value
            let leader_cols =
                opponent_column_analysis(leader, view.num_rows, view.num_cols);
            for leader_col in &leader_cols {
                if let Some((match_val, _)) = leader_col.partial_match
                    && match_val == drawn_card
                    && let Some(pos) = find_mimic_column_position(view, drawn_card)
                {
                    return DeckDrawAction::Keep(pos);
                }
            }
        }

        // Priority 3: Greedy fallback — replace highest revealed if improvement
        let highest = highest_revealed_value(&view.my_board);
        if highest.is_some_and(|h| drawn_card < h) {
            return DeckDrawAction::Keep(position_of_highest_revealed(&view.my_board));
        }

        // Discard and flip a random hidden card
        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        if let Some(&pos) = hidden.choose(rng) {
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden cards — must keep somewhere
        DeckDrawAction::Keep(position_of_highest_revealed(&view.my_board))
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        // Priority 1: If card matches a leader's value, try to mirror column structure
        if let Some(leader) = find_leader(view)
            && leader_has_value(leader, drawn_card)
            && let Some(pos) = find_mimic_column_position(view, drawn_card)
        {
            return pos;
        }

        // Priority 2: Replace highest revealed card that is greater than drawn card
        let best_revealed = view
            .my_board
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                VisibleSlot::Revealed(v) if *v > drawn_card => Some((i, *v)),
                _ => None,
            })
            .max_by_key(|(_, v)| *v);

        if let Some((pos, _)) = best_revealed {
            return pos;
        }

        // Priority 3: Replace a hidden slot
        if let Some(pos) = view
            .my_board
            .iter()
            .position(|s| matches!(s, VisibleSlot::Hidden))
        {
            return pos;
        }

        // Last resort: replace highest revealed
        position_of_highest_revealed(&view.my_board)
    }
}

/// Find a position on our board to place `value` that mirrors the leader's column
/// structure. Prefers columns where we already have matching cards (to build toward
/// a clear), then columns with hidden slots.
fn find_mimic_column_position(view: &StrategyView, value: CardValue) -> Option<usize> {
    // First pass: find a column where we already have 1+ matching revealed cards
    // and there's a slot to place the new card (hidden or non-matching revealed)
    for col in 0..view.num_cols {
        let indices = view.column_indices(col);
        let mut matching_count = 0;
        let mut has_cleared = false;
        let mut best_target: Option<(usize, i8)> = None; // (index, priority)

        for &idx in &indices {
            match view.my_board[idx] {
                VisibleSlot::Revealed(v) if v == value => matching_count += 1,
                VisibleSlot::Revealed(v) => {
                    // Candidate for replacement — prefer replacing higher values
                    let priority = v;
                    if best_target.is_none() || priority > best_target.unwrap().1 {
                        best_target = Some((idx, priority));
                    }
                }
                VisibleSlot::Hidden => {
                    // Hidden slots get low priority (value 0 equivalent)
                    if best_target.is_none() {
                        best_target = Some((idx, 0));
                    }
                }
                VisibleSlot::Cleared => has_cleared = true,
            }
        }

        if has_cleared {
            continue;
        }

        if matching_count >= 1
            && let Some((pos, _)) = best_target
        {
            return Some(pos);
        }
    }

    // Second pass: find any column with a hidden slot
    for col in 0..view.num_cols {
        let indices = view.column_indices(col);
        let has_cleared = indices
            .iter()
            .any(|&idx| matches!(view.my_board[idx], VisibleSlot::Cleared));
        if has_cleared {
            continue;
        }
        for &idx in &indices {
            if matches!(view.my_board[idx], VisibleSlot::Hidden) {
                return Some(idx);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view(board: Vec<VisibleSlot>) -> StrategyView {
        StrategyView {
            my_index: 0,
            my_board: board,
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![vec![
                VisibleSlot::Revealed(2),
                VisibleSlot::Revealed(3),
                VisibleSlot::Hidden,
                VisibleSlot::Revealed(5),
                VisibleSlot::Revealed(5),
                VisibleSlot::Revealed(5),
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Revealed(1),
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
            ]],
            opponent_indices: vec![1],
            discard_piles: vec![vec![3]],
            deck_remaining: 100,
            cumulative_scores: vec![0, 0],
            is_final_turn: false,
        }
    }

    #[test]
    fn initial_flips_spread_across_columns() {
        let board = vec![VisibleSlot::Hidden; 12];
        let view = StrategyView {
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
        };

        let strategy = MimicStrategy;
        let mut rng = rand::rng();
        let flips = strategy.choose_initial_flips(&view, 2, &mut rng);

        assert_eq!(flips.len(), 2);
        // Each flip should be in a different column
        let col0 = flips[0] / view.num_rows;
        let col1 = flips[1] / view.num_rows;
        assert_ne!(col0, col1, "Flips should be in different columns");
    }

    #[test]
    fn takes_discard_matching_leader_value() {
        let board = vec![VisibleSlot::Hidden; 12];
        // Leader has a revealed 5
        let mut view = make_view(board);
        view.discard_piles = vec![vec![5]];

        let strategy = MimicStrategy;
        let mut rng = rand::rng();
        let draw = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(draw, DrawChoice::DrawFromDiscard(0)));
    }

    #[test]
    fn draws_from_deck_when_discard_not_on_leader() {
        let board = vec![VisibleSlot::Hidden; 12];
        // Leader has 2, 3, 5, 1 revealed — no 10
        let mut view = make_view(board);
        view.discard_piles = vec![vec![10]];

        let strategy = MimicStrategy;
        let mut rng = rand::rng();
        let draw = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(draw, DrawChoice::DrawFromDeck));
    }

    #[test]
    fn keeps_deck_draw_matching_leader() {
        let board = vec![
            VisibleSlot::Revealed(8),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view(board);

        let strategy = MimicStrategy;
        let mut rng = rand::rng();
        // Leader has value 2 revealed; drawn card is 2, which is < 8 (highest revealed)
        let action = strategy.choose_deck_draw_action(&view, 2, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert_eq!(pos, 0, "Should replace highest revealed (8) with leader's value (2)");
            }
            _ => panic!("Should keep card matching leader's value"),
        }
    }

    #[test]
    fn discard_placement_replaces_high_card() {
        let board = vec![
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(3),
            VisibleSlot::Revealed(4),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(6),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            VisibleSlot::Revealed(4),
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
        ];
        let view = make_view(board);

        let strategy = MimicStrategy;
        let mut rng = rand::rng();
        // Drawing a 7 — leader doesn't have 7, so fallback: replace highest revealed > 7 → the 10
        let pos = strategy.choose_discard_draw_placement(&view, 7, &mut rng);
        assert_eq!(pos, 0, "Should replace the 10 (highest > drawn)");
    }

    #[test]
    fn find_leader_returns_lowest_expected() {
        let view = StrategyView {
            my_index: 0,
            my_board: vec![VisibleSlot::Hidden; 12],
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![
                vec![VisibleSlot::Revealed(10); 12], // high-scoring opponent
                vec![VisibleSlot::Revealed(1); 12],  // low-scoring leader
            ],
            opponent_indices: vec![1, 2],
            discard_piles: vec![vec![]],
            deck_remaining: 100,
            cumulative_scores: vec![0, 0, 0],
            is_final_turn: false,
        };

        let leader = find_leader(&view).unwrap();
        // Leader should be the board with all 1s (score 12) not all 10s (score 120)
        assert_eq!(leader[0], VisibleSlot::Revealed(1));
    }
}
