use rand::RngCore;
use rand::prelude::SliceRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, DecisionLogic, DeckDrawAction, DrawChoice, Phase, PhaseDescription, PriorityRule,
    Strategy, StrategyDescription, StrategyView,
};

use super::common::column_analysis;

/// A rushing strategy:
/// - Goal: reveal all cards as fast as possible to end the round, forcing
///   opponents to stop with unoptimized boards.
/// - Almost always draws from the deck (to get the discard+flip option).
/// - Only takes from discard if the card is <= 0 or completes a column clear.
/// - After a deck draw, almost always discards and flips. Only keeps if the
///   card is <= 0 or replaces a revealed card >= 10.
/// - When flipping, prefers columns closest to full reveal (most revealed cards).
/// - When placing from discard, replaces a hidden card to keep revealing.
pub struct RusherStrategy;

impl Strategy for RusherStrategy {
    fn name(&self) -> &str {
        "Rusher"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Rusher".into(),
            summary: "Ends the round as fast as possible by revealing all cards quickly. Forces opponents to stop with unoptimized boards, betting that speed beats optimization.".into(),
            complexity: Complexity::Low,
            strengths: vec![
                "Ends rounds quickly, punishing slow opponents".into(),
                "Simple decision-making with minimal analysis".into(),
                "Effective against strategies that need many turns to optimize".into(),
            ],
            weaknesses: vec![
                "Often finishes with a mediocre board".into(),
                "Rarely benefits from column clears".into(),
                "Vulnerable to the going-out penalty if opponents have low scores".into(),
            ],
            phases: vec![
                PhaseDescription {
                    phase: Phase::InitialFlips,
                    label: "Initial Flips".into(),
                    logic: DecisionLogic::Simple {
                        text: "Random — no information to optimize on before any cards are revealed.".into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::ChooseDraw,
                    label: "Draw Decision".into(),
                    logic: DecisionLogic::PriorityList {
                        rules: vec![
                            PriorityRule {
                                condition: "Discard top is ≤ 0".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Negative and zero cards are always worth grabbing.".into()),
                            },
                            PriorityRule {
                                condition: "Discard top completes a column clear".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Column clears remove cards and effectively reveal slots.".into()),
                            },
                            PriorityRule {
                                condition: "Otherwise".into(),
                                action: "Draw from deck".into(),
                                detail: Some("Deck draws allow discarding and flipping, which is faster for revealing cards.".into()),
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
                                condition: "Drawn card ≤ 0".into(),
                                action: "Keep it — replace a hidden card".into(),
                                detail: Some("Low cards are worth keeping; placing over hidden still reveals a slot.".into()),
                            },
                            PriorityRule {
                                condition: "Drawn card < a revealed card ≥ 10".into(),
                                action: "Keep it — replace that high revealed card".into(),
                                detail: Some("Worth the detour to remove a very high card.".into()),
                            },
                            PriorityRule {
                                condition: "Hidden cards remain".into(),
                                action: "Discard and flip a hidden card in the column closest to full reveal".into(),
                                detail: Some("Prioritizes columns with the most revealed cards to finish revealing faster.".into()),
                            },
                            PriorityRule {
                                condition: "No hidden cards left".into(),
                                action: "Keep it — replace the highest revealed card".into(),
                                detail: Some("Forced to keep since there's nothing to flip.".into()),
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
                                condition: "Hidden cards exist".into(),
                                action: "Replace a hidden card".into(),
                                detail: Some("Keeps the reveal momentum going.".into()),
                            },
                            PriorityRule {
                                condition: "No hidden cards left".into(),
                                action: "Replace the highest revealed card".into(),
                                detail: None,
                            },
                        ],
                    },
                },
            ],
            concepts: vec![],
        }
    }

    fn choose_initial_flips(
        &self,
        view: &StrategyView,
        count: usize,
        rng: &mut dyn RngCore,
    ) -> Vec<usize> {
        let mut hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();
        hidden.shuffle(rng);
        hidden.truncate(count);
        hidden
    }

