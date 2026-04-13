use rand::RngCore;
use rand::seq::SliceRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{DeckDrawAction, DrawChoice, Strategy, StrategyView};

/// Completely random strategy — all decisions are uniformly random.
pub struct RandomStrategy;

impl Strategy for RandomStrategy {
    fn name(&self) -> &str {
        "Random"
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

    fn choose_draw(&self, view: &StrategyView, rng: &mut dyn RngCore) -> DrawChoice {
        // 50/50 between deck and discard (if discard available)
        let has_discard = view.discard_piles.iter().any(|p| !p.is_empty());
        if has_discard && random_bool(rng) {
            // Pick a random non-empty discard pile
            let non_empty: Vec<usize> = view
                .discard_piles
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.is_empty())
                .map(|(i, _)| i)
                .collect();
            let &pile = non_empty.choose(rng).unwrap();
            DrawChoice::DrawFromDiscard(pile)
        } else {
            DrawChoice::DrawFromDeck
        }
    }

    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        _drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> DeckDrawAction {
        let non_cleared: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| !matches!(s, VisibleSlot::Cleared))
            .map(|(i, _)| i)
            .collect();

        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        if !hidden.is_empty() && random_bool(rng) {
            // Discard and flip a random hidden card
            let &pos = hidden.choose(rng).unwrap();
            DeckDrawAction::DiscardAndFlip(pos)
        } else {
            // Keep and place at a random non-cleared position
            let &pos = non_cleared.choose(rng).unwrap();
            DeckDrawAction::Keep(pos)
        }
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        _drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> usize {
        let non_cleared: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| !matches!(s, VisibleSlot::Cleared))
            .map(|(i, _)| i)
            .collect();
        *non_cleared.choose(rng).unwrap()
    }
}

fn random_bool(rng: &mut dyn RngCore) -> bool {
    rng.next_u32() & 1 == 0
}
