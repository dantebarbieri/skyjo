use rand::RngCore;
use rand::prelude::SliceRandom;
use rand::seq::IndexedRandom;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, ConceptReference, DecisionLogic, DecisionNode, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, PriorityRule, Strategy, StrategyDescription, StrategyView,
};

use super::common::{card_usefulness_to_player, column_analysis, next_player_board};

/// A saboteur strategy that wins by controlling the discard pile.
///
/// Key insight: whatever card you discard (either the drawn card or the displaced
/// board card) becomes the new discard top for the next player. The Saboteur
/// ensures the next player always finds unhelpful cards waiting for them.
///
/// Prefers drawing from the deck for maximum control — you get to choose whether
/// to keep or discard, controlling what ends up on the pile. Only takes from
/// discard when it's a genuine improvement (Greedy threshold).
pub struct SaboteurStrategy;

impl Strategy for SaboteurStrategy {
    fn name(&self) -> &str {
        "Saboteur"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Saboteur".into(),
            summary: "Wins by controlling the discard pile. Ensures the next player always finds unhelpful cards waiting for them. Draws from deck for maximum control over what ends up on the pile.".into(),
            complexity: Complexity::Medium,
            strengths: vec![
                "Actively poisons the discard pile for the next player".into(),
                "Buries cards that would help opponents by keeping them".into(),
                "Draws from deck for maximum pile control".into(),
            ],
            weaknesses: vec![
                "May keep suboptimal cards to deny opponents".into(),
                "Focused on one opponent (the next player)".into(),
                "No card counting or probability analysis".into(),
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
                                detail: Some("Genuine improvement — Greedy threshold.".into()),
                            },
                            PriorityRule {
                                condition: "Discard top < highest revealed card on board".into(),
                                action: "Take from discard pile".into(),
                                detail: Some("Guaranteed improvement by replacing the worst card.".into()),
                            },
                            PriorityRule {
                                condition: "Otherwise".into(),
                                action: "Draw from deck".into(),
                                detail: Some("Deck gives more control — we choose keep vs discard, controlling what the next player sees.".into()),
                            },
                        ],
                    },
                },
                PhaseDescription {
                    phase: Phase::DeckDrawAction,
                    label: "After Drawing from Deck".into(),
                    logic: DecisionLogic::DecisionTree {
                        root: DecisionNode::Condition {
                            test: "Is the drawn card useful to the next player? (usefulness > 0)".into(),
                            if_true: Box::new(DecisionNode::Condition {
                                test: "Would any keep candidate displace a card even MORE useful to them?".into(),
                                if_true: Box::new(DecisionNode::Action {
                                    action: "Keep at the position that displaces the most-useful-to-opponent card".into(),
                                    detail: Some("The displaced card goes to the pile, but it's worse for us to let them have the drawn card. Pick the position whose displaced card is most useful to opponent, since we're burying the drawn card anyway.".into()),
                                }),
                                if_false: Box::new(DecisionNode::Action {
                                    action: "Keep the drawn card to bury it (place on a hidden slot if possible)".into(),
                                    detail: Some("Deny the opponent access to this card by absorbing it into our board.".into()),
                                }),
                            }),
                            if_false: Box::new(DecisionNode::Condition {
                                test: "Is the drawn card high-value (≥ 8)?".into(),
                                if_true: Box::new(DecisionNode::Action {
                                    action: "Discard it as poison — high card on the discard pile hurts the opponent".into(),
                                    detail: Some("Flip a hidden card, preferring columns with partial matches.".into()),
                                }),
                                if_false: Box::new(DecisionNode::Condition {
                                    test: "Does the drawn card improve the board? (lower than highest revealed)".into(),
                                    if_true: Box::new(DecisionNode::Action {
                                        action: "Keep it — replace highest revealed, preferring to displace cards least useful to opponent".into(),
                                        detail: None,
                                    }),
                                    if_false: Box::new(DecisionNode::Action {
                                        action: "Discard and flip a hidden card, preferring columns with partial matches".into(),
                                        detail: None,
                                    }),
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
                                action: "Replace the best target, tiebreaking by displacing cards least useful to the next player".into(),
                                detail: Some("Primary: own score improvement. Secondary: minimize what the opponent gains from the displaced card.".into()),
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
                    used_for: "Evaluating every card that might end up on the discard pile to ensure the next player finds only unhelpful options.".into(),
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
        // Greedy threshold: only take discard if it's a genuine improvement
        if let Some(discard_val) = view.discard_top(0) {
            let highest_revealed = highest_revealed_value(&view.my_board);
            if discard_val <= 0 || highest_revealed.is_some_and(|h| discard_val < h) {
                return DrawChoice::DrawFromDiscard(0);
            }
        }
        // Prefer deck — more control over what ends up on the discard pile
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
        let drawn_usefulness = next_board
            .map(|b| card_usefulness_to_player(b, view.num_rows, view.num_cols, drawn_card))
            .unwrap_or(0.0);

        // Build keep candidates: positions where keeping improves our board
        let mut keep_candidates: Vec<KeepCandidate> = Vec::new();
        for (i, slot) in view.my_board.iter().enumerate() {
            if let VisibleSlot::Revealed(v) = slot
                && drawn_card < *v
            {
                let displaced_usefulness = next_board
                    .map(|b| card_usefulness_to_player(b, view.num_rows, view.num_cols, *v))
                    .unwrap_or(0.0);
                keep_candidates.push(KeepCandidate {
                    pos: i,
                    displaced_value: *v,
                    displaced_usefulness,
                    improvement: (*v as i32) - (drawn_card as i32),
                });
            }
        }

        if drawn_usefulness > 0.0 {
            // Drawn card IS useful to next player — prefer keeping it to bury it
            // BUT: if a keep candidate would displace something even MORE useful, do that
            let max_displaced_usefulness = keep_candidates
                .iter()
                .map(|c| c.displaced_usefulness)
                .fold(f64::NEG_INFINITY, f64::max);

            if !keep_candidates.is_empty() && max_displaced_usefulness > drawn_usefulness {
                // A displaced card is even more useful to opponent than the drawn card.
                // Keep at position that displaces the most-useful-to-opponent card
                // (we're already burying the drawn card; might as well maximize damage)
                keep_candidates.sort_by(|a, b| {
                    b.displaced_usefulness
                        .partial_cmp(&a.displaced_usefulness)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                return DeckDrawAction::Keep(keep_candidates[0].pos);
            }

            // No keep candidate displaces something more useful — bury drawn card on hidden slot
            if !hidden.is_empty() {
                let pos = pick_hidden_for_column_progress(view, &hidden, rng);
                return DeckDrawAction::Keep(pos);
            }

            // No hidden slots; keep on highest revealed if it at least improves
            if !keep_candidates.is_empty() {
                // Tiebreak: least useful displaced card to opponent
                keep_candidates.sort_by(|a, b| {
                    b.improvement.cmp(&a.improvement).then(
                        a.displaced_usefulness
                            .partial_cmp(&b.displaced_usefulness)
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
                });
                return DeckDrawAction::Keep(keep_candidates[0].pos);
            }

            // Can't keep anywhere useful — forced to discard
            let pos = position_of_highest_revealed(&view.my_board);
            return DeckDrawAction::Keep(pos);
        }

        // Drawn card is NOT useful to next player (usefulness <= 0)
        if drawn_card >= 8 {
            // High-value poison — discard it onto the pile for the next player
            if !hidden.is_empty() {
                let pos = pick_hidden_for_column_progress(view, &hidden, rng);
                return DeckDrawAction::DiscardAndFlip(pos);
            }
            // No hidden cards; must keep — place over highest revealed
            let pos = position_of_highest_revealed(&view.my_board);
            return DeckDrawAction::Keep(pos);
        }

        // Drawn card is not useful to opponent and not high-value poison
        if !keep_candidates.is_empty() {
            // It improves our board — keep it, displacing card least useful to opponent
            keep_candidates.sort_by(|a, b| {
                b.improvement.cmp(&a.improvement).then(
                    a.displaced_usefulness
                        .partial_cmp(&b.displaced_usefulness)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
            });
            return DeckDrawAction::Keep(keep_candidates[0].pos);
        }

        // Doesn't improve board — discard and flip
        if !hidden.is_empty() {
            let pos = pick_hidden_for_column_progress(view, &hidden, rng);
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden cards left — must keep somewhere
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

        // Evaluate all valid placements
        let mut candidates: Vec<(usize, i32, f64)> = Vec::new(); // (pos, improvement, displaced_usefulness)

        for (i, slot) in view.my_board.iter().enumerate() {
            match slot {
                VisibleSlot::Revealed(v) => {
                    let improvement = (*v as i32) - (drawn_card as i32);
                    let displaced_usefulness = next_board
                        .map(|b| card_usefulness_to_player(b, view.num_rows, view.num_cols, *v))
                        .unwrap_or(0.0);
                    candidates.push((i, improvement, displaced_usefulness));
                }
                VisibleSlot::Hidden => {
                    // Unknown displaced card — use 0 for displaced usefulness
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

struct KeepCandidate {
    pos: usize,
    #[allow(dead_code)]
    displaced_value: CardValue,
    displaced_usefulness: f64,
    improvement: i32,
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

    fn make_view(my_board: Vec<VisibleSlot>, opponent_board: Vec<VisibleSlot>) -> StrategyView {
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
    fn prefers_deck_draw_for_control() {
        // Discard top is 5, our highest revealed is 4 — no improvement from discard
        let my_board = vec![
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
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let opp_board = vec![VisibleSlot::Hidden; 12];
        let mut view = make_view(my_board, opp_board);
        view.discard_piles = vec![vec![5]];

        let strategy = SaboteurStrategy;
        let mut rng = rand::rng();
        let choice = strategy.choose_draw(&view, &mut rng);
        assert_eq!(choice, DrawChoice::DrawFromDeck);
    }

    #[test]
    fn takes_discard_when_genuine_improvement() {
        // Discard top is 2, our highest revealed is 10
        let my_board = vec![
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
        let opp_board = vec![VisibleSlot::Hidden; 12];
        let view = make_view(my_board, opp_board);

        let strategy = SaboteurStrategy;
        let mut rng = rand::rng();
        // discard pile has [3], top is 3 which is < 10
        let choice = strategy.choose_draw(&view, &mut rng);
        assert_eq!(choice, DrawChoice::DrawFromDiscard(0));
    }

    #[test]
    fn buries_card_useful_to_opponent() {
        // Next player has two 5s in col 0 — a 5 is very useful to them
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
        // Our board: all hidden — drawn 5 doesn't improve (nothing revealed to beat)
        let my_board = vec![VisibleSlot::Hidden; 12];
        let view = make_view(my_board, opp_board);

        let strategy = SaboteurStrategy;
        let mut rng = rand::rng();

        // Drew a 5 — very useful to opponent (would complete their column)
        let action = strategy.choose_deck_draw_action(&view, 5, &mut rng);
        match action {
            DeckDrawAction::Keep(_) => {} // Expected: bury it
            DeckDrawAction::DiscardAndFlip(_) => {
                panic!("Should keep 5 to deny opponent column clear")
            }
        }
    }

    #[test]
    fn discards_high_poison_when_not_useful_to_opponent() {
        // Opponent board: all hidden, no partial matches
        let opp_board = vec![VisibleSlot::Hidden; 12];
        // Our board: all hidden
        let my_board = vec![VisibleSlot::Hidden; 12];
        let view = make_view(my_board, opp_board);

        let strategy = SaboteurStrategy;
        let mut rng = rand::rng();

        // Drew a 10 — high value, not useful to opponent (they have no 10 partial matches)
        let action = strategy.choose_deck_draw_action(&view, 10, &mut rng);
        match action {
            DeckDrawAction::DiscardAndFlip(_) => {} // Expected: discard as poison
            DeckDrawAction::Keep(_) => {
                panic!("Should discard high card as poison on the pile")
            }
        }
    }

    #[test]
    fn displaces_least_useful_card_to_opponent() {
        // Opponent has two 7s in col 0
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
        // Our board: revealed 10 at pos 0 and revealed 7 at pos 1
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

        let strategy = SaboteurStrategy;
        let mut rng = rand::rng();

        // Drew a 3 — not useful to opponent, improves board
        // Can replace 10 (displaces 10, not useful to opponent) or 7 (displaces 7, very useful)
        // Should prefer replacing 10 to avoid giving opponent a 7
        let action = strategy.choose_deck_draw_action(&view, 3, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert_eq!(
                    pos, 0,
                    "Should replace the 10, not the 7 (7 helps opponent)"
                );
            }
            _ => panic!("Should keep the 3"),
        }
    }

    #[test]
    fn discard_placement_minimizes_opponent_benefit() {
        // Opponent has two 8s in col 0
        let opp_board = vec![
            VisibleSlot::Revealed(8),
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
        ];
        // Our board: revealed 8 at pos 0 and revealed 9 at pos 3
        let my_board = vec![
            VisibleSlot::Revealed(8),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Revealed(9),
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

        let strategy = SaboteurStrategy;
        let mut rng = rand::rng();

        // Drew a 2 from discard — both 8 and 9 are improvements
        // Displacing 8 would help opponent (matches their partial column)
        // Displacing 9 does not — should prefer replacing 9
        let pos = strategy.choose_discard_draw_placement(&view, 2, &mut rng);
        assert_eq!(pos, 3, "Should replace the 9, not the 8 (8 helps opponent)");
    }
}