    fn choose_draw(&self, view: &StrategyView, _rng: &mut dyn RngCore) -> DrawChoice {
        if let Some(discard_val) = view.discard_top(0) {
            // Take from discard if card is <= 0
            if discard_val <= 0 {
                return DrawChoice::DrawFromDiscard(0);
            }

            // Take from discard if it completes a column clear
            if completes_column_clear(view, discard_val) {
                return DrawChoice::DrawFromDiscard(0);
            }
        }

        DrawChoice::DrawFromDeck
    }

    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> DeckDrawAction {
        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        // Keep if card is <= 0: place over a hidden card to reveal a slot
        if drawn_card <= 0 {
            if let Some(&pos) = hidden.first() {
                return DeckDrawAction::Keep(pos);
            }
            // No hidden cards; replace highest revealed
            return DeckDrawAction::Keep(position_of_highest_revealed(&view.my_board));
        }

        // Keep if it replaces a revealed card >= 10
        if let Some((pos, high_val)) = highest_revealed_at_least(&view.my_board, 10)
            && drawn_card < high_val
        {
            return DeckDrawAction::Keep(pos);
        }

        // Otherwise, discard and flip — prefer columns closest to full reveal
        if !hidden.is_empty() {
            let pos = best_flip_position(view, &hidden);
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden cards left — must keep; place over highest revealed
        DeckDrawAction::Keep(position_of_highest_revealed(&view.my_board))
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        // Check if it completes a column clear — place there
        if let Some(pos) = column_clear_position(view, drawn_card) {
            return pos;
        }

        // Replace a hidden card to keep revealing
        if let Some(pos) = view
            .my_board
            .iter()
            .position(|s| matches!(s, VisibleSlot::Hidden))
        {
            return pos;
        }

        // Fallback: replace the highest revealed card
        position_of_highest_revealed(&view.my_board)
    }
}

/// Check if placing `value` would complete a column clear (all slots in a column
/// match the same value after placement).
fn completes_column_clear(view: &StrategyView, value: CardValue) -> bool {
    column_clear_position(view, value).is_some()
}

/// Find the position where placing `value` would complete a column clear.
/// A column clear happens when all non-cleared slots in a column have the same value.
/// We need: exactly 1 hidden slot remaining, all revealed slots match `value`.
fn column_clear_position(view: &StrategyView, value: CardValue) -> Option<usize> {
    let columns = column_analysis(view);
    for col_info in &columns {
        if col_info.cleared_count > 0 {
            continue;
        }
        if col_info.hidden_indices.len() != 1 {
            continue;
        }
        if let Some((match_val, match_count)) = col_info.partial_match {
            // All revealed cards match `value` and there's exactly 1 hidden slot
            if match_val == value && match_count == col_info.revealed_values.len() {
                return Some(col_info.hidden_indices[0]);
            }
        }
    }
    None
}

/// Find the highest revealed card with value >= threshold.
/// Returns (position, value) of the highest such card.
fn highest_revealed_at_least(
    board: &[VisibleSlot],
    threshold: CardValue,
) -> Option<(usize, CardValue)> {
    board
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            VisibleSlot::Revealed(v) if *v >= threshold => Some((i, *v)),
            _ => None,
        })
        .max_by_key(|(_, v)| *v)
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

