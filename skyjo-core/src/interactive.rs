use rand::seq::SliceRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::board::PlayerBoard;
use crate::card::{CardValue, Slot, VisibleSlot};
use crate::error::{Result, SkyjoError};
use crate::history::ColumnClearEvent;
use crate::rules::Rules;
use crate::strategy::{DeckDrawAction, DrawChoice, Strategy, StrategyView};

/// What the game needs from the current player.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionNeeded {
    ChooseInitialFlips {
        player: usize,
        count: usize,
    },
    ChooseDraw {
        player: usize,
        /// Which discard piles can this player draw from.
        drawable_piles: Vec<usize>,
    },
    ChooseDeckDrawAction {
        player: usize,
        drawn_card: CardValue,
    },
    ChooseDiscardDrawPlacement {
        player: usize,
        drawn_card: CardValue,
    },
    RoundOver {
        round_number: usize,
        round_scores: Vec<i32>,
        cumulative_scores: Vec<i32>,
        going_out_player: Option<usize>,
        end_of_round_clears: Vec<ColumnClearEvent>,
    },
    GameOver {
        final_scores: Vec<i32>,
        winners: Vec<usize>,
        round_number: usize,
        round_scores: Vec<i32>,
        going_out_player: Option<usize>,
        end_of_round_clears: Vec<ColumnClearEvent>,
    },
}

/// An action submitted by a player.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlayerAction {
    InitialFlip { position: usize },
    DrawFromDeck,
    DrawFromDiscard { pile_index: usize },
    UndoDrawFromDiscard,
    KeepDeckDraw { position: usize },
    DiscardAndFlip { position: usize },
    PlaceDiscardDraw { position: usize },
    ContinueToNextRound,
}

/// Serializable game state for a specific player's view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveGameState {
    pub num_players: usize,
    pub player_names: Vec<String>,
    pub num_rows: usize,
    pub num_cols: usize,
    pub round_number: usize,
    pub current_player: usize,
    pub action_needed: ActionNeeded,
    pub boards: Vec<Vec<VisibleSlot>>,
    pub discard_tops: Vec<Option<CardValue>>,
    pub discard_sizes: Vec<usize>,
    pub deck_remaining: usize,
    pub cumulative_scores: Vec<i32>,
    pub going_out_player: Option<usize>,
    pub is_final_turn: bool,
    pub last_column_clears: Vec<ColumnClearEvent>,
}

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    SetupFlips { next_player: usize, flips_remaining: usize },
    WaitingForDraw,
    WaitingForDeckDrawAction { drawn_card: CardValue },
    WaitingForDiscardPlacement { drawn_card: CardValue, source_pile: usize },
    RoundOver,
    GameOver,
}

/// Interactive, step-based game engine for human play.
pub struct InteractiveGame {
    rules: Box<dyn Rules>,
    rng: StdRng,
    num_players: usize,
    player_names: Vec<String>,

    // Game state
    boards: Vec<PlayerBoard>,
    deck: Vec<CardValue>,
    discard_piles: Vec<Vec<CardValue>>,
    cumulative_scores: Vec<i32>,

    // Round tracking
    round_number: usize,
    phase: Phase,
    going_out_player: Option<usize>,
    turns_after_goer: usize,
    current_player: usize,
    starter: usize,

    // Cross-round state
    last_round_goer: Option<usize>,

    // Last column clears for UI feedback
    last_column_clears: Vec<ColumnClearEvent>,

    // End-of-round data
    last_round_scores: Vec<i32>,
    last_end_of_round_clears: Vec<ColumnClearEvent>,
}

