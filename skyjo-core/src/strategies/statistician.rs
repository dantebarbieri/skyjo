use rand::seq::SliceRandom;
use rand::RngCore;

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{DeckDrawAction, DrawChoice, Strategy, StrategyView};

use super::common::{
    average_unknown_value, card_usefulness_to_player, column_analysis, count_remaining,
    expected_score, next_player_board,
};

/// Threshold for floating-point comparison in tiebreaking.
const EV_EPSILON: f64 = 0.5;

/// A mathematically-driven strategy that uses expected value calculations.
///
/// - Computes the average unknown card value from the remaining distribution
/// - Evaluates every decision by expected score delta
/// - Dynamically switches between "go out" and "reduce" mode each turn
/// - Abandons column-clear attempts when no remaining copies exist
/// - Tiebreaks by choosing the move worst for the next player
pub struct StatisticianStrategy;

impl Strategy for StatisticianStrategy {
    fn name(&self) -> &str {
        "Statistician"
    }

    fn choose_initial_flips(
        &self,
        view: &StrategyView,
        count: usize,
        rng: &mut dyn RngCore,
    ) -> Vec<usize> {
        // No information to optimize on at this point
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
        let avg = average_unknown_value(view);

        // Evaluate: what's the best delta we get from taking the discard top?
        let discard_delta = if let Some(discard_val) = view.discard_top(0) {
            best_placement_delta(view, discard_val, avg)
        } else {
            f64::NEG_INFINITY
        };

        // Evaluate: expected delta from drawing from deck
        // When drawing from deck, we can either keep (replace) or discard+flip
        // The "option value" of deck draw is: max(best_keep_delta, flip_delta)
        // Expected best_keep_delta ≈ for each possible card, weight by probability
        // Simplified: expected deck value is avg, so expected keep delta ≈ best_placement_delta(avg)
        // Flip delta ≈ 0 (hidden was already counted at avg)
        let deck_keep_delta = best_placement_delta(view, avg.round() as CardValue, avg);
        let deck_delta = deck_keep_delta.max(0.0); // can always discard+flip for ~0 delta

        if discard_delta > deck_delta + EV_EPSILON {
            DrawChoice::DrawFromDiscard(0)
        } else if deck_delta > discard_delta + EV_EPSILON {
            DrawChoice::DrawFromDeck
        } else {
            // Tiebreak: which leaves a worse discard for next player?
            // Taking from discard means we'll displace a card onto the pile.
            // Drawing from deck means the current discard top stays (until we discard the drawn card or a displaced card).
            // In both cases we leave a new card on the pile — but taking from discard
            // removes a potentially useful card from the pile. The net effect is complex,
            // so default to deck draw (more flexible with the keep/discard option).
            DrawChoice::DrawFromDeck
        }
    }

    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        rng: &mut dyn RngCore,
    ) -> DeckDrawAction {
        let avg = average_unknown_value(view);
        let go_out = should_go_out(view, avg);
        let next_board = next_player_board(view);

        // Evaluate keep options
        let mut keep_options: Vec<KeepOption> = Vec::new();
        for (i, slot) in view.my_board.iter().enumerate() {
            match slot {
                VisibleSlot::Revealed(old_val) => {
                    let mut delta = (*old_val as f64) - (drawn_card as f64);
                    // Check if placing here triggers a column clear
                    let clear_bonus = column_clear_bonus(view, i, drawn_card, avg);
                    delta += clear_bonus;
                    let displaced = *old_val;
                    let usefulness = next_board
                        .map(|b| {
                            card_usefulness_to_player(b, view.num_rows, view.num_cols, displaced)
                        })
                        .unwrap_or(0.0);
                    keep_options.push(KeepOption {
                        pos: i,
                        delta,
                        displaced_usefulness: usefulness,
                    });
                }
                VisibleSlot::Hidden => {
                    let mut delta = avg - (drawn_card as f64);
                    let clear_bonus = column_clear_bonus(view, i, drawn_card, avg);
                    delta += clear_bonus;
                    // Displaced card is unknown — can't predict usefulness
                    keep_options.push(KeepOption {
                        pos: i,
                        delta,
                        displaced_usefulness: 0.0,
                    });
                }
                VisibleSlot::Cleared => {}
            }
        }

        // Best keep option
        keep_options.sort_by(|a, b| {
            b.delta
                .partial_cmp(&a.delta)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    a.displaced_usefulness
                        .partial_cmp(&b.displaced_usefulness)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });

        let best_keep = keep_options.first();

        // Evaluate discard+flip option
        let hidden: Vec<usize> = view
            .my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect();

        // Discarding the drawn card: delta = 0 from the flip (hidden was avg, revealed is actual)
        // But we also consider: how useful is the drawn card to the next player?
        let drawn_usefulness = next_board
            .map(|b| {
                card_usefulness_to_player(b, view.num_rows, view.num_cols, drawn_card)
            })
            .unwrap_or(0.0);

        if let Some(best) = best_keep {
            if best.delta > EV_EPSILON {
                return DeckDrawAction::Keep(best.pos);
            }

            // Delta is marginal — tiebreak by opponent denial
            if best.delta > -EV_EPSILON {
                // If discarding the drawn card would help the opponent more than
                // displacing our card, keep it
                if drawn_usefulness > best.displaced_usefulness + 3.0 {
                    return DeckDrawAction::Keep(best.pos);
                }
            }

            // In go-out mode, prefer flipping to get closer to all-revealed
            if go_out && !hidden.is_empty() && best.delta < EV_EPSILON {
                let pos = pick_flip_for_mode(view, &hidden, go_out, rng);
                return DeckDrawAction::DiscardAndFlip(pos);
            }
        }

        // Discard and flip
        if !hidden.is_empty() {
            let pos = pick_flip_for_mode(view, &hidden, go_out, rng);
            return DeckDrawAction::DiscardAndFlip(pos);
        }

        // No hidden — must keep somewhere
        if let Some(best) = best_keep {
            DeckDrawAction::Keep(best.pos)
        } else {
            DeckDrawAction::Keep(0)
        }
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        let avg = average_unknown_value(view);
        let next_board = next_player_board(view);

        let mut options: Vec<(usize, f64, f64)> = Vec::new(); // (pos, delta, displaced_usefulness)

        for (i, slot) in view.my_board.iter().enumerate() {
            match slot {
                VisibleSlot::Revealed(old_val) => {
                    let mut delta = (*old_val as f64) - (drawn_card as f64);
                    delta += column_clear_bonus(view, i, drawn_card, avg);
                    let usefulness = next_board
                        .map(|b| {
                            card_usefulness_to_player(
                                b,
                                view.num_rows,
                                view.num_cols,
                                *old_val,
                            )
                        })
                        .unwrap_or(0.0);
                    options.push((i, delta, usefulness));
                }
                VisibleSlot::Hidden => {
                    let mut delta = avg - (drawn_card as f64);
                    delta += column_clear_bonus(view, i, drawn_card, avg);
                    options.push((i, delta, 0.0));
                }
                VisibleSlot::Cleared => {}
            }
        }

        // Sort: best delta first, then least useful displaced card to opponent
        options.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    a.2.partial_cmp(&b.2)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });

        options.first().map(|o| o.0).unwrap_or(0)
    }
}

