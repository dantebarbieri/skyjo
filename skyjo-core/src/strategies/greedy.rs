use rand::seq::SliceRandom;
use rand::RngCore;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{DeckDrawAction, DrawChoice, Strategy, StrategyView};

/// A simple greedy strategy:
/// - Takes from discard if the card is ≤ 0 or lower than the highest revealed card.
/// - Keeps deck draws ≤ 4, placing over the highest revealed card (or a hidden slot).
/// - Otherwise discards and flips a hidden card.
/// - Initial flips are random (no information to optimize on).
pub struct GreedyStrategy;

impl Strategy for GreedyStrategy {
    fn name(&self) -> &str {
        "Greedy"
    }

    fn choose_initial_flips(
        &self,
        view: &StrategyView,
        count: usize,
        rng: &mut dyn RngCore,
    ) -> Vec<usize> {
        // No information to optimize on, pick random
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
        // Take from discard if the top card is ≤ 0 or lower than our highest revealed card
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
        if drawn_card <= 4 {
            // Keep: place over the highest revealed card, or a hidden slot
            let pos = best_replacement_position(view, drawn_card);
            DeckDrawAction::Keep(pos)
        } else {
            // Discard and flip a hidden card
            let hidden: Vec<usize> = view
                .my_board
                .iter()
                .enumerate()
                .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
                .map(|(i, _)| i)
                .collect();
            if hidden.is_empty() {
                // No hidden cards left, must keep — place over highest
                let pos = best_replacement_position(view, drawn_card);
                DeckDrawAction::Keep(pos)
            } else {
                let &pos = hidden.choose(rng).unwrap();
                DeckDrawAction::DiscardAndFlip(pos)
            }
        }
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

/// Find the highest revealed card value on the board.
fn highest_revealed_value(board: &[VisibleSlot]) -> Option<CardValue> {
    board
        .iter()
        .filter_map(|s| match s {
            VisibleSlot::Revealed(v) => Some(*v),
            _ => None,
        })
        .max()
}

/// Find the best position to place a new card:
/// - If there's a revealed card with a higher value, replace the highest one.
/// - Otherwise, replace a hidden card (unknown value, worth replacing).
/// - As a last resort, replace the highest revealed card.
fn best_replacement_position(view: &StrategyView, _drawn_card: CardValue) -> usize {
    let board = &view.my_board;

    // Find the highest revealed card position
    let highest_revealed_pos = board
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            VisibleSlot::Revealed(v) => Some((i, *v)),
            _ => None,
        })
        .max_by_key(|(_, v)| *v);

    // Prefer replacing the highest revealed card
    if let Some((pos, _)) = highest_revealed_pos {
        return pos;
    }

    // Otherwise pick the first hidden slot
    board
        .iter()
        .position(|s| matches!(s, VisibleSlot::Hidden))
        .unwrap_or(0)
}