impl InteractiveGame {
    pub fn new(
        rules: Box<dyn Rules>,
        num_players: usize,
        player_names: Vec<String>,
        seed: u64,
    ) -> Result<Self> {
        if num_players < 2 {
            return Err(SkyjoError::NotEnoughPlayers);
        }
        if num_players > 8 {
            return Err(SkyjoError::TooManyPlayers);
        }

        let mut game = InteractiveGame {
            rng: StdRng::seed_from_u64(seed),
            num_players,
            player_names,
            boards: Vec::new(),
            deck: Vec::new(),
            discard_piles: Vec::new(),
            cumulative_scores: vec![0; num_players],
            round_number: 0,
            phase: Phase::SetupFlips { next_player: 0, flips_remaining: 0 }, // overwritten by deal_round
            going_out_player: None,
            turns_after_goer: 0,
            current_player: 0,
            starter: 0,
            last_round_goer: None,
            last_column_clears: Vec::new(),
            last_round_scores: Vec::new(),
            last_end_of_round_clears: Vec::new(),
            rules,
        };

        game.deal_round()?;
        Ok(game)
    }

    fn deal_round(&mut self) -> Result<()> {
        // Build & shuffle deck
        let mut deck = self.rules.build_deck();
        deck.shuffle(&mut self.rng);

        let cards_per_player = self.rules.num_cards_per_player();
        let num_rows = self.rules.num_rows();
        let num_cols = self.rules.num_cols();

        // Deal cards to each player
        let mut dealt_hands: Vec<Vec<CardValue>> = Vec::with_capacity(self.num_players);
        for _ in 0..self.num_players {
            let mut hand = Vec::with_capacity(cards_per_player);
            for _ in 0..cards_per_player {
                hand.push(deck.pop().expect("deck too small for dealing"));
            }
            dealt_hands.push(hand);
        }

        // Initialize boards (all hidden)
        self.boards = dealt_hands
            .iter()
            .map(|h| PlayerBoard::new(h, num_rows, num_cols))
            .collect();

        // First discard: pop one card from deck onto discard pile 0
        let first_discard = deck.pop().expect("deck empty after dealing");
        let pile_count = self.rules.discard_pile_count(self.num_players);
        self.discard_piles = vec![Vec::new(); pile_count];
        self.discard_piles[0].push(first_discard);
        self.deck = deck;

        // Reset round state
        self.going_out_player = None;
        self.turns_after_goer = 0;
        self.last_column_clears.clear();

        // Start with setup flips for player 0
        self.phase = Phase::SetupFlips { next_player: 0, flips_remaining: self.rules.initial_flips() };

        Ok(())
    }

    /// Get the current action needed from the game.
    pub fn get_action_needed(&self) -> ActionNeeded {
        match &self.phase {
            Phase::SetupFlips { next_player, flips_remaining } => ActionNeeded::ChooseInitialFlips {
                player: *next_player,
                count: *flips_remaining,
            },
            Phase::WaitingForDraw => ActionNeeded::ChooseDraw {
                player: self.current_player,
                drawable_piles: self.rules.drawable_piles(self.current_player, self.num_players),
            },
            Phase::WaitingForDeckDrawAction { drawn_card } => ActionNeeded::ChooseDeckDrawAction {
                player: self.current_player,
                drawn_card: *drawn_card,
            },
            Phase::WaitingForDiscardPlacement { drawn_card, .. } => {
                ActionNeeded::ChooseDiscardDrawPlacement {
                    player: self.current_player,
                    drawn_card: *drawn_card,
                }
            }
            Phase::RoundOver => ActionNeeded::RoundOver {
                round_number: self.round_number,
                round_scores: self.last_round_scores.clone(),
                cumulative_scores: self.cumulative_scores.clone(),
                going_out_player: self.going_out_player,
                end_of_round_clears: self.last_end_of_round_clears.clone(),
            },
            Phase::GameOver => {
                let winners = self.rules.resolve_winners(&self.cumulative_scores);
                ActionNeeded::GameOver {
                    final_scores: self.cumulative_scores.clone(),
                    winners,
                    round_number: self.round_number,
                    round_scores: self.last_round_scores.clone(),
                    going_out_player: self.going_out_player,
                    end_of_round_clears: self.last_end_of_round_clears.clone(),
                }
            }
        }
    }

