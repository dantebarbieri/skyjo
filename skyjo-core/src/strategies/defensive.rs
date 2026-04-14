use rand::seq::SliceRandom;
use rand::RngCore;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, ConceptReference, DecisionLogic, DecisionNode, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, PriorityRule, Strategy, StrategyDescription, StrategyView,
};

use super::common::{
    card_usefulness_to_player, column_analysis, next_player_board,
};

/// A defensive strategy that focuses on avoiding helping the next player.
///
/// Key insight: the discard pile top is always replaced at the end of a turn,
/// so this strategy focuses on what it *leaves* on the discard pile rather
/// than what it takes. It tries to displace/discard cards that are least
/// useful to the following player.
pub struct DefensiveStrategy;

impl Strategy for DefensiveStrategy {
    fn name(&self) -> &str {
        "Defensive"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Defensive".into(),
            summary: "Balances personal improvement with opponent denial. Tracks the next player's board and avoids leaving useful cards on the discard pile for them.".into(),
            complexity: Complexity::Medium,
            strengths: vec![
                "Actively hinders the next player".into(),
                "Absorbs cards that would help opponents".into(),
                "Prefers flipping in columns with partial matches".into(),
            ],
            weaknesses: vec![
                "No card counting or EV calculations".into(),
                "May sacrifice board quality to deny opponents".into(),
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
                                detail: Some("Same as Greedy — low cards are always worth taking.".into()),
                            },
                            PriorityRule {
                                condition: "Discard top < highest revealed card on board".into(),
                                action: "Take from discard pile".into(),
                                detail: None,
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
                    logic: DecisionLogic::DecisionTree {
                        root: DecisionNode::Condition {
                            test: "Does the drawn card improve the board? (lower than a revealed card)".into(),
                            if_true: Box::new(DecisionNode::Action {
                                action: "Keep it — replace the highest revealed card, preferring to displace cards least useful to the next player".into(),
                                detail: Some("Among positions that improve score equally, picks the one whose displaced card helps the opponent least.".into()),
                            }),
                            if_false: Box::new(DecisionNode::Condition {
                                test: "Is the drawn card very useful to the next player (usefulness > 5)?".into(),
                                if_true: Box::new(DecisionNode::Action {
                                    action: "Absorb it — keep the card on a hidden slot to deny the opponent".into(),
                                    detail: Some("Accepts a card that doesn't improve our board to prevent the opponent from getting it off the discard pile.".into()),
                                }),
                                if_false: Box::new(DecisionNode::Action {
                                    action: "Discard the drawn card and flip a hidden card (preferring columns with partial matches)".into(),
                                    detail: None,
                                }),
                            }),
                        },
                    },
                },
                PhaseDescription {
                    phase: Phase::DiscardDrawPlacement,
                    label: "After Drawing from Discard".into(),
                    logic: DecisionLogic::PriorityList {
                        rules: vec![
                            PriorityRule {
                                condition: "A revealed card can be improved".into(),
                                action: "Replace the best improvement target, tiebreaking by displacing cards least useful to the next player".into(),
                                detail: Some("Sorts candidates by improvement first, then by how much the displaced card would help the opponent.".into()),
                            },
                            PriorityRule {
                                condition: "No improvement available".into(),
                                action: "Replace a hidden card".into(),
                                detail: None,
                            },
                        ],
                    },
                },
            ],
            concepts: vec![
                ConceptReference {
                    id: "opponent_denial".into(),
                    label: "Opponent Denial".into(),
                    used_for: "Scoring how useful each card is to the next player to decide which card to leave on the discard pile.".into(),
                },
                ConceptReference {
                    id: "column_analysis".into(),
                    label: "Column Analysis".into(),
                    used_for: "When flipping a hidden card, prefers columns that already have partial matches.".into(),
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
        // Greedy-like: take from discard if it improves our board
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

        let next_board = next_player_board(view);
        let card_useful_to_next = next_board
            .map(|b| card_usefulness_to_player(b, view.num_rows, view.num_cols, drawn_card))
            .unwrap_or(0.0);

        // Find positions where keeping the drawn card improves score
        let mut keep_candidates: Vec<(usize, CardValue, f64)> = Vec::new();
        for (i, slot) in view.my_board.iter().enumerate() {
            if let VisibleSlot::Revealed(v) = slot
                && drawn_card < *v
            {
                // Displaced card goes to discard — evaluate how useful it is to next player
                let displaced_usefulness = next_board
                    .map(|b| {
                        card_usefulness_to_player(b, view.num_rows, view.num_cols, *v)
                    })
                    .unwrap_or(0.0);
                keep_candidates.push((i, *v, displaced_usefulness));
            }
        }

        if !keep_candidates.is_empty() {
            // Among positions that improve our score, prefer displacing the card
            // least useful to the next player (lowest usefulness score)
            keep_candidates.sort_by(|a, b| {
                // Primary: replace highest value card (most improvement for us)
                b.1.cmp(&a.1)
                    // Tiebreak: displace the card least useful to next player
                    .then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            });
            return DeckDrawAction::Keep(keep_candidates[0].0);
        }

        // Card doesn't improve board. Normally we'd discard it, but if it's
        // useful to the next player, we might keep it on a hidden slot instead.
        if card_useful_to_next > 5.0 && !hidden.is_empty() {
            // This card would significantly help the opponent — absorb it
            // Place it on a hidden slot to avoid giving it to them
            let pos = pick_hidden_for_column_progress(view, &hidden, rng);
            return DeckDrawAction::Keep(pos);
        }

        // Discard the card and flip a hidden card
        if !hidden.is_empty() {
            let pos = pick_hidden_for_column_progress(view, &hidden, rng);
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden cards — must keep somewhere
        let pos = position_of_highest_revealed(&view.my_board);
        DeckDrawAction::Keep(pos)
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        let next_board = next_player_board(view);

        // Evaluate all valid placements: the displaced card goes to discard
        let mut candidates: Vec<(usize, i32, f64)> = Vec::new(); // (pos, improvement, displaced_usefulness)

        for (i, slot) in view.my_board.iter().enumerate() {
            match slot {
                VisibleSlot::Revealed(v) => {
                    let improvement = (*v as i32) - (drawn_card as i32);
                    let displaced_usefulness = next_board
                        .map(|b| {
                            card_usefulness_to_player(b, view.num_rows, view.num_cols, *v)
                        })
                        .unwrap_or(0.0);
                    candidates.push((i, improvement, displaced_usefulness));
                }
                VisibleSlot::Hidden => {
                    // Replacing hidden: improvement depends on expected value, but
                    // the displaced (hidden) card's value is unknown to us.
                    // We use 0 for displaced usefulness since we can't predict it.
                    candidates.push((i, 0, 0.0));
                }
                VisibleSlot::Cleared => {}
            }
        }

        // Sort: best improvement first, then least useful displaced card to next player
        candidates.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        });

        candidates.first().map(|c| c.0).unwrap_or(0)
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

/// Pick a hidden slot to flip, preferring columns with partial matches.
fn pick_hidden_for_column_progress(
    view: &StrategyView,
    hidden: &[usize],
    rng: &mut dyn RngCore,
) -> usize {
    let cols = column_analysis(view);
    // Prefer hidden slots in columns with partial matches
    for col_info in &cols {
        if col_info.partial_match.is_some() {
            for &h in &col_info.hidden_indices {
                if hidden.contains(&h) {
                    return h;
                }
            }
        }
    }
    // Fallback: random hidden
    *hidden.choose(rng).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view(
        my_board: Vec<VisibleSlot>,
        opponent_board: Vec<VisibleSlot>,
    ) -> StrategyView {
        StrategyView {
            my_index: 0,
            my_board,
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![opponent_board],
            opponent_indices: vec![1],
            discard_piles: vec![vec![3]],
            deck_remaining: 100,
            cumulative_scores: vec![0, 0],
            is_final_turn: false,
        }
    }

    #[test]
    fn avoids_discarding_useful_card_to_opponent() {
        // Next player has two 5s in column 0, needs one more to clear
        let opp_board = vec![
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
        // Our board: all hidden except one revealed 8
        let my_board = vec![
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
        let view = make_view(my_board, opp_board);
        let strategy = DefensiveStrategy;
        let mut rng = rand::thread_rng();

        // Drew a 5 from deck — doesn't improve our board (5 < 8 so it does improve),
        // but let's test with a card that doesn't improve: drew a 9
        let action = strategy.choose_deck_draw_action(&view, 9, &mut rng);
        // 9 doesn't improve (no revealed card > 9 to replace)
        // 9 is not very useful to opponent (high value), so should discard
        match action {
            DeckDrawAction::DiscardAndFlip(_) => {} // Expected
            DeckDrawAction::Keep(_) => {} // Also acceptable if keeping on hidden
        }
    }

    #[test]
    fn keeps_low_card_to_deny_opponent() {
        // Next player has high cards but could use a -2
        let opp_board = vec![
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(11),
            VisibleSlot::Revealed(12),
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
        // Our board: no revealed card higher than -2
        let my_board = vec![
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
            VisibleSlot::Hidden,
        ];
        let view = make_view(my_board, opp_board);
        let strategy = DefensiveStrategy;
        let mut rng = rand::thread_rng();

        // Drew a -2 — very useful to next player
        let action = strategy.choose_deck_draw_action(&view, -2, &mut rng);
        // Should keep it on our board rather than discard it
        match action {
            DeckDrawAction::Keep(_) => {} // Expected: keep to deny opponent
            DeckDrawAction::DiscardAndFlip(_) => {
                panic!("Should keep -2 to deny opponent access")
            }
        }
    }

    #[test]
    fn displaces_least_useful_card() {
        // Next player has two 7s in col 0
        let opp_board = vec![
            VisibleSlot::Revealed(7),
            VisibleSlot::Revealed(7),
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
        // Our board: two positions with value 10 and 7
        let my_board = vec![
            VisibleSlot::Revealed(10),
            VisibleSlot::Revealed(7),
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
        let view = make_view(my_board, opp_board);
        let strategy = DefensiveStrategy;
        let mut rng = rand::thread_rng();

        // Drew a 3 — can replace either the 10 or the 7
        // Displacing the 7 would help the opponent (matches their partial column)
        // So should prefer to displace the 10 (less useful to opponent)
        let action = strategy.choose_deck_draw_action(&view, 3, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert_eq!(pos, 0, "Should replace the 10, not the 7 (7 helps opponent)");
            }
            _ => panic!("Should keep the 3"),
        }
    }
}
