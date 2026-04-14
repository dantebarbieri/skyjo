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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::StrategyView;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn name_returns_random() {
        assert_eq!(RandomStrategy.name(), "Random");
    }

    #[test]
    fn describe_returns_valid_description() {
        let desc = RandomStrategy.describe();
        assert_eq!(desc.name, "Random");
        assert!(!desc.summary.is_empty());
        assert_eq!(desc.complexity, Complexity::Trivial);
        assert!(!desc.strengths.is_empty());
        assert!(!desc.weaknesses.is_empty());
        assert_eq!(desc.phases.len(), 4);
    }

    fn make_view() -> StrategyView {
        StrategyView {
            my_index: 0,
            my_board: vec![
                VisibleSlot::Hidden,
                VisibleSlot::Revealed(5),
                VisibleSlot::Hidden,
                VisibleSlot::Revealed(10),
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
            ],
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![],
            opponent_indices: vec![],
            discard_piles: vec![vec![3]],
            deck_remaining: 100,
            cumulative_scores: vec![0, 0],
            is_final_turn: false,
        }
    }

    #[test]
    fn choose_initial_flips_returns_correct_count() {
        let view = make_view();
        let mut rng = StdRng::seed_from_u64(42);
        let flips = RandomStrategy.choose_initial_flips(&view, 2, &mut rng);
        assert_eq!(flips.len(), 2);
        assert_ne!(flips[0], flips[1]);
        // All flips should be at hidden positions
        for &pos in &flips {
            assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
        }
    }

    #[test]
    fn choose_draw_returns_valid_choice() {
        let view = make_view();
        let mut rng = StdRng::seed_from_u64(42);
        // Run multiple times to exercise both branches
        for seed in 0..20u64 {
            let mut r = StdRng::seed_from_u64(seed);
            let choice = RandomStrategy.choose_draw(&view, &mut r);
            match choice {
                DrawChoice::DrawFromDeck => {}
                DrawChoice::DrawFromDiscard(pile) => {
                    assert!(!view.discard_piles[pile].is_empty());
                }
            }
        }
        // Suppress unused warning
        let _ = rng.next_u32();
    }

    #[test]
    fn choose_deck_draw_action_returns_valid_action() {
        let view = make_view();
        let mut rng = StdRng::seed_from_u64(42);
        let action = RandomStrategy.choose_deck_draw_action(&view, 5, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert!(!matches!(view.my_board[pos], VisibleSlot::Cleared));
            }
            DeckDrawAction::DiscardAndFlip(pos) => {
                assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
            }
        }
    }
}