    /// Apply a player action and advance the game state.
    /// Returns the resulting ActionNeeded.
    pub fn apply_action(&mut self, action: PlayerAction) -> Result<ActionNeeded> {
        self.last_column_clears.clear();

        match (&self.phase, action) {
            (Phase::SetupFlips { next_player, flips_remaining }, PlayerAction::InitialFlip { position }) => {
                let player = *next_player;
                let remaining = *flips_remaining;
                self.boards[player].flip(position)?;

                if remaining > 1 {
                    // Same player, one fewer flip remaining
                    self.phase = Phase::SetupFlips { next_player: player, flips_remaining: remaining - 1 };
                } else {
                    // This player is done — move to next player or start the game
                    let next = player + 1;
                    if next < self.num_players {
                        self.phase = Phase::SetupFlips {
                            next_player: next,
                            flips_remaining: self.rules.initial_flips(),
                        };
                    } else {
                        // All players have flipped — determine starting player
                        self.starter = if self.round_number == 0 {
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
                            self.last_round_goer.unwrap_or(0)
                        };
                        self.current_player = self.starter;
                        self.phase = Phase::WaitingForDraw;
                    }
                }
            }

            (Phase::WaitingForDraw, PlayerAction::DrawFromDeck) => {
                let drawn = self.draw_from_deck()?;
                self.phase = Phase::WaitingForDeckDrawAction { drawn_card: drawn };
            }

            (Phase::WaitingForDraw, PlayerAction::DrawFromDiscard { pile_index }) => {
                let drawable = self.rules.drawable_piles(self.current_player, self.num_players);
                if !drawable.contains(&pile_index) {
                    return Err(SkyjoError::EmptyDiscardPile);
                }
                let drawn = self.discard_piles[pile_index]
                    .pop()
                    .ok_or(SkyjoError::EmptyDiscardPile)?;
                self.phase = Phase::WaitingForDiscardPlacement { drawn_card: drawn, source_pile: pile_index };
            }

            (Phase::WaitingForDeckDrawAction { drawn_card }, PlayerAction::KeepDeckDraw { position }) => {
                let drawn = *drawn_card;
                let displaced = self.boards[self.current_player].replace(position, drawn)?;
                let target = self.rules.discard_target(self.current_player);
                self.discard_piles[target].push(displaced);
                self.last_column_clears = self.check_and_clear_columns(self.current_player, Some(displaced));
                self.advance_turn();
            }

            (Phase::WaitingForDeckDrawAction { drawn_card }, PlayerAction::DiscardAndFlip { position }) => {
                let drawn = *drawn_card;
                let target = self.rules.discard_target(self.current_player);
                self.discard_piles[target].push(drawn);
                self.boards[self.current_player].flip(position)?;
                self.last_column_clears = self.check_and_clear_columns(self.current_player, None);
                self.advance_turn();
            }

            (Phase::WaitingForDiscardPlacement { drawn_card, source_pile }, PlayerAction::UndoDrawFromDiscard) => {
                let drawn = *drawn_card;
                let pile = *source_pile;
                self.discard_piles[pile].push(drawn);
                self.phase = Phase::WaitingForDraw;
            }

            (Phase::WaitingForDiscardPlacement { drawn_card, .. }, PlayerAction::PlaceDiscardDraw { position }) => {
                let drawn = *drawn_card;
                let displaced = self.boards[self.current_player].replace(position, drawn)?;
                let target = self.rules.discard_target(self.current_player);
                self.discard_piles[target].push(displaced);
                self.last_column_clears = self.check_and_clear_columns(self.current_player, Some(displaced));
                self.advance_turn();
            }

            (Phase::RoundOver, PlayerAction::ContinueToNextRound) => {
                self.round_number += 1;
                self.deal_round()?;
            }

            (Phase::GameOver, _) => {
                return Err(SkyjoError::GameAlreadyOver);
            }

            _ => {
                // Invalid action for current phase
                return Err(SkyjoError::InvalidPosition(0));
            }
        }

        Ok(self.get_action_needed())
    }

