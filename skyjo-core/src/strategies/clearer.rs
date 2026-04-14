use rand::RngCore;
use rand::prelude::SliceRandom;
use rand::seq::IndexedRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, ConceptReference, DecisionLogic, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, PriorityRule, Strategy, StrategyDescription, StrategyView,
};

use super::common::column_analysis;

/// A strategy focused on column clearing as the primary way to reduce score.
///
/// Tolerates high values on the board if they enable column clears (e.g., keeping
/// a 10 when two other 10s are already in the same column). Falls back to
/// greedy-style logic when no clear opportunity exists.
pub struct ClearerStrategy;

impl Strategy for ClearerStrategy {
    fn name(&self) -> &str {
        "Clearer"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Clearer".into(),
            summary: "Prioritizes column clearing as the primary way to reduce score. Willing to keep high-value cards (e.g., a 10) if they complete or advance a column clear, since a cleared column scores 0.".into(),
            complexity: Complexity::Medium,
            strengths: vec![
                "Exploits the powerful column-clear mechanic".into(),
                "Can eliminate high-value columns entirely".into(),
                "Falls back to greedy logic when no clear opportunity exists".into(),
            ],
            weaknesses: vec![
                "May hold high cards hoping for a clear that never comes".into(),
                "No card counting — doesn't check if remaining copies exist".into(),
                "Doesn't consider what opponents need".into(),
            ],
            phases: vec![
                PhaseDescription {
                    phase: Phase::InitialFlips,
                    label: "Initial Flips".into(),
                    logic: DecisionLogic::Simple {
                        text: "Flip cards in the same column to maximize the chance of discovering an early match. Picks a random column and flips there first, spilling into another column only if needed.".into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::ChooseDraw,
                    label: "Draw Decision".into(),
                    logic: DecisionLogic::PriorityList {
                        rules: vec![
                            PriorityRule {
                                condition: "Discard top completes or advances a column clear".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Takes high-value cards if they match a partial column.".into()),
                            },
                            PriorityRule {
                                condition: "Discard top ≤ 0".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Greedy fallback — low cards are always worth taking.".into()),
                            },
                            PriorityRule {
                                condition: "Discard top < highest revealed card".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Greedy fallback — guaranteed improvement.".into()),
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
                                condition: "Card completes a column clear (fills the last slot)".into(),
                                action: "Keep it — place in the completing position".into(),
                                detail: Some("Highest priority: the entire column is removed and scores 0.".into()),
                            },
                            PriorityRule {
                                condition: "Card advances a partial column (1+ matching cards in a column)".into(),
                                action: "Keep it — place in that column".into(),
                                detail: Some("Builds toward a future clear by increasing the match count.".into()),
                            },
                            PriorityRule {
                                condition: "Card < highest revealed card (greedy improvement)".into(),
                                action: "Keep it — replace the highest revealed card".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "Card ≤ 8 and matches a card in a column with hidden slots".into(),
                                action: "Keep it — place in that column to start building".into(),
                                detail: Some("Early-game building: tolerates moderate values to seed future clears.".into()),
                            },
                            PriorityRule {
                                condition: "Otherwise".into(),
                                action: "Discard and flip a hidden card (preferring columns with partial matches)".into(),
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
                                condition: "Card completes a column clear".into(),
                                action: "Place in the completing position".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "Card advances a partial column".into(),
                                action: "Place in that column".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "A revealed card is higher than drawn card".into(),
                                action: "Replace the highest such revealed card".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "Otherwise".into(),
                                action: "Replace a hidden card, or the highest revealed as last resort".into(),
                                detail: None,
                            },
                        ],
                    },
                },
            ],
            concepts: vec![
                ConceptReference {
                    id: "column_analysis".into(),
                    label: "Column Analysis".into(),
                    used_for: "Detecting partial column matches and finding positions that complete or advance clears.".into(),
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
        // Flip cards in the same column to maximize chance of finding an early match.
        // Pick a random column, take as many flips there as possible, then spill into
        // another random column if needed.
        let mut result = Vec::with_capacity(count);
        let mut cols: Vec<usize> = (0..view.num_cols).collect();
        cols.shuffle(rng);

        for &col in &cols {
            if result.len() >= count {
                break;
            }
            let base = col * view.num_rows;
            for row in 0..view.num_rows {
                if result.len() >= count {
                    break;
                }
                let idx = base + row;
                if matches!(view.my_board[idx], VisibleSlot::Hidden) {
                    result.push(idx);
                }
            }
        }

        result
    }

    fn choose_draw(&self, view: &StrategyView, _rng: &mut dyn RngCore) -> DrawChoice {
        if let Some(discard_val) = view.discard_top(0) {
            // Check if discard value completes or advances a column clear
            if find_clear_target(view, discard_val).is_some() {
                return DrawChoice::DrawFromDiscard(0);
            }

            // Greedy fallback: take if it's ≤ 0 or lower than highest revealed
            let highest = highest_revealed_value(&view.my_board);
            if discard_val <= 0 || highest.is_some_and(|h| discard_val < h) {
                return DrawChoice::DrawFromDiscard(0);
            }
        }
        DrawChoice::DrawFromDeck
    }

    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> DeckDrawAction {
        // Priority 1: Does this card complete a column clear?
        if let Some(pos) = find_completing_position(view, drawn_card) {
            return DeckDrawAction::Keep(pos);
        }

        // Priority 2: Does this card advance a partial column (place in column with 1+ match)?
        if let Some(pos) = find_advancing_position(view, drawn_card) {
            return DeckDrawAction::Keep(pos);
        }

        // Priority 3: Greedy — replace highest revealed if improvement
        let highest = highest_revealed_value(&view.my_board);
        if highest.is_some_and(|h| drawn_card < h) {
            let pos = position_of_highest_revealed(&view.my_board);
            return DeckDrawAction::Keep(pos);
        }

        // Priority 4: Higher tolerance — keep if card value ≤ 8 and there's a column
        // where it could pair with an existing match (even just 1 matching card)
        if drawn_card <= 8
            && let Some(pos) = find_build_position(view, drawn_card)
        {
            return DeckDrawAction::Keep(pos);
        }

        // Discard and flip
        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        if !hidden.is_empty() {
            let pos = pick_hidden_in_partial_column(view, &hidden, rng);
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden cards — must keep
        let pos = position_of_highest_revealed(&view.my_board);
        DeckDrawAction::Keep(pos)
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        // Priority 1: Complete a column clear
        if let Some(pos) = find_completing_position(view, drawn_card) {
            return pos;
        }

        // Priority 2: Advance a partial column
        if let Some(pos) = find_advancing_position(view, drawn_card) {
            return pos;
        }

        // Priority 3: Greedy — replace highest revealed
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

        // Replace a hidden slot
        if let Some(pos) = view
            .my_board
            .iter()
            .position(|s| matches!(s, VisibleSlot::Hidden))
        {
            return pos;
        }

        // Last resort
        position_of_highest_revealed(&view.my_board)
    }
}

/// Find a position where placing `value` would complete a column clear
/// (all slots in the column become revealed with the same value).
fn find_completing_position(view: &StrategyView, value: CardValue) -> Option<usize> {
    let cols = column_analysis(view);
    for col_info in &cols {
        if col_info.cleared_count > 0 {
            continue;
        }
        let num_rows = view.num_rows;
        // Need all but 1 slot to already be revealed with this value
        if let Some((match_val, match_count)) = col_info.partial_match
            && match_val == value
            && match_count == num_rows - 1
        {
            // The remaining slot must be hidden or a different revealed value
            // Find the non-matching slot in this column
            for &idx in &col_info.indices {
                match view.my_board[idx] {
                    VisibleSlot::Revealed(v) if v != value => return Some(idx),
                    VisibleSlot::Hidden => return Some(idx),
                    _ => {}
                }
            }
        }
        // Also check: all revealed in column match value and there's exactly 1 hidden
        if col_info.hidden_indices.len() == 1
            && col_info.revealed_values.iter().all(|(_, v)| *v == value)
            && col_info.revealed_values.len() == num_rows - 1
        {
            return Some(col_info.hidden_indices[0]);
        }
    }
    None
}

/// Find a position where placing `value` advances a partial column match.
/// Returns a position in a column that already has 1+ matching cards of the same value.
fn find_advancing_position(view: &StrategyView, value: CardValue) -> Option<usize> {
    let cols = column_analysis(view);
    let mut best: Option<(usize, usize)> = None; // (position, match_count)

    for col_info in &cols {
        if col_info.cleared_count > 0 {
            continue;
        }
        // Count how many revealed cards in this column match the value
        let matching = col_info
            .revealed_values
            .iter()
            .filter(|(_, v)| *v == value)
            .count();

        if matching == 0 {
            continue;
        }

        // Find a slot to place the card: prefer replacing a non-matching revealed card,
        // then a hidden slot
        let target = col_info
            .revealed_values
            .iter()
            .filter(|(_, v)| *v != value)
            .max_by_key(|(_, v)| *v) // replace highest non-matching
            .map(|(idx, _)| *idx)
            .or_else(|| col_info.hidden_indices.first().copied());

        if let Some(pos) = target
            && (best.is_none() || matching > best.unwrap().1)
        {
            best = Some((pos, matching));
        }
    }

    best.map(|(pos, _)| pos)
}

/// Find a position to build toward a column clear: place in a column that has
/// exactly 1 matching card and has hidden slots (early-game building).
fn find_build_position(view: &StrategyView, value: CardValue) -> Option<usize> {
    let cols = column_analysis(view);
    for col_info in &cols {
        if col_info.cleared_count > 0 {
            continue;
        }
        let matching = col_info
            .revealed_values
            .iter()
            .filter(|(_, v)| *v == value)
            .count();

        if matching >= 1 && !col_info.hidden_indices.is_empty() {
            // Place on a hidden slot in this column
            return Some(col_info.hidden_indices[0]);
        }
    }
    None
}

/// Pick a hidden slot to flip, preferring columns with partial matches.
fn pick_hidden_in_partial_column(
    view: &StrategyView,
    hidden: &[usize],
    rng: &mut dyn RngCore,
) -> usize {
    let cols = column_analysis(view);
    for col_info in &cols {
        if col_info.partial_match.is_some() {
            for &h in &col_info.hidden_indices {
                if hidden.contains(&h) {
                    return h;
                }
            }
        }
    }
    *hidden.choose(rng).unwrap()
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

/// Find a clear target: a column where placing `value` either completes or advances a match.
/// Returns the target position if found.
fn find_clear_target(view: &StrategyView, value: CardValue) -> Option<usize> {
    find_completing_position(view, value).or_else(|| find_advancing_position(view, value))
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
            opponent_boards: vec![],
            opponent_indices: vec![],
            discard_piles: vec![vec![3]],
            deck_remaining: 100,
            cumulative_scores: vec![0],
            is_final_turn: false,
        }
    }

    #[test]
    fn takes_high_value_to_complete_column_clear() {
        let board = vec![
            // col 0: two 10s and a hidden
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(10),
            VisibleSlot::Hidden,
            // col 1-3: hidden
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
        let mut view = make_view(board);
        view.discard_piles = vec![vec![10]];

        let strategy = ClearerStrategy;
        let mut rng = rand::rng();

        // Should take the 10 from discard to complete the column clear
        let draw = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(draw, DrawChoice::DrawFromDiscard(0)));
    }

    #[test]
    fn places_card_to_advance_column_match() {
        let board = vec![
            // col 0: one 7 and two hidden
            VisibleSlot::Revealed(7),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            // col 1-3: various
            VisibleSlot::Revealed(3),
            VisibleSlot::Revealed(4),
            VisibleSlot::Revealed(5),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view(board);
        let strategy = ClearerStrategy;
        let mut rng = rand::rng();

        // Drew a 7 from deck — should place in col 0 to build toward a clear
        let action = strategy.choose_deck_draw_action(&view, 7, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                // Should be in column 0 (indices 0, 1, 2)
                assert!(pos <= 2, "Should place 7 in column 0 (pos={pos})");
            }
            _ => panic!("Should keep the 7 to advance column match"),
        }
    }

    #[test]
    fn tolerates_high_value_for_clear() {
        let board = vec![
            // col 0: two 9s and a hidden
            VisibleSlot::Revealed(9),
            VisibleSlot::Revealed(9),
            VisibleSlot::Hidden,
            // col 1-3: low values
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view(board);
        let strategy = ClearerStrategy;
        let mut rng = rand::rng();

        // Drew a 9 — should keep it to complete the column clear
        let action = strategy.choose_deck_draw_action(&view, 9, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert_eq!(pos, 2, "Should place 9 in the hidden slot of column 0");
            }
            _ => panic!("Should keep the 9 to complete column clear"),
        }
    }
}
