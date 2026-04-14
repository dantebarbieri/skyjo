use rand::RngCore;
use rand::prelude::SliceRandom;
use rand::seq::IndexedRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, ConceptReference, DecisionLogic, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, PriorityRule, Strategy, StrategyDescription, StrategyView,
};

use super::common::column_analysis;

/// Danger level based on cumulative score relative to the 100-point elimination threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DangerLevel {
    /// Cumulative score < 50: play aggressively (greedy-style).
    Low,
    /// Cumulative score >= 70: play cautiously.
    High,
    /// Cumulative score >= 85: play ultra-conservatively to survive.
    Critical,
}

fn danger_level(cumulative: i32) -> DangerLevel {
    if cumulative >= 85 {
        DangerLevel::Critical
    } else if cumulative >= 70 {
        DangerLevel::High
    } else {
        DangerLevel::Low
    }
}

/// A survival-oriented strategy that adapts its risk tolerance based on
/// cumulative score relative to the 100-point elimination threshold.
///
/// - **Low danger** (< 50): Plays like Greedy — takes good cards, replaces bad ones.
/// - **High danger** (>= 70): Conservative — only takes very low discards, cautious replacements.
/// - **Critical danger** (>= 85): Ultra-conservative — only takes non-positive cards, avoids risk.
pub struct SurvivorStrategy;