    fn advance_turn(&mut self) {
        let player = self.current_player;

        // Check if this player went out
        if self.boards[player].all_revealed() && self.going_out_player.is_none() {
            self.going_out_player = Some(player);
        }

        // Check if the round is over
        if let Some(goer) = self.going_out_player {
            if goer == player && self.boards[player].all_revealed() && self.turns_after_goer == 0 {
                // Player just went out this turn — don't count as a turn after
            } else {
                self.turns_after_goer += 1;
            }

            if self.turns_after_goer >= self.num_players - 1 {
                self.end_round();
                return;
            }
        }

        // Move to next player, skipping the going-out player
        loop {
            self.current_player = (self.current_player + 1) % self.num_players;
            if self.going_out_player != Some(self.current_player) {
                break;
            }
        }

        self.phase = Phase::WaitingForDraw;
    }

    fn end_round(&mut self) {
        let num_cols = self.rules.num_cols();

        // Reveal all hidden cards and check column clearing
        let mut end_of_round_clears: Vec<ColumnClearEvent> = Vec::new();
        for (player_idx, board) in self.boards.iter_mut().enumerate() {
            // Reveal all hidden
            for slot in board.slots.iter_mut() {
                if let Slot::Hidden(v) = *slot {
                    *slot = Slot::Revealed(v);
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

        // Compute raw scores
        let mut round_scores: Vec<i32> = self.boards.iter().map(|b| b.score()).collect();

        // Apply going-out penalty
        if let Some(goer) = self.going_out_player {
            let goer_score = round_scores[goer];
            let is_solo_lowest = round_scores
                .iter()
                .enumerate()
                .all(|(i, &s)| i == goer || s > goer_score);
            let min_other = round_scores
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != goer)
                .map(|(_, &s)| s)
                .min()
                .unwrap_or(goer_score);
            round_scores[goer] =
                self.rules
                    .apply_going_out_penalty(goer_score, min_other, is_solo_lowest);
        }

        // Update cumulative scores
        for (cum, round) in self.cumulative_scores.iter_mut().zip(&round_scores) {
            *cum += round;
        }

        self.last_round_goer = self.going_out_player;
        self.last_round_scores = round_scores;
        self.last_end_of_round_clears = end_of_round_clears;

        // Check if game is over
        if self
            .cumulative_scores
            .iter()
            .any(|&s| s >= self.rules.end_threshold())
        {
            self.phase = Phase::GameOver;
        } else {
            self.phase = Phase::RoundOver;
        }
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

    fn check_and_clear_columns(&mut self, player: usize, displaced_card: Option<CardValue>) -> Vec<ColumnClearEvent> {
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

    /// Get the full game state (all cards visible — for local multiplayer).
    pub fn get_full_state(&self) -> InteractiveGameState {
        let boards: Vec<Vec<VisibleSlot>> = self
            .boards
            .iter()
            .map(|b| {
                b.slots
                    .iter()
                    .map(|s| match s {
                        Slot::Hidden(_) => VisibleSlot::Hidden,
                        Slot::Revealed(v) => VisibleSlot::Revealed(*v),
                        Slot::Cleared => VisibleSlot::Cleared,
                    })
                    .collect()
            })
            .collect();

        let discard_tops: Vec<Option<CardValue>> = self
            .discard_piles
            .iter()
            .map(|p| p.last().copied())
            .collect();

        let discard_sizes: Vec<usize> = self.discard_piles.iter().map(|p| p.len()).collect();

        InteractiveGameState {
            num_players: self.num_players,
            player_names: self.player_names.clone(),
            num_rows: self.rules.num_rows(),
            num_cols: self.rules.num_cols(),
            round_number: self.round_number,
            current_player: self.current_player,
            action_needed: self.get_action_needed(),
            boards,
            discard_tops,
            discard_sizes,
            deck_remaining: self.deck.len(),
            cumulative_scores: self.cumulative_scores.clone(),
            going_out_player: self.going_out_player,
            is_final_turn: self.going_out_player.is_some(),
            last_column_clears: self.last_column_clears.clone(),
        }
    }

    /// Get the game state from a specific player's perspective (hides other players' hidden cards).
    pub fn get_player_state(&self, _player: usize) -> InteractiveGameState {
        let boards: Vec<Vec<VisibleSlot>> = self
            .boards
            .iter()
            .map(|b| b.visible_view())
            .collect();

        let discard_tops: Vec<Option<CardValue>> = self
            .discard_piles
            .iter()
            .map(|p| p.last().copied())
            .collect();

        let discard_sizes: Vec<usize> = self.discard_piles.iter().map(|p| p.len()).collect();

        InteractiveGameState {
            num_players: self.num_players,
            player_names: self.player_names.clone(),
            num_rows: self.rules.num_rows(),
            num_cols: self.rules.num_cols(),
            round_number: self.round_number,
            current_player: self.current_player,
            action_needed: self.get_action_needed(),
            boards,
            discard_tops,
            discard_sizes,
            deck_remaining: self.deck.len(),
            cumulative_scores: self.cumulative_scores.clone(),
            going_out_player: self.going_out_player,
            is_final_turn: self.going_out_player.is_some(),
            last_column_clears: self.last_column_clears.clone(),
        }
    }

    /// Compute the action a bot strategy would take in the current game state.
    /// Uses the game's RNG for deterministic bot behavior.
    pub fn get_bot_action(&mut self, strategy: &dyn Strategy) -> Result<PlayerAction> {
        match &self.phase {
            Phase::SetupFlips { next_player, flips_remaining } => {
                let player = *next_player;
                let remaining = *flips_remaining;
                let view = self.build_strategy_view(player);
                let positions = strategy.choose_initial_flips(&view, remaining, &mut self.rng);
                if positions.is_empty() {
                    return Err(SkyjoError::InvalidAction("Strategy returned no flip positions".into()));
                }
                Ok(PlayerAction::InitialFlip { position: positions[0] })
            }
            Phase::WaitingForDraw => {
                let view = self.build_strategy_view(self.current_player);
                let choice = strategy.choose_draw(&view, &mut self.rng);
                match choice {
                    DrawChoice::DrawFromDeck => Ok(PlayerAction::DrawFromDeck),
                    DrawChoice::DrawFromDiscard(pile_index) => Ok(PlayerAction::DrawFromDiscard { pile_index }),
                }
            }
            Phase::WaitingForDeckDrawAction { drawn_card } => {
                let drawn_card = *drawn_card;
                let view = self.build_strategy_view(self.current_player);
                let action = strategy.choose_deck_draw_action(&view, drawn_card, &mut self.rng);
                match action {
                    DeckDrawAction::Keep(position) => Ok(PlayerAction::KeepDeckDraw { position }),
                    DeckDrawAction::DiscardAndFlip(position) => Ok(PlayerAction::DiscardAndFlip { position }),
                }
            }
            Phase::WaitingForDiscardPlacement { drawn_card, .. } => {
                let drawn_card = *drawn_card;
                let view = self.build_strategy_view(self.current_player);
                let position = strategy.choose_discard_draw_placement(&view, drawn_card, &mut self.rng);
                Ok(PlayerAction::PlaceDiscardDraw { position })
            }
            Phase::RoundOver => Ok(PlayerAction::ContinueToNextRound),
            Phase::GameOver => Err(SkyjoError::InvalidAction("Game is over, no action needed".into())),
        }
    }

    /// Build a StrategyView for the given player from the current game state.
    /// All boards are shown as visible slots (hidden cards have no value revealed).
    fn build_strategy_view(&self, player: usize) -> StrategyView {
        let my_board = self.boards[player].visible_view();
        let mut opponent_boards = Vec::with_capacity(self.num_players - 1);
        let mut opponent_indices = Vec::with_capacity(self.num_players - 1);
        for i in 0..self.num_players {
            if i != player {
                opponent_boards.push(self.boards[i].visible_view());
                opponent_indices.push(i);
            }
        }
        StrategyView {
            my_index: player,
            my_board,
            num_rows: self.rules.num_rows(),
            num_cols: self.rules.num_cols(),
            opponent_boards,
            opponent_indices,
            discard_piles: self.discard_piles.clone(),
            deck_remaining: self.deck.len(),
            cumulative_scores: self.cumulative_scores.clone(),
            is_final_turn: self.going_out_player.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::StandardRules;

    #[test]
    fn interactive_game_basic_flow() {
        let game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Alice".to_string(), "Bob".to_string()],
            42,
        )
        .unwrap();

        // Should start with setup flips for player 0
        match game.get_action_needed() {
            ActionNeeded::ChooseInitialFlips { player, count } => {
                assert_eq!(player, 0);
                assert_eq!(count, 2);
            }
            other => panic!("Expected ChooseInitialFlips, got {:?}", other),
        }
    }

    #[test]
    fn interactive_game_setup_flips_advance() {
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Alice".to_string(), "Bob".to_string()],
            42,
        )
        .unwrap();

        // Player 0 first flip
        let result = game
            .apply_action(PlayerAction::InitialFlip { position: 0 })
            .unwrap();

        // Should still need player 0's second flip
        match result {
            ActionNeeded::ChooseInitialFlips { player, count } => {
                assert_eq!(player, 0);
                assert_eq!(count, 1);
            }
            other => panic!("Expected ChooseInitialFlips for player 0 (second flip), got {:?}", other),
        }

        // Player 0 second flip
        let result = game
            .apply_action(PlayerAction::InitialFlip { position: 1 })
            .unwrap();

        // Should now need player 1 flips
        match result {
            ActionNeeded::ChooseInitialFlips { player, count } => {
                assert_eq!(player, 1);
                assert_eq!(count, 2);
            }
            other => panic!("Expected ChooseInitialFlips for player 1, got {:?}", other),
        }

        // Player 1 first flip
        game.apply_action(PlayerAction::InitialFlip { position: 0 }).unwrap();

        // Player 1 second flip
        let result = game
            .apply_action(PlayerAction::InitialFlip { position: 1 })
            .unwrap();

        // Should now be waiting for draw
        match result {
            ActionNeeded::ChooseDraw { player, .. } => {
                // Player should be the one determined by starting player logic
                assert!(player < 2);
            }
            other => panic!("Expected ChooseDraw, got {:?}", other),
        }
    }

    #[test]
    fn interactive_game_draw_and_place() {
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Alice".to_string(), "Bob".to_string()],
            42,
        )
        .unwrap();

        // Setup flips for both players (one at a time)
        game.apply_action(PlayerAction::InitialFlip { position: 0 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 1 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 0 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 1 }).unwrap();

        // Draw from deck
        let result = game.apply_action(PlayerAction::DrawFromDeck).unwrap();
        match &result {
            ActionNeeded::ChooseDeckDrawAction { drawn_card, .. } => {
                assert!((-2..=12).contains(drawn_card));
            }
            other => panic!("Expected ChooseDeckDrawAction, got {:?}", other),
        }

        // Keep the card at position 2
        let result = game
            .apply_action(PlayerAction::KeepDeckDraw { position: 2 })
            .unwrap();

        // Should now be the other player's turn
        match result {
            ActionNeeded::ChooseDraw { .. } => {}
            other => panic!("Expected ChooseDraw for next player, got {:?}", other),
        }
    }

    #[test]
    fn interactive_game_discard_and_flip() {
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Alice".to_string(), "Bob".to_string()],
            42,
        )
        .unwrap();

        // Setup flips (one at a time)
        game.apply_action(PlayerAction::InitialFlip { position: 0 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 1 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 0 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 1 }).unwrap();

        // Draw from deck
        game.apply_action(PlayerAction::DrawFromDeck).unwrap();

        // Discard and flip position 3 (a hidden card)
        let result = game
            .apply_action(PlayerAction::DiscardAndFlip { position: 3 })
            .unwrap();

        match result {
            ActionNeeded::ChooseDraw { .. } => {}
            other => panic!("Expected ChooseDraw, got {:?}", other),
        }
    }

    #[test]
    fn interactive_game_draw_from_discard() {
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Alice".to_string(), "Bob".to_string()],
            42,
        )
        .unwrap();

        // Setup flips (one at a time)
        game.apply_action(PlayerAction::InitialFlip { position: 0 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 1 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 0 }).unwrap();
        game.apply_action(PlayerAction::InitialFlip { position: 1 }).unwrap();

        // Draw from discard
        let result = game
            .apply_action(PlayerAction::DrawFromDiscard { pile_index: 0 })
            .unwrap();

        match &result {
            ActionNeeded::ChooseDiscardDrawPlacement { drawn_card, .. } => {
                assert!((-2..=12).contains(drawn_card));
            }
            other => panic!("Expected ChooseDiscardDrawPlacement, got {:?}", other),
        }

        // Place on board
        let result = game
            .apply_action(PlayerAction::PlaceDiscardDraw { position: 2 })
            .unwrap();

        match result {
            ActionNeeded::ChooseDraw { .. } => {}
            other => panic!("Expected ChooseDraw, got {:?}", other),
        }
    }

    #[test]
    fn interactive_game_full_state() {
        let game = InteractiveGame::new(
            Box::new(StandardRules),
            3,
            vec![
                "Alice".to_string(),
                "Bob".to_string(),
                "Charlie".to_string(),
            ],
            42,
        )
        .unwrap();

        let state = game.get_full_state();
        assert_eq!(state.num_players, 3);
        assert_eq!(state.player_names.len(), 3);
        assert_eq!(state.boards.len(), 3);
        assert_eq!(state.boards[0].len(), 12); // 3x4
        assert_eq!(state.num_rows, 3);
        assert_eq!(state.num_cols, 4);
        assert_eq!(state.round_number, 0);
    }

    /// Play a complete game through the interactive API and verify it finishes.
    #[test]
    fn interactive_game_plays_to_completion() {
        use rand::Rng;

        let seed = 123;
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["P1".to_string(), "P2".to_string()],
            seed,
        )
        .unwrap();

        let mut rng = StdRng::seed_from_u64(seed + 1000); // separate rng for random choices
        let mut max_actions = 10_000;

        loop {
            let action_needed = game.get_action_needed();
            let action = match action_needed {
                ActionNeeded::ChooseInitialFlips { player, .. } => {
                    // Pick the first available hidden position for this player
                    let state = game.get_full_state();
                    let board = &state.boards[player];
                    let pos = board
                        .iter()
                        .position(|s| matches!(s, VisibleSlot::Hidden))
                        .unwrap();
                    PlayerAction::InitialFlip { position: pos }
                }
                ActionNeeded::ChooseDraw { drawable_piles, .. } => {
                    if rng.gen_bool(0.5) {
                        PlayerAction::DrawFromDeck
                    } else {
                        PlayerAction::DrawFromDiscard {
                            pile_index: drawable_piles[0],
                        }
                    }
                }
                ActionNeeded::ChooseDeckDrawAction { .. } => {
                    if rng.gen_bool(0.5) {
                        // Find a non-cleared position
                        let state = game.get_full_state();
                        let board = &state.boards[state.current_player];
                        let pos = board
                            .iter()
                            .position(|s| !matches!(s, VisibleSlot::Cleared))
                            .unwrap();
                        PlayerAction::KeepDeckDraw { position: pos }
                    } else {
                        // Find a hidden position
                        let state = game.get_full_state();
                        let board = &state.boards[state.current_player];
                        let pos = board
                            .iter()
                            .position(|s| matches!(s, VisibleSlot::Hidden))
                            .unwrap_or_else(|| {
                                board
                                    .iter()
                                    .position(|s| !matches!(s, VisibleSlot::Cleared))
                                    .unwrap()
                            });
                        if matches!(board[pos], VisibleSlot::Hidden) {
                            PlayerAction::DiscardAndFlip { position: pos }
                        } else {
                            PlayerAction::KeepDeckDraw { position: pos }
                        }
                    }
                }
                ActionNeeded::ChooseDiscardDrawPlacement { .. } => {
                    let state = game.get_full_state();
                    let board = &state.boards[state.current_player];
                    let pos = board
                        .iter()
                        .position(|s| !matches!(s, VisibleSlot::Cleared))
                        .unwrap();
                    PlayerAction::PlaceDiscardDraw { position: pos }
                }
                ActionNeeded::RoundOver { .. } => PlayerAction::ContinueToNextRound,
                ActionNeeded::GameOver {
                    winners,
                    final_scores,
                    ..
                } => {
                    assert!(!winners.is_empty());
                    assert_eq!(final_scores.len(), 2);
                    // At least one score should be >= 100
                    assert!(final_scores.iter().any(|&s| s >= 100));
                    return; // Test passes
                }
            };

            game.apply_action(action).unwrap();

            max_actions -= 1;
            assert!(max_actions > 0, "Game did not complete in time");
        }
    }

