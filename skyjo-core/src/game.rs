use rand::prelude::SliceRandom;
// Use StdRng (ChaCha12) instead of StdRng to ensure identical RNG sequences
// across all platforms. StdRng uses different algorithms on 32-bit (WASM)
// vs 64-bit (native), causing different game outcomes for the same seed.
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Default safety limit: maximum turns per round to prevent infinite loops.
/// In standard Skyjo (150 cards, 4 cols × 3 rows), a round typically takes
/// 30-100 turns. 10,000 is generous enough to never trigger in normal play.
pub const DEFAULT_MAX_TURNS_PER_ROUND: usize = 10_000;

use crate::board::PlayerBoard;
use crate::card::CardValue;
use crate::error::{Result, SkyjoError};
use crate::history::*;
use crate::rules::Rules;
use crate::strategy::{DeckDrawAction, DrawChoice, Strategy, StrategyView};

pub struct Game {
    rules: Box<dyn Rules>,
    strategies: Vec<Box<dyn Strategy>>,
    rng: StdRng,
    num_players: usize,
    _seed: u64,

    // Per-round mutable state
    boards: Vec<PlayerBoard>,
    deck: Vec<CardValue>,
    discard_piles: Vec<Vec<CardValue>>,
    cumulative_scores: Vec<i32>,

    // History accumulator
    history: GameHistory,

    // Track who went out last round (for determining round starter)
    last_round_goer: Option<usize>,

    max_turns_per_round: usize,
}

impl Game {
    pub fn new(
        rules: Box<dyn Rules>,
        strategies: Vec<Box<dyn Strategy>>,
        seed: u64,
    ) -> Result<Self> {
        let num_players = strategies.len();
        if num_players < 2 {
            return Err(SkyjoError::NotEnoughPlayers);
        }
        if num_players > 8 {
            return Err(SkyjoError::TooManyPlayers);
        }

        let strategy_names: Vec<String> = strategies.iter().map(|s| s.name().to_string()).collect();
        let rules_name = rules.name().to_string();

        Ok(Game {
            rng: StdRng::seed_from_u64(seed),
            num_players,
            _seed: seed,
            boards: Vec::new(),
            deck: Vec::new(),
            discard_piles: Vec::new(),
            cumulative_scores: vec![0; num_players],
            history: GameHistory {
                seed,
                num_players,
                strategy_names,
                rules_name,
                rounds: Vec::new(),
                final_scores: Vec::new(),
                winners: Vec::new(),
            },
            last_round_goer: None,
            max_turns_per_round: DEFAULT_MAX_TURNS_PER_ROUND,
            rules,
            strategies,
        })
    }

    /// Set the maximum number of turns allowed per round.
    /// Rounds exceeding this limit are forcefully ended and marked as truncated.
    pub fn set_max_turns_per_round(&mut self, limit: usize) {
        self.max_turns_per_round = limit;
    }

    /// Run the entire game to completion. Returns the finished GameHistory.
    pub fn play(mut self) -> Result<GameHistory> {
        loop {
            self.play_round()?;
            if self
                .cumulative_scores
                .iter()
                .any(|&s| s >= self.rules.end_threshold())
            {
                break;
            }
        }
        self.history.final_scores = self.cumulative_scores.clone();
        self.history.winners = self.rules.resolve_winners(&self.cumulative_scores);
        Ok(self.history)
    }

