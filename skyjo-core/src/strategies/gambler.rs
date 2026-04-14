use rand::RngCore;
use rand::prelude::SliceRandom;
use rand::seq::IndexedRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, ConceptReference, DecisionLogic, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, PriorityRule, Strategy, StrategyDescription, StrategyView,
};

use super::common::column_analysis;

/// A risk-loving strategy that always draws from the deck and aggressively
/// replaces hidden cards. Chases column clears on long-shot odds, tolerating
/// high-value cards if they match a partial column.
pub struct GamblerStrategy;

impl Strategy for GamblerStrategy {
    fn name(&self) -> &str {
        "Gambler"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Gambler".into(),
            summary: "Always takes risks. Draws exclusively from the deck, aggressively replaces hidden cards, and chases column clears on long-shot odds.".into(),
            complexity: Complexity::Low,
            strengths: vec![
                "Can get lucky with high-variance plays".into(),
                "Aggressively pursues column clears regardless of card value".into(),
                "Reveals hidden cards quickly by always drawing from deck".into(),
            ],
            weaknesses: vec![
                "Ignores known-good cards on the discard pile".into(),
                "No risk assessment — treats all hidden replacements equally".into(),
                "Chases unlikely column clears that may never complete".into(),
            ],
            phases: vec![
                PhaseDescription {
                    phase: Phase::InitialFlips,
                    label: "Initial Flips".into(),
                    logic: DecisionLogic::Simple {
                        text: "Random — picks hidden positions at random with no optimization."
                            .into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::ChooseDraw,
                    label: "Draw Decision".into(),
                    logic: DecisionLogic::Simple {
                        text: "Always draws from the deck. Never takes from the discard pile."
                            .into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::DeckDrawAction,
                    label: "After Drawing from Deck".into(),
                    logic: DecisionLogic::PriorityList {
                        rules: vec![
                            PriorityRule {
                                condition: "Card completes a column clear (fills the last slot)"
                                    .into(),
                                action: "Keep it — place in the completing position".into(),
                                detail: Some(
                                    "Highest priority: the entire column is removed and scores 0."
                                        .into(),
                                ),
                            },
                            PriorityRule {
                                condition:
                                    "Card matches a partial column (1+ matching cards in a column)"
                                        .into(),
                                action: "Keep it — place in that column to chase the clear".into(),
                                detail: Some(
                                    "Always chases the clear regardless of card value.".into(),
                                ),
                            },
                            PriorityRule {
                                condition: "Card < 10 and hidden cards remain".into(),
                                action: "Keep it — replace a random hidden card".into(),
                                detail: Some(
                                    "Gambles that replacing a hidden card is an improvement.".into(),
                                ),
                            },
                            PriorityRule {
                                condition: "Card ≥ 10 and hidden cards remain".into(),
                                action: "Discard and flip a random hidden card".into(),
                                detail: Some(
                                    "Only rejects the very worst draws (10, 11, 12).".into(),
                                ),
                            },
                            PriorityRule {
                                condition: "No hidden cards left".into(),
                                action: "Keep it — replace the highest revealed card".into(),
                                detail: Some(
                                    "Forced to keep since there's nothing to flip.".into(),
                                ),
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
                                condition: "Hidden cards remain".into(),
                                action: "Replace a random hidden card".into(),
                                detail: Some("Gambles on the replacement being an improvement.".into()),
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
            concepts: vec![ConceptReference {
                id: "column_analysis".into(),
                label: "Column Analysis".into(),
                used_for:
                    "Detecting partial column matches to chase long-shot column clears.".into(),
            }],
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

    fn choose_draw(&self, _view: &StrategyView, _rng: &mut dyn RngCore) -> DrawChoice {
        DrawChoice::DrawFromDeck
    }

    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> DeckDrawAction {
        let cols = column_analysis(view);

        // Priority 1: Does this card complete a column clear?
        if let Some(pos) = find_completing_position(view, &cols, drawn_card) {
            return DeckDrawAction::Keep(pos);
        }

        // Priority 2: Does this card match a partial column? Chase the clear regardless of value.
        if let Some(pos) = find_advancing_position(view, &cols, drawn_card) {
            return DeckDrawAction::Keep(pos);
        }

        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        // Priority 3: If card < 10, keep and replace a random hidden card
        if drawn_card < 10 && !hidden.is_empty() {
            let &pos = hidden.choose(rng).unwrap();
            return DeckDrawAction::Keep(pos);
        }

        // Priority 4: If card >= 10, discard and flip a random hidden card
        if !hidden.is_empty() {
            let &pos = hidden.choose(rng).unwrap();
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // Priority 5: No hidden cards — replace highest revealed
        let pos = position_of_highest_revealed(&view.my_board);
        DeckDrawAction::Keep(pos)
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        _drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> usize {
        // Replace a random hidden card if any exist
        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        if !hidden.is_empty() {
            return *hidden.choose(rng).unwrap();
        }

        // No hidden cards — replace highest revealed
        position_of_highest_revealed(&view.my_board)
    }
}

/// Find a position where placing `value` would complete a column clear
/// (all slots in the column become revealed with the same value).
fn find_completing_position(
    view: &StrategyView,
    cols: &[super::common::ColumnInfo],
    value: CardValue,
) -> Option<usize> {
    let num_rows = view.num_rows;
    for col_info in cols {
        if col_info.cleared_count > 0 {
            continue;
        }
        // Need all but 1 slot to already be revealed with this value
        if let Some((match_val, match_count)) = col_info.partial_match
            && match_val == value
            && match_count == num_rows - 1
        {
            // Find the non-matching slot in this column
            for &idx in &col_info.indices {
                match view.my_board[idx] {
                    VisibleSlot::Revealed(v) if v != value => return Some(idx),
                    VisibleSlot::Hidden => return Some(idx),
                    _ => {}
                }
            }
        }
        // Also check: all revealed match value and there's exactly 1 hidden
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
/// Returns a position in the column that already has 1+ matching cards.
fn find_advancing_position(
    _view: &StrategyView,
    cols: &[super::common::ColumnInfo],
    value: CardValue,
) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None; // (position, match_count)

    for col_info in cols {
        if col_info.cleared_count > 0 {
            continue;
        }
        let matching = col_info
            .revealed_values
            .iter()
            .filter(|(_, v)| *v == value)
            .count();

        if matching == 0 {
            continue;
        }

        // Find a slot to place: prefer replacing a non-matching revealed card,
        // then a hidden slot
        let target = col_info
            .revealed_values
            .iter()
            .filter(|(_, v)| *v != value)
            .max_by_key(|(_, v)| *v)
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
    fn always_draws_from_deck() {
        let board = vec![VisibleSlot::Hidden; 12];
        let mut view = make_view(board);
        view.discard_piles = vec![vec![-2]]; // Even a great discard card is ignored
        let strategy = GamblerStrategy;
        let mut rng = rand::rng();
        let draw = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(draw, DrawChoice::DrawFromDeck));
    }

    #[test]
    fn keeps_low_card_on_hidden() {
        let board = vec![
            VisibleSlot::Revealed(5),
            VisibleSlot::Revealed(3),
            VisibleSlot::Revealed(4),
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
        let strategy = GamblerStrategy;
        let mut rng = rand::rng();

        // Card value 2 (< 10) should be kept on a hidden card
        let action = strategy.choose_deck_draw_action(&view, 2, &mut rng);
        assert!(matches!(action, DeckDrawAction::Keep(_)));
    }

    #[test]
    fn discards_high_card_and_flips() {
        let board = vec![
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Revealed(4),
            VisibleSlot::Revealed(5),
            VisibleSlot::Revealed(6),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view(board);
        let strategy = GamblerStrategy;
        let mut rng = rand::rng();

        // Card value 11 (>= 10) should be discarded and flip a hidden
        let action = strategy.choose_deck_draw_action(&view, 11, &mut rng);
        assert!(matches!(action, DeckDrawAction::DiscardAndFlip(_)));
    }

    #[test]
    fn chases_column_clear_with_high_card() {
        let board = vec![
            // col 0: two 10s and a hidden
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(10),
            VisibleSlot::Hidden,
            // col 1-3: low values
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view(board);
        let strategy = GamblerStrategy;
        let mut rng = rand::rng();

        // Drew a 10 — should keep to complete column clear even though >= 10
        let action = strategy.choose_deck_draw_action(&view, 10, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert_eq!(pos, 2, "Should place 10 in the hidden slot of column 0");
            }
            _ => panic!("Should keep the 10 to complete column clear"),
        }
    }

    #[test]
    fn chases_partial_column_regardless_of_value() {
        let board = vec![
            // col 0: one 11 and two hidden
            VisibleSlot::Revealed(11),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            // col 1-3: various
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let view = make_view(board);
        let strategy = GamblerStrategy;
        let mut rng = rand::rng();

        // Drew an 11 — should place in col 0 to chase the clear
        let action = strategy.choose_deck_draw_action(&view, 11, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert!(pos <= 2, "Should place 11 in column 0 (pos={pos})");
            }
            _ => panic!("Should keep the 11 to chase column clear"),
        }
    }

    #[test]
    fn no_hidden_replaces_highest_revealed() {
        let board = vec![
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(8),
            VisibleSlot::Revealed(3),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(4),
            VisibleSlot::Revealed(5),
            VisibleSlot::Revealed(6),
            VisibleSlot::Revealed(7),
            VisibleSlot::Revealed(0),
            VisibleSlot::Revealed(-1),
            VisibleSlot::Revealed(-2),
            VisibleSlot::Revealed(9),
        ];
        let view = make_view(board);
        let strategy = GamblerStrategy;
        let mut rng = rand::rng();

        // No hidden cards and card >= 10 — must replace highest revealed (9 at index 11)
        let action = strategy.choose_deck_draw_action(&view, 12, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert_eq!(
                    pos, 11,
                    "Should replace highest revealed card (9 at index 11)"
                );
            }
            _ => panic!("Should keep since no hidden cards to flip"),
        }
    }
}