    #[test]
    fn bot_action_random_produces_valid_actions() {
        use crate::strategies::RandomStrategy;
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Bot1".to_string(), "Bot2".to_string()],
            42,
        )
        .unwrap();

        let strategy = RandomStrategy;
        // Play through initial flips (2 per player = 4 actions)
        for _ in 0..4 {
            let action = game.get_bot_action(&strategy).unwrap();
            assert!(matches!(action, PlayerAction::InitialFlip { .. }));
            game.apply_action(action).unwrap();
        }

        // Should now be in playing phase
        let action_needed = game.get_action_needed();
        assert!(matches!(action_needed, ActionNeeded::ChooseDraw { .. }));

        // Get a draw action
        let action = game.get_bot_action(&strategy).unwrap();
        assert!(matches!(action, PlayerAction::DrawFromDeck | PlayerAction::DrawFromDiscard { .. }));
        game.apply_action(action).unwrap();
    }

    #[test]
    fn bot_action_greedy_produces_valid_actions() {
        use crate::strategies::GreedyStrategy;
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Bot1".to_string(), "Bot2".to_string()],
            99,
        )
        .unwrap();

        let strategy = GreedyStrategy;
        // Play through initial flips
        for _ in 0..4 {
            let action = game.get_bot_action(&strategy).unwrap();
            assert!(matches!(action, PlayerAction::InitialFlip { .. }));
            game.apply_action(action).unwrap();
        }

        // Play a full turn
        let action = game.get_bot_action(&strategy).unwrap();
        assert!(matches!(action, PlayerAction::DrawFromDeck | PlayerAction::DrawFromDiscard { .. }));
        game.apply_action(action).unwrap();

        let action = game.get_bot_action(&strategy).unwrap();
        assert!(matches!(
            action,
            PlayerAction::KeepDeckDraw { .. }
            | PlayerAction::DiscardAndFlip { .. }
            | PlayerAction::PlaceDiscardDraw { .. }
        ));
        game.apply_action(action).unwrap();
    }

    #[test]
    fn bot_full_game_plays_to_completion() {
        use crate::strategies::GreedyStrategy;
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            3,
            vec!["Bot1".to_string(), "Bot2".to_string(), "Bot3".to_string()],
            777,
        )
        .unwrap();

        let strategy = GreedyStrategy;
        let mut max_actions = 10_000;

        loop {
            let action_needed = game.get_action_needed();
            if matches!(action_needed, ActionNeeded::GameOver { .. }) {
                break;
            }

            let action = game.get_bot_action(&strategy).unwrap();
            game.apply_action(action).unwrap();

            max_actions -= 1;
            assert!(max_actions > 0, "Bot game did not complete in time");
        }

        if let ActionNeeded::GameOver { winners, final_scores, .. } = game.get_action_needed() {
            assert!(!winners.is_empty());
            assert_eq!(final_scores.len(), 3);
            assert!(final_scores.iter().any(|&s| s >= 100));
        } else {
            panic!("Expected GameOver");
        }
    }

    #[test]
    fn bot_action_returns_error_on_game_over() {
        use crate::strategies::RandomStrategy;
        let mut game = InteractiveGame::new(
            Box::new(StandardRules),
            2,
            vec!["Bot1".to_string(), "Bot2".to_string()],
            777,
        )
        .unwrap();

        let strategy = RandomStrategy;
        let mut max_actions = 10_000;

        loop {
            let action_needed = game.get_action_needed();
            if matches!(action_needed, ActionNeeded::GameOver { .. }) {
                break;
            }
            let action = game.get_bot_action(&strategy).unwrap();
            game.apply_action(action).unwrap();
            max_actions -= 1;
            assert!(max_actions > 0, "Game did not complete");
        }

        // Now get_bot_action should error
        let result = game.get_bot_action(&strategy);
        assert!(result.is_err());
    }
}