    fn play_round(&mut self) -> Result<()> {
        let round_number = self.history.rounds.len();

        // 1. Build & shuffle deck
        let mut deck = self.rules.build_deck();
        deck.shuffle(&mut self.rng);
        let initial_deck_order = deck.clone();

        let cards_per_player = self.rules.num_cards_per_player();
        let num_rows = self.rules.num_rows();
        let num_cols = self.rules.num_cols();

        // 2. Deal cards to each player
        let mut dealt_hands: Vec<Vec<CardValue>> = Vec::with_capacity(self.num_players);
        for _ in 0..self.num_players {
            let mut hand = Vec::with_capacity(cards_per_player);
            for _ in 0..cards_per_player {
                hand.push(deck.pop().expect("deck too small for dealing"));
            }
            dealt_hands.push(hand);
        }

        // 3. Initialize boards (all hidden)
        self.boards = dealt_hands
            .iter()
            .map(|h| PlayerBoard::new(h, num_rows, num_cols))
            .collect();

        // 4. First discard: pop one card from deck onto discard pile 0
        let first_discard = deck.pop().expect("deck empty after dealing");
        let pile_count = self.rules.discard_pile_count(self.num_players);
        self.discard_piles = vec![Vec::new(); pile_count];
        self.discard_piles[0].push(first_discard);
        self.deck = deck;

        // 5. Setup flips
        let flip_count = self.rules.initial_flips();
        let mut setup_flips: Vec<Vec<usize>> = Vec::with_capacity(self.num_players);
        for i in 0..self.num_players {
            let view = self.build_view(i, false);
            let flips = self.strategies[i].choose_initial_flips(&view, flip_count, &mut self.rng);
            for &pos in &flips {
                self.boards[i].flip(pos)?;
            }
            setup_flips.push(flips);
        }

        // 6. Determine starting player
        let starter = if round_number == 0 {
            let revealed_sums: Vec<i32> = self
                .boards
                .iter()
                .map(|b| {
                    b.slots
                        .iter()
                        .filter_map(|s| s.visible_value().map(|v| v as i32))
                        .sum()
                })
                .collect();
            self.rules.first_round_starting_player(&revealed_sums)
        } else {
            // After round 1: player who went out last round goes first
            self.last_round_goer.unwrap_or(0)
        };

        // 7. Turn loop
        let mut going_out_player: Option<usize> = None;
        let mut turns_after_goer: usize = 0;
        let mut current = starter;
        let mut turns: Vec<TurnRecord> = Vec::new();
        let mut truncated = false;

        loop {
            // If someone has gone out and we've given everyone else a turn, stop
            if going_out_player.is_some() && turns_after_goer >= self.num_players - 1 {
                break;
            }

            // Safety: prevent infinite rounds from buggy strategies
            if turns.len() >= self.max_turns_per_round {
                // Force end: treat the current player as going out
                going_out_player.get_or_insert(current);
                truncated = true;
                break;
            }

            // Skip the going-out player on subsequent passes
            if going_out_player == Some(current) {
                current = (current + 1) % self.num_players;
                continue;
            }

            let is_final = going_out_player.is_some();
            let turn_record = self.play_turn(current, is_final)?;
            let went_out = turn_record.went_out;
            turns.push(turn_record);

            if went_out && going_out_player.is_none() {
                going_out_player = Some(current);
            }

            if going_out_player.is_some() && going_out_player != Some(current) {
                // This shouldn't happen: a different player "went out" during final turns.
                // But if it does, we still count it as a final turn consumed.
                turns_after_goer += 1;
            } else if going_out_player == Some(current) && went_out {
                // The player just went out this turn, don't count it as a "turn after"
            } else if going_out_player.is_some() {
                turns_after_goer += 1;
            }

            current = (current + 1) % self.num_players;
        }

        // 8. Reveal all hidden cards and check column clearing
        let mut end_of_round_clears: Vec<ColumnClearEvent> = Vec::new();
        for (player_idx, board) in self.boards.iter_mut().enumerate() {
            // Reveal all hidden
            for slot in board.slots.iter_mut() {
                if let crate::card::Slot::Hidden(v) = *slot {
                    *slot = crate::card::Slot::Revealed(v);
                }
            }
            // Check column clearing after reveal
            for col in 0..num_cols {
                if let Some(val) = board.check_column_match(col) {
                    let cleared_cards = board.clear_column(col);
                    let target = self.rules.discard_target(player_idx);
                    for card in cleared_cards {
                        self.discard_piles[target].push(card);
                    }
                    end_of_round_clears.push(ColumnClearEvent {
                        player_index: player_idx,
                        column: col,
                        card_value: val,
                        displaced_card: None,
                    });
                }
            }
        }

        // 9. Compute raw scores
        let mut round_scores: Vec<i32> = self.boards.iter().map(|b| b.score()).collect();

        // 10. Apply going-out penalty
        if let Some(goer) = going_out_player {
            let goer_score = round_scores[goer];
            let min_other = round_scores
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != goer)
                .map(|(_, &s)| s)
                .min()
                .unwrap_or(goer_score);
            let is_solo_lowest = round_scores
                .iter()
                .enumerate()
                .all(|(i, &s)| i == goer || s > goer_score);
            round_scores[goer] =
                self.rules
                    .apply_going_out_penalty(goer_score, min_other, is_solo_lowest);
        }