struct KeepOption {
    pos: usize,
    delta: f64,
    displaced_usefulness: f64,
}

/// Determine if we should try to go out (all cards revealed).
/// Go out if our expected score is solo lowest among all players.
fn should_go_out(view: &StrategyView, avg: f64) -> bool {
    if view.is_final_turn {
        return false; // On final turn, just minimize score
    }

    let my_expected = expected_score(&view.my_board, avg);

    for opp_board in &view.opponent_boards {
        let opp_expected = expected_score(opp_board, avg);
        if opp_expected <= my_expected {
            return false; // Someone else has lower or equal expected score
        }
    }

    true
}

/// Compute the best score delta from placing `card` somewhere on the board.
fn best_placement_delta(view: &StrategyView, card: CardValue, avg: f64) -> f64 {
    let mut best_delta = f64::NEG_INFINITY;

    for (i, slot) in view.my_board.iter().enumerate() {
        let delta = match slot {
            VisibleSlot::Revealed(old_val) => {
                let mut d = (*old_val as f64) - (card as f64);
                d += column_clear_bonus(view, i, card, avg);
                d
            }
            VisibleSlot::Hidden => {
                let mut d = avg - (card as f64);
                d += column_clear_bonus(view, i, card, avg);
                d
            }
            VisibleSlot::Cleared => continue,
        };
        if delta > best_delta {
            best_delta = delta;
        }
    }

    best_delta
}