impl Strategy for SurvivorStrategy {
    fn name(&self) -> &str {
        "Survivor"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Survivor".into(),
            summary: "Adapts risk tolerance based on cumulative score. Plays aggressively when safe (< 50), cautiously when threatened (>= 70), and ultra-conservatively when near elimination (>= 85).".into(),
            complexity: Complexity::Medium,
            strengths: vec![
                "Adapts to game state — aggressive early, careful late".into(),
                "Avoids catastrophic rounds when close to 100".into(),
                "Effective in multi-round games where survival matters".into(),
            ],
            weaknesses: vec![
                "No card counting or probability analysis".into(),
                "Rigid threshold-based logic, not smooth adaptation".into(),
                "Doesn't consider opponents' scores or positions".into(),
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
                                condition: "Low danger: discard top <= 0 OR < highest revealed".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Greedy-style: take any card that improves or maintains the board.".into()),
                            },
                            PriorityRule {
                                condition: "High danger: discard top <= 3".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Only takes low-value cards to minimize round score.".into()),
                            },
                            PriorityRule {
                                condition: "Critical danger: discard top <= 0".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Only takes non-positive cards — any positive card is too risky.".into()),
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
                                condition: "Low danger: drawn < highest revealed".into(),
                                action: "Keep it — replace the highest revealed card".into(),
                                detail: Some("Greedy-style: only keeps strict improvements.".into()),
                            },
                            PriorityRule {
                                condition: "High danger: drawn <= 0 OR replaces revealed >= 8".into(),
                                action: "Keep it — replace the target card".into(),
                                detail: Some("Only keeps very safe cards or replaces very high values.".into()),
                            },
                            PriorityRule {
                                condition: "Critical danger: drawn <= 0 OR replaces revealed >= 10".into(),
                                action: "Keep it — replace the target card".into(),
                                detail: Some("Ultra-conservative: only guaranteed improvements.".into()),
                            },
                            PriorityRule {
                                condition: "Hidden cards remain".into(),
                                action: "Discard and flip a hidden card (prefer columns with partial matches)".into(),
                                detail: None,
                            },
                            PriorityRule {
                                condition: "No hidden cards left".into(),
                                action: "Keep it — replace the highest revealed card".into(),
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
                                condition: "Low danger: revealed card > drawn".into(),
                                action: "Replace the highest such revealed card".into(),
                                detail: Some("Standard greedy replacement.".into()),
                            },
                            PriorityRule {
                                condition: "High danger: revealed card >= drawn + 5".into(),
                                action: "Replace the highest such revealed card".into(),
                                detail: Some("Only replaces when the improvement is significant.".into()),
                            },
                            PriorityRule {
                                condition: "Critical danger: revealed card >= drawn + 8".into(),
                                action: "Replace the highest such revealed card".into(),
                                detail: Some("Ultra-conservative: needs a large gap to justify replacement.".into()),
                            },
                            PriorityRule {
                                condition: "No qualifying revealed card".into(),
                                action: "Replace a hidden card".into(),
                                detail: None,
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
            concepts: vec![
                ConceptReference {
                    id: "column_analysis".into(),
                    label: "Column Analysis".into(),
                    used_for: "When flipping a hidden card, prefers columns that already have partial matches to increase column-clear chances.".into(),
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
        let danger = danger_level(view.cumulative_scores[view.my_index]);

        if let Some(discard_val) = view.discard_top(0) {
            match danger {
                DangerLevel::Low => {
                    let highest_revealed = highest_revealed_value(&view.my_board);
                    if discard_val <= 0 || highest_revealed.is_some_and(|h| discard_val < h) {
                        return DrawChoice::DrawFromDiscard(0);
                    }
                }
                DangerLevel::High => {
                    if discard_val <= 3 {
                        return DrawChoice::DrawFromDiscard(0);
                    }
                }
                DangerLevel::Critical => {
                    if discard_val <= 0 {
                        return DrawChoice::DrawFromDiscard(0);
                    }
                }
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
        let danger = danger_level(view.cumulative_scores[view.my_index]);

        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        let highest = highest_revealed_with_pos(&view.my_board);

        let should_keep = match danger {
            DangerLevel::Low => {
                // Greedy: keep if drawn < highest revealed
                highest.is_some_and(|(_, h)| drawn_card < h)
            }
            DangerLevel::High => {
                // Keep only if drawn <= 0 or replaces a revealed card >= 8
                drawn_card <= 0 || highest_revealed_at_least(&view.my_board, 8).is_some()
            }
            DangerLevel::Critical => {
                // Keep only if drawn <= 0 or replaces a revealed card >= 10
                drawn_card <= 0 || highest_revealed_at_least(&view.my_board, 10).is_some()
            }
        };

        if should_keep {
            // Find the best position to place the card
            let pos = match danger {
                DangerLevel::Low => {
                    // Replace highest revealed card (greedy)
                    highest.map(|(pos, _)| pos).unwrap_or(0)
                }
                DangerLevel::High => {
                    // If drawn <= 0, replace highest revealed; otherwise replace the card >= 8
                    if drawn_card <= 0 {
                        highest.map(|(pos, _)| pos).unwrap_or(0)
                    } else {
                        highest_revealed_at_least(&view.my_board, 8)
                            .map(|(pos, _)| pos)
                            .unwrap_or(0)
                    }
                }
                DangerLevel::Critical => {
                    if drawn_card <= 0 {
                        highest.map(|(pos, _)| pos).unwrap_or(0)
                    } else {
                        highest_revealed_at_least(&view.my_board, 10)
                            .map(|(pos, _)| pos)
                            .unwrap_or(0)
                    }
                }
            };
            return DeckDrawAction::Keep(pos);
        }

        // Discard and flip a hidden card
        if !hidden.is_empty() {
            let pos = pick_hidden_for_column_progress(view, &hidden, rng);
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden cards left — must keep; place over highest revealed
        if let Some((pos, _)) = highest {
            return DeckDrawAction::Keep(pos);
        }

        // Fallback
        DeckDrawAction::Keep(0)
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        let danger = danger_level(view.cumulative_scores[view.my_index]);

        let min_diff = match danger {
            DangerLevel::Low => 1,      // any improvement
            DangerLevel::High => 5,     // significant improvement only
            DangerLevel::Critical => 8, // large improvement only
        };

        // Find revealed cards that exceed the drawn card by at least min_diff
        let best_revealed = view
            .my_board
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                VisibleSlot::Revealed(v) if (*v as i32 - drawn_card as i32) >= min_diff => {
                    Some((i, *v))
                }
                _ => None,
            })
            .max_by_key(|(_, v)| *v);

        if let Some((pos, _)) = best_revealed {
            return pos;
        }

        // No qualifying revealed card — replace a hidden card
        if let Some(pos) = view
            .my_board
            .iter()
            .position(|s| matches!(s, VisibleSlot::Hidden))
        {
            return pos;
        }

        // No hidden cards left — replace the highest revealed card
        position_of_highest_revealed(&view.my_board)
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

fn highest_revealed_with_pos(board: &[VisibleSlot]) -> Option<(usize, CardValue)> {
    board
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            VisibleSlot::Revealed(v) => Some((i, *v)),
            _ => None,
        })
        .max_by_key(|(_, v)| *v)
}

/// Find the highest revealed card with value >= threshold, returning (position, value).
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

/// Pick a hidden slot to flip, preferring columns with partial matches.
fn pick_hidden_for_column_progress(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view_with_score(
        my_board: Vec<VisibleSlot>,
        cumulative: i32,
        discard_top: Option<CardValue>,
    ) -> StrategyView {
        let discard = if let Some(v) = discard_top {
            vec![v]
        } else {
            vec![]
        };
        StrategyView {
            my_index: 0,
            my_board,
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![vec![VisibleSlot::Hidden; 12]],
            opponent_indices: vec![1],
            discard_piles: vec![discard],
            deck_remaining: 100,
            cumulative_scores: vec![cumulative, 0],
            is_final_turn: false,
        }
    }

    #[test]
    fn test_danger_levels() {
        assert_eq!(danger_level(0), DangerLevel::Low);
        assert_eq!(danger_level(49), DangerLevel::Low);
        assert_eq!(danger_level(70), DangerLevel::High);
        assert_eq!(danger_level(84), DangerLevel::High);
        assert_eq!(danger_level(85), DangerLevel::Critical);
        assert_eq!(danger_level(99), DangerLevel::Critical);
    }

    #[test]
    fn low_danger_takes_discard_below_highest_revealed() {
        let board = vec![
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 20, Some(5));
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // Low danger, discard=5 < highest revealed=10 -> take discard
        let choice = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(choice, DrawChoice::DrawFromDiscard(0)));
    }

    #[test]
    fn high_danger_rejects_moderate_discard() {
        let board = vec![
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 75, Some(5));
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // High danger, discard=5 > 3 threshold -> draw from deck
        let choice = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(choice, DrawChoice::DrawFromDeck));
    }

    #[test]
    fn high_danger_takes_low_discard() {
        let board = vec![
            VisibleSlot::Revealed(10),
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
        let view = make_view_with_score(board, 75, Some(2));
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // High danger, discard=2 <= 3 threshold -> take discard
        let choice = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(choice, DrawChoice::DrawFromDiscard(0)));
    }

    #[test]
    fn critical_danger_only_takes_non_positive_discard() {
        let board = vec![
            VisibleSlot::Revealed(10),
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
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // Critical, discard=3 > 0 -> draw from deck
        let view = make_view_with_score(board.clone(), 90, Some(3));
        let choice = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(choice, DrawChoice::DrawFromDeck));

        // Critical, discard=0 <= 0 -> take discard
        let view = make_view_with_score(board.clone(), 90, Some(0));
        let choice = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(choice, DrawChoice::DrawFromDiscard(0)));

        // Critical, discard=-2 <= 0 -> take discard
        let view = make_view_with_score(board, 90, Some(-2));
        let choice = strategy.choose_draw(&view, &mut rng);
        assert!(matches!(choice, DrawChoice::DrawFromDiscard(0)));
    }

    #[test]
    fn low_danger_keeps_improving_deck_draw() {
        let board = vec![
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 20, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // Low danger, drawn=5 < highest=10 -> keep, replace position 0 (the 10)
        let action = strategy.choose_deck_draw_action(&view, 5, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => assert_eq!(pos, 0),
            _ => panic!("Should keep the drawn card"),
        }
    }

    #[test]
    fn high_danger_discards_moderate_deck_draw() {
        let board = vec![
            VisibleSlot::Revealed(6),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 75, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // High danger, drawn=5, no revealed >= 8, drawn > 0 -> discard and flip
        let action = strategy.choose_deck_draw_action(&view, 5, &mut rng);
        assert!(matches!(action, DeckDrawAction::DiscardAndFlip(_)));
    }

    #[test]
    fn high_danger_keeps_non_positive_deck_draw() {
        let board = vec![
            VisibleSlot::Revealed(6),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 75, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // High danger, drawn=-1 <= 0 -> keep, replace highest (pos 0, value 6)
        let action = strategy.choose_deck_draw_action(&view, -1, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => assert_eq!(pos, 0),
            _ => panic!("Should keep non-positive card at high danger"),
        }
    }

    #[test]
    fn critical_danger_keeps_replacing_very_high_card() {
        let board = vec![
            VisibleSlot::Revealed(11),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 90, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // Critical, drawn=5, revealed 11 >= 10 -> keep, replace the 11
        let action = strategy.choose_deck_draw_action(&view, 5, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => assert_eq!(pos, 0),
            _ => panic!("Should keep to replace the 11 at critical danger"),
        }
    }

    #[test]
    fn critical_danger_discards_when_no_very_high_card() {
        let board = vec![
            VisibleSlot::Revealed(8),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 90, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // Critical, drawn=5, highest revealed=8 < 10 threshold, drawn > 0 -> discard
        let action = strategy.choose_deck_draw_action(&view, 5, &mut rng);
        assert!(matches!(action, DeckDrawAction::DiscardAndFlip(_)));
    }

    #[test]
    fn low_danger_discard_placement_replaces_any_higher() {
        let board = vec![
            VisibleSlot::Revealed(6),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 20, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // Low danger, drawn=5 — 6 > 5 by >= 1 -> replace the 6 at pos 0
        let pos = strategy.choose_discard_draw_placement(&view, 5, &mut rng);
        assert_eq!(pos, 0);
    }

    #[test]
    fn high_danger_discard_placement_requires_large_gap() {
        let board = vec![
            VisibleSlot::Revealed(8),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 75, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // High danger, drawn=5, highest=8, diff=3 < 5 -> no qualifying revealed, replace hidden
        let pos = strategy.choose_discard_draw_placement(&view, 5, &mut rng);
        // Should be a hidden position (index 2-11)
        assert!(pos >= 2);
    }

    #[test]
    fn high_danger_discard_placement_replaces_when_gap_sufficient() {
        let board = vec![
            VisibleSlot::Revealed(11),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 75, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // High danger, drawn=5, 11-5=6 >= 5 -> replace the 11 at pos 0
        let pos = strategy.choose_discard_draw_placement(&view, 5, &mut rng);
        assert_eq!(pos, 0);
    }

    #[test]
    fn critical_danger_discard_placement_needs_huge_gap() {
        let board = vec![
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(3),
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
        let view = make_view_with_score(board, 90, None);
        let strategy = SurvivorStrategy;
        let mut rng = rand::rng();

        // Critical, drawn=5, 10-5=5 < 8 -> no qualifying, replace hidden
        let pos = strategy.choose_discard_draw_placement(&view, 5, &mut rng);
        assert!(pos >= 2);

        // Critical, drawn=2, 10-2=8 >= 8 -> replace the 10 at pos 0
        let pos = strategy.choose_discard_draw_placement(&view, 2, &mut rng);
        assert_eq!(pos, 0);
    }
}
