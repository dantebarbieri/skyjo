use rand::RngCore;
use rand::prelude::SliceRandom;
use rand::seq::IndexedRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, DecisionLogic, DeckDrawAction, DrawChoice, Phase, PhaseDescription, Strategy,
    StrategyDescription, StrategyView,
};

/// Completely random strategy — all decisions are uniformly random.
pub struct RandomStrategy;

impl Strategy for RandomStrategy {
    fn name(&self) -> &str {
        "Random"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Random".into(),
            summary: "Makes every decision by pure chance. Each valid option has equal probability of being chosen.".into(),
            complexity: Complexity::Trivial,
            strengths: vec!["Completely unpredictable".into()],
            weaknesses: vec![
                "No strategy at all".into(),
                "Will keep high cards and discard low ones just as often as the reverse".into(),
            ],
            phases: vec![
                PhaseDescription {
                    phase: Phase::InitialFlips,
                    label: "Initial Flips".into(),
                    logic: DecisionLogic::Simple {
                        text: "Pick random hidden positions to flip.".into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::ChooseDraw,
                    label: "Draw Decision".into(),
                    logic: DecisionLogic::Simple {
                        text: "50/50 chance between drawing from the deck and taking from the discard pile.".into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::DeckDrawAction,
                    label: "After Drawing from Deck".into(),
                    logic: DecisionLogic::Simple {
                        text: "50/50 chance between keeping the card (placed at a random position) and discarding it (flipping a random hidden card).".into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::DiscardDrawPlacement,
                    label: "After Drawing from Discard".into(),
                    logic: DecisionLogic::Simple {
                        text: "Place the card at a random non-cleared position.".into(),
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
