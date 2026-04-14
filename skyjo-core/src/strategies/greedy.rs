use rand::RngCore;
use rand::seq::SliceRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, DecisionLogic, DeckDrawAction, DrawChoice, Phase, PhaseDescription, PriorityRule,
    Strategy, StrategyDescription, StrategyView,
};

/// A greedy strategy:
/// - Takes from discard if the card is ≤ 0 or lower than the highest revealed card.
/// - Keeps deck draws only when they strictly improve the board (replace a
///   higher-valued revealed card). Otherwise discards and flips a hidden card
///   to make progress toward going out.
/// - Initial flips are random (no information to optimize on).
pub struct GreedyStrategy;

impl Strategy for GreedyStrategy {
    fn name(&self) -> &str {
        "Greedy"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Greedy".into(),
            summary: "Makes locally optimal choices by comparing card values. Takes known-good cards from the discard pile and only keeps deck draws that strictly improve the board.".into(),
            complexity: Complexity::Low,
            strengths: vec![
                "Simple and predictable".into(),
                "Never makes the board worse".into(),
            ],
            weaknesses: vec![
                "Ignores column-clear opportunities".into(),
                "No card counting or probability analysis".into(),
                "Doesn't consider what opponents need".into(),
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
                                detail: Some("Negative and zero cards always improve or maintain score.".into()),
                            },
                            PriorityRule {
                                condition: "Discard top < highest revealed card on board".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Guaranteed improvement by replacing the worst card.".into()),
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
                                condition: "Drawn card < highest revealed card".into(),
                                action: "Keep it — replace the highest revealed card".into(),
                                detail: Some("Only keeps the card when it's a strict improvement.".into()),
                            },
                            PriorityRule {
                                condition: "Hidden cards remain".into(),
                                action: "Discard the drawn card and flip a random hidden card".into(),
                                detail: Some("Makes progress toward revealing all cards and going out.".into()),
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
                                condition: "A revealed card is higher than drawn card".into(),
                                action: "Replace the highest such revealed card".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "No revealed card is higher".into(),
                                action: "Replace a hidden card".into(),
                                detail: Some("Reveals the position while placing the known card.".into()),
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
            let highest_revealed = highest_revealed_value(&view.my_board);
            if discard_val <= 0 || highest_revealed.is_some_and(|h| discard_val < h) {
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
        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        let highest = highest_revealed_value(&view.my_board);

        // Keep the card only if it strictly improves the board:
        // there must be a revealed card with a higher value to replace.
        if highest.is_some_and(|h| drawn_card < h) {
            let pos = position_of_highest_revealed(&view.my_board);
            return DeckDrawAction::Keep(pos);
        }

        // Otherwise, discard and flip a hidden card to make progress
        if !hidden.is_empty() {
            let &pos = hidden.choose(rng).unwrap();
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden cards left — must keep; place over highest revealed
        if let Some(pos) = highest.map(|_| position_of_highest_revealed(&view.my_board)) {
            return DeckDrawAction::Keep(pos);
        }

        // Fallback: all cleared, place at 0 (will error, but no valid move exists)
        DeckDrawAction::Keep(0)
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        best_replacement_position(view, drawn_card)
    }
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

/// Find the best position to place a card drawn from discard:
/// - Replace the highest revealed card if drawn_card is lower.
/// - Otherwise replace a hidden card.
/// - Last resort: replace the highest revealed card anyway.
fn best_replacement_position(view: &StrategyView, drawn_card: CardValue) -> usize {
    let board = &view.my_board;

    // If we have a revealed card higher than drawn_card, replace it
    let highest = board
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            VisibleSlot::Revealed(v) if *v > drawn_card => Some((i, *v)),
            _ => None,
        })
        .max_by_key(|(_, v)| *v);

    if let Some((pos, _)) = highest {
        return pos;
    }

    // Otherwise replace a hidden card (reveals via replacement)
    if let Some(pos) = board.iter().position(|s| matches!(s, VisibleSlot::Hidden)) {
        return pos;
    }

    // Last resort: replace the highest revealed card
    position_of_highest_revealed(board)
}