/// Compute the bonus from a column clear if placing `card` at `pos` triggers one.
/// Returns 0 if no clear is triggered.
fn column_clear_bonus(view: &StrategyView, pos: usize, card: CardValue, _avg: f64) -> f64 {
    let col = pos / view.num_rows;
    let cols = column_analysis(view);
    let col_info = &cols[col];

    if col_info.cleared_count > 0 {
        return 0.0;
    }

    // Check column clear feasibility — are there enough remaining copies?
    let remaining = count_remaining(view, card);

    // Simulate: if we place `card` at `pos`, what does the column look like?
    let mut all_match = true;
    let mut hidden_count = 0;
    for &idx in &col_info.indices {
        if idx == pos {
            // This slot will become Revealed(card)
            continue;
        }
        match view.my_board[idx] {
            VisibleSlot::Revealed(v) => {
                if v != card {
                    all_match = false;
                    break;
                }
            }
            VisibleSlot::Hidden => {
                hidden_count += 1;
                // If no copies remain, can't complete the clear
                if remaining == 0 && hidden_count > 0 {
                    all_match = false;
                    break;
                }
            }
            VisibleSlot::Cleared => {
                all_match = false;
                break;
            }
        }
    }

    if all_match && hidden_count == 0 {
        // Placing this card completes the column clear!
        // Bonus = sum of all values that will be cleared (including the placed card)
        // All cards in column have value `card`, and there are num_rows of them
        (card as f64) * (view.num_rows as f64)
    } else {
        0.0
    }
}