        // 11. Update cumulative scores
        for (cum, round) in self.cumulative_scores.iter_mut().zip(&round_scores) {
            *cum += round;
        }

        self.last_round_goer = going_out_player;

        // 12. Record round history
        self.history.rounds.push(RoundHistory {
            round_number,
            initial_deck_order,
            dealt_hands,
            setup_flips,
            starting_player: starter,
            turns,
            going_out_player,
            end_of_round_clears,
            round_scores,
            cumulative_scores: self.cumulative_scores.clone(),
            truncated,
        });

        Ok(())
    }

    fn play_turn(&mut self, player: usize, is_final_turn: bool) -> Result<TurnRecord> {
        let view = self.build_view(player, is_final_turn);
        let draw_choice = self.strategies[player].choose_draw(&view, &mut self.rng);

        let (action, column_clears) = match draw_choice {
            DrawChoice::DrawFromDeck => {
                let drawn = self.draw_from_deck()?;
                let view = self.build_view(player, is_final_turn);
                let deck_action =
                    self.strategies[player].choose_deck_draw_action(&view, drawn, &mut self.rng);
                match deck_action {
                    DeckDrawAction::Keep(pos) => {
                        let displaced = self.boards[player].replace(pos, drawn)?;
                        let target = self.rules.discard_target(player);
                        self.discard_piles[target].push(displaced);
                        let clears = self.check_and_clear_columns(player, Some(displaced));
                        (
                            TurnAction::DrewFromDeck {
                                drawn_card: drawn,
                                action: deck_action,
                                displaced_card: Some(displaced),
                            },
                            clears,
                        )
                    }
                    DeckDrawAction::DiscardAndFlip(pos) => {
                        let target = self.rules.discard_target(player);
                        self.discard_piles[target].push(drawn);
                        self.boards[player].flip(pos)?;
                        let clears = self.check_and_clear_columns(player, None);
                        (
                            TurnAction::DrewFromDeck {
                                drawn_card: drawn,
                                action: deck_action,
                                displaced_card: None,
                            },
                            clears,
                        )
                    }
                }
            }
            DrawChoice::DrawFromDiscard(pile) => {
                let drawn = self.discard_piles[pile]
                    .pop()
                    .ok_or(SkyjoError::EmptyDiscardPile)?;
                let view = self.build_view(player, is_final_turn);
                let pos = self.strategies[player].choose_discard_draw_placement(
                    &view,
                    drawn,
                    &mut self.rng,
                );
                let displaced = self.boards[player].replace(pos, drawn)?;
                let target = self.rules.discard_target(player);
                self.discard_piles[target].push(displaced);
                let clears = self.check_and_clear_columns(player, Some(displaced));
                (
                    TurnAction::DrewFromDiscard {
                        pile_index: pile,
                        drawn_card: drawn,
                        placement: pos,
                        displaced_card: displaced,
                    },
                    clears,
                )
            }
        };

        let went_out = self.boards[player].all_revealed();

        Ok(TurnRecord {
            player_index: player,
            action,
            column_clears,
            went_out,
        })
    }

    fn draw_from_deck(&mut self) -> Result<CardValue> {
        if let Some(card) = self.deck.pop() {
            return Ok(card);
        }

        if !self.rules.reshuffle_on_empty_deck() {
            return Err(SkyjoError::EmptyDeck);
        }

        // Reshuffle: take all discard piles entirely, shuffle into deck,
        // then flip top card to start a new discard pile.
        for pile in &mut self.discard_piles {
            self.deck.append(pile);
        }

        if self.deck.is_empty() {
            return Err(SkyjoError::EmptyDeck);
        }

        self.deck.shuffle(&mut self.rng);

        // Start a new discard pile with the top card
        let new_discard = self.deck.pop().unwrap();
        self.discard_piles[0].push(new_discard);

        self.deck.pop().ok_or(SkyjoError::EmptyDeck)
    }

    fn check_and_clear_columns(
        &mut self,
        player: usize,
        displaced_card: Option<CardValue>,
    ) -> Vec<ColumnClearEvent> {
        let num_cols = self.boards[player].num_cols;
        let mut clears = Vec::new();
        for col in 0..num_cols {
            if let Some(val) = self.boards[player].check_column_match(col) {
                let cleared_cards = self.boards[player].clear_column(col);
                let target = self.rules.discard_target(player);
                for card in cleared_cards {
                    self.discard_piles[target].push(card);
                }
                clears.push(ColumnClearEvent {
                    player_index: player,
                    column: col,
                    card_value: val,
                    displaced_card,
                });
            }
        }
        clears
    }

    fn build_view(&self, player: usize, is_final_turn: bool) -> StrategyView {
        StrategyView {
            my_index: player,
            my_board: self.boards[player].visible_view(),
            num_rows: self.boards[player].num_rows,
            num_cols: self.boards[player].num_cols,
            opponent_boards: (0..self.num_players)
                .filter(|&i| i != player)
                .map(|i| self.boards[i].visible_view())
                .collect(),
            opponent_indices: (0..self.num_players).filter(|&i| i != player).collect(),
            discard_piles: self.discard_piles.clone(),
            deck_remaining: self.deck.len(),
            cumulative_scores: self.cumulative_scores.clone(),
            is_final_turn,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::StandardRules;
    use crate::strategies::RandomStrategy;

    fn make_strategies(n: usize) -> Vec<Box<dyn Strategy>> {
        (0..n)
            .map(|_| Box::new(RandomStrategy) as Box<dyn Strategy>)
            .collect()
    }

    #[test]
    fn new_fails_with_too_few_players() {
        let result = Game::new(Box::new(StandardRules), make_strategies(1), 42);
        assert!(matches!(result, Err(SkyjoError::NotEnoughPlayers)));
    }

    #[test]
    fn new_fails_with_zero_players() {
        let result = Game::new(Box::new(StandardRules), make_strategies(0), 42);
        assert!(matches!(result, Err(SkyjoError::NotEnoughPlayers)));
    }

    #[test]
    fn new_fails_with_too_many_players() {
        let result = Game::new(Box::new(StandardRules), make_strategies(9), 42);
        assert!(matches!(result, Err(SkyjoError::TooManyPlayers)));
    }

    #[test]
    fn new_succeeds_with_two_players() {
        let result = Game::new(Box::new(StandardRules), make_strategies(2), 42);
        assert!(result.is_ok());
    }

    #[test]
    fn new_succeeds_with_eight_players() {
        let result = Game::new(Box::new(StandardRules), make_strategies(8), 42);
        assert!(result.is_ok());
    }

    #[test]
    fn new_succeeds_with_all_valid_counts() {
        for n in 2..=8 {
            let result = Game::new(Box::new(StandardRules), make_strategies(n), 42);
            assert!(result.is_ok(), "Game::new should succeed with {n} players");
        }
    }

    #[test]
    fn seeded_rng_produces_deterministic_results() {
        let history1 = Game::new(Box::new(StandardRules), make_strategies(3), 99)
            .unwrap()
            .play()
            .unwrap();
        let history2 = Game::new(Box::new(StandardRules), make_strategies(3), 99)
            .unwrap()
            .play()
            .unwrap();

        assert_eq!(history1.final_scores, history2.final_scores);
        assert_eq!(history1.winners, history2.winners);
        assert_eq!(history1.rounds.len(), history2.rounds.len());
        for (r1, r2) in history1.rounds.iter().zip(history2.rounds.iter()) {
            assert_eq!(r1.initial_deck_order, r2.initial_deck_order);
            assert_eq!(r1.round_scores, r2.round_scores);
            assert_eq!(r1.turns.len(), r2.turns.len());
        }
    }

    #[test]
    fn different_seeds_produce_different_results() {
        let history1 = Game::new(Box::new(StandardRules), make_strategies(2), 1)
            .unwrap()
            .play()
            .unwrap();
        let history2 = Game::new(Box::new(StandardRules), make_strategies(2), 2)
            .unwrap()
            .play()
            .unwrap();

        // Different seeds should produce different deck orders
        assert_ne!(
            history1.rounds[0].initial_deck_order,
            history2.rounds[0].initial_deck_order
        );
    }
}