/// Choose the best hidden card to flip. Prefers hidden cards in columns that
/// are closest to being fully revealed (most revealed cards already).
fn best_flip_position(view: &StrategyView, hidden: &[usize]) -> usize {
    let columns = column_analysis(view);

    // For each hidden index, score it by how many revealed cards are in its column.
    // Higher = closer to full reveal = preferred.
    let mut best_pos = hidden[0];
    let mut best_revealed_count = 0;

    for &idx in hidden {
        for col_info in &columns {
            if col_info.hidden_indices.contains(&idx) {
                let revealed_count = col_info.revealed_values.len();
                if revealed_count > best_revealed_count {
                    best_revealed_count = revealed_count;
                    best_pos = idx;
                }
                break;
            }
        }
    }

    best_pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::StrategyView;

    fn make_view(board: Vec<VisibleSlot>) -> StrategyView {
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
    fn test_name_and_describe() {
        let s = RusherStrategy;
        assert_eq!(s.name(), "Rusher");
        let desc = s.describe();
        assert_eq!(desc.name, "Rusher");
        assert_eq!(desc.complexity, Complexity::Low);
        assert_eq!(desc.phases.len(), 4);
    }

    #[test]
    fn test_choose_draw_prefers_deck() {
        let s = RusherStrategy;
        let mut rng = rand::rng();
        // Discard top is 5 — not <= 0, no column clear possible
        let mut view = make_view(vec![VisibleSlot::Hidden; 12]);
        view.discard_piles = vec![vec![5]];
        assert_eq!(s.choose_draw(&view, &mut rng), DrawChoice::DrawFromDeck);
    }

    #[test]
    fn test_choose_draw_takes_negative_discard() {
        let s = RusherStrategy;
        let mut rng = rand::rng();
        let mut view = make_view(vec![VisibleSlot::Hidden; 12]);
        view.discard_piles = vec![vec![-1]];
        assert_eq!(
            s.choose_draw(&view, &mut rng),
            DrawChoice::DrawFromDiscard(0)
        );
    }

    #[test]
    fn test_deck_draw_prefers_discard_and_flip() {
        let s = RusherStrategy;
        let mut rng = rand::rng();
        let mut board = vec![VisibleSlot::Hidden; 12];
        board[0] = VisibleSlot::Revealed(3);
        board[1] = VisibleSlot::Revealed(4);
        let view = make_view(board);
        // Drawn card is 5 — not <= 0, no revealed >= 10 — should discard and flip
        let action = s.choose_deck_draw_action(&view, 5, &mut rng);
        assert!(matches!(action, DeckDrawAction::DiscardAndFlip(_)));
    }

    #[test]
    fn test_deck_draw_keeps_negative() {
        let s = RusherStrategy;
        let mut rng = rand::rng();
        let view = make_view(vec![VisibleSlot::Hidden; 12]);
        let action = s.choose_deck_draw_action(&view, -2, &mut rng);
        assert!(matches!(action, DeckDrawAction::Keep(_)));
    }

    #[test]
    fn test_deck_draw_replaces_high_revealed() {
        let s = RusherStrategy;
        let mut rng = rand::rng();
        let mut board = vec![VisibleSlot::Hidden; 12];
        board[0] = VisibleSlot::Revealed(11);
        let view = make_view(board);
        // Drawn card is 5, which is < 11 (revealed >= 10) — should keep and replace
        let action = s.choose_deck_draw_action(&view, 5, &mut rng);
        assert_eq!(action, DeckDrawAction::Keep(0));
    }

    #[test]
    fn test_discard_placement_prefers_hidden() {
        let s = RusherStrategy;
        let mut rng = rand::rng();
        let mut board = vec![VisibleSlot::Hidden; 12];
        board[0] = VisibleSlot::Revealed(3);
        let view = make_view(board);
        let pos = s.choose_discard_draw_placement(&view, 5, &mut rng);
        // Should pick a hidden card position, not position 0 (revealed)
        assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
    }

    #[test]
    fn test_best_flip_prefers_most_revealed_column() {
        // Col 0 (indices 0,1,2): 2 revealed, 1 hidden
        // Col 1 (indices 3,4,5): 0 revealed, 3 hidden
        let board = vec![
            VisibleSlot::Revealed(3),
            VisibleSlot::Revealed(4),
            VisibleSlot::Hidden, // idx 2 — col 0, 2 revealed
            VisibleSlot::Hidden, // idx 3 — col 1, 0 revealed
            VisibleSlot::Hidden, // idx 4
            VisibleSlot::Hidden, // idx 5
            VisibleSlot::Hidden, // idx 6 — col 2
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden, // idx 9 — col 3
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view(board);
        let hidden: Vec<usize> = vec![2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let pos = best_flip_position(&view, &hidden);
        // Should pick index 2 (col 0 has 2 revealed cards, closest to full reveal)
        assert_eq!(pos, 2);
    }

    #[test]
    fn test_column_clear_detection() {
        // Col 0: two 5s revealed, one hidden — placing 5 should complete the clear
        let board = vec![
            VisibleSlot::Revealed(5),
            VisibleSlot::Revealed(5),
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
        assert!(completes_column_clear(&view, 5));
        assert!(!completes_column_clear(&view, 3));
        assert_eq!(column_clear_position(&view, 5), Some(2));
    }
}