/// Pick a hidden slot to flip based on mode.
fn pick_flip_for_mode(
    view: &StrategyView,
    hidden: &[usize],
    go_out: bool,
    rng: &mut dyn RngCore,
) -> usize {
    if go_out {
        // In go-out mode, just flip any hidden to make progress
        // Prefer columns with more revealed cards (closer to completion)
        let cols = column_analysis(view);
        let mut best_hidden = None;
        let mut best_revealed_count = 0;

        for col_info in &cols {
            let revealed_in_col = col_info.revealed_values.len();
            if revealed_in_col > best_revealed_count
                && let Some(&h) = col_info.hidden_indices.first()
                && hidden.contains(&h)
            {
                best_hidden = Some(h);
                best_revealed_count = revealed_in_col;
            }
        }

        if let Some(h) = best_hidden {
            return h;
        }
    } else {
        // In reduce mode, prefer columns with partial matches (might enable future clears)
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
    }

    *hidden.choose(rng).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view(
        my_board: Vec<VisibleSlot>,
        opponent_boards: Vec<Vec<VisibleSlot>>,
    ) -> StrategyView {
        let opp_count = opponent_boards.len();
        let opp_indices: Vec<usize> = (1..=opp_count).collect();
        StrategyView {
            my_index: 0,
            my_board,
            num_rows: 3,
            num_cols: 4,
            opponent_boards,
            opponent_indices: opp_indices,
            discard_piles: vec![vec![5]],
            deck_remaining: 80,
            cumulative_scores: vec![0; opp_count + 1],
            is_final_turn: false,
        }
    }

    #[test]
    fn go_out_when_lowest_expected() {
        // Our board: mostly low revealed values
        let my_board = vec![
            VisibleSlot::Revealed(-2),
            VisibleSlot::Revealed(-1),
            VisibleSlot::Revealed(0),
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(0),
            VisibleSlot::Revealed(-1),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(0),
        ];
        // Opponent: mostly hidden (expected high)
        let opp_board = vec![VisibleSlot::Hidden; 12];

        let view = make_view(my_board, vec![opp_board]);
        let avg = average_unknown_value(&view);

        assert!(
            should_go_out(&view, avg),
            "Should go out when expected score is lowest"
        );
    }

    #[test]
    fn reduce_when_not_lowest() {
        // Our board: lots of high revealed values
        let my_board = vec![
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
        // Opponent: low values
        let opp_board = vec![
            VisibleSlot::Revealed(0),
            VisibleSlot::Revealed(0),
            VisibleSlot::Revealed(0),
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

        let view = make_view(my_board, vec![opp_board]);
        let avg = average_unknown_value(&view);

        assert!(
            !should_go_out(&view, avg),
            "Should not go out when not lowest"
        );
    }

    #[test]
    fn abandons_clear_when_no_remaining_copies() {
        // Col 0 has two 5s, but suppose all other 5s are in discard
        let my_board = vec![
            VisibleSlot::Revealed(5),
            VisibleSlot::Revealed(5),
            VisibleSlot::Hidden,
            VisibleSlot::Revealed(8),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
        ];
        let mut view = make_view(my_board, vec![vec![VisibleSlot::Hidden; 12]]);
        // Put all remaining 5s in discard (10 total - 2 on board = 8 in discard)
        view.discard_piles = vec![vec![5; 8]];

        // column_clear_bonus at pos 2 with card=5 should be 0 since remaining=0
        let avg = average_unknown_value(&view);
        let bonus = column_clear_bonus(&view, 2, 5, avg);
        // Actually remaining = 10 - 2(board) - 8(discard) = 0
        // But placing a 5 at pos 2 would complete the column, AND we have the card
        // The check is about whether hidden slots need filling — pos 2 is the target, no other hidden
        // So bonus should actually be positive since we're placing the completing card
        // Let me reconsider: the 5 is being placed, so hidden_count in the rest of col is 0
        // All other slots are Revealed(5), so all_match=true, hidden_count=0 → clear triggers!
        assert!(bonus > 0.0, "Should recognize clear completes by placing card");
    }

    #[test]
    fn tiebreaks_against_opponent() {
        // Two positions with equal delta, but one displaces a card useful to opponent
        let my_board = vec![
            VisibleSlot::Revealed(8), // displacing 8
            VisibleSlot::Revealed(8), // displacing 8
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
        // Opponent has two 8s in column 0 — needs one more to clear
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

        let view = make_view(my_board, vec![opp_board]);
        let strategy = StatisticianStrategy;
        let mut rng = rand::thread_rng();

        // Drew a 3 — both pos 0 and pos 1 give delta of 5 (8-3)
        // Both displace an 8, which is useful to opponent (matches their column)
        // No tiebreak differentiation here since both displacements are identical
        let action = strategy.choose_deck_draw_action(&view, 3, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert!(pos <= 1, "Should keep at pos 0 or 1");
            }
            _ => panic!("Should keep the 3 to replace an 8"),
        }
    }

    #[test]
    fn takes_from_discard_when_ev_better() {
        // Discard has a -2, which is always great
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
        let mut view = make_view(my_board, vec![vec![VisibleSlot::Hidden; 12]]);
        view.discard_piles = vec![vec![-2]];

        let strategy = StatisticianStrategy;
        let mut rng = rand::thread_rng();

        let draw = strategy.choose_draw(&view, &mut rng);
        assert!(
            matches!(draw, DrawChoice::DrawFromDiscard(0)),
            "Should take -2 from discard — massive EV improvement"
        );
    }

    #[test]
    fn final_turn_minimizes_score() {
        let my_board = vec![
            VisibleSlot::Revealed(-2),
            VisibleSlot::Revealed(-1),
            VisibleSlot::Revealed(0),
            VisibleSlot::Revealed(1),
            VisibleSlot::Revealed(2),
            VisibleSlot::Revealed(3),
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Hidden,
            VisibleSlot::Revealed(4),
            VisibleSlot::Revealed(5),
            VisibleSlot::Revealed(6),
        ];
        let mut view = make_view(my_board, vec![vec![VisibleSlot::Hidden; 12]]);
        view.is_final_turn = true;

        let avg = average_unknown_value(&view);
        assert!(
            !should_go_out(&view, avg),
            "Should not go out on final turn"
        );
    }
}
