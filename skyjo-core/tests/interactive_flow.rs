use skyjo_core::*;

fn new_2p_game(seed: u64) -> InteractiveGame {
    InteractiveGame::new(
        Box::new(StandardRules),
        2,
        vec!["Alice".into(), "Bob".into()],
        seed,
    )
    .unwrap()
}

/// Complete all initial flips for both players (2 each for StandardRules).
fn do_all_flips(game: &mut InteractiveGame) {
    for _ in 0..4 {
        match game.get_action_needed() {
            ActionNeeded::ChooseInitialFlips { player, .. } => {
                let state = game.get_full_state();
                let board = &state.boards[player];
                let pos = board
                    .iter()
                    .position(|s| matches!(s, VisibleSlot::Hidden))
                    .unwrap();
                game.apply_action(PlayerAction::InitialFlip { position: pos })
                    .unwrap();
            }
            other => panic!("Expected ChooseInitialFlips during setup, got {:?}", other),
        }
    }
}

#[test]
fn initial_state_needs_flips() {
    let game = new_2p_game(42);
    match game.get_action_needed() {
        ActionNeeded::ChooseInitialFlips { player, count } => {
            assert_eq!(player, 0);
            assert_eq!(count, 2);
        }
        other => panic!("Expected ChooseInitialFlips for player 0, got {:?}", other),
    }
}

#[test]
fn initial_flips_advance_to_next_player() {
    let mut game = new_2p_game(42);

    // Player 0 flips two cards
    game.apply_action(PlayerAction::InitialFlip { position: 0 })
        .unwrap();
    game.apply_action(PlayerAction::InitialFlip { position: 1 })
        .unwrap();

    // Should now request flips from player 1
    match game.get_action_needed() {
        ActionNeeded::ChooseInitialFlips { player, count } => {
            assert_eq!(player, 1);
            assert_eq!(count, 2);
        }
        other => panic!("Expected ChooseInitialFlips for player 1, got {:?}", other),
    }
}

#[test]
fn all_flips_done_transitions_to_draw() {
    let mut game = new_2p_game(42);
    do_all_flips(&mut game);

    match game.get_action_needed() {
        ActionNeeded::ChooseDraw { player, .. } => {
            assert!(player < 2);
        }
        other => panic!("Expected ChooseDraw after all flips, got {:?}", other),
    }
}

#[test]
fn draw_from_deck_then_keep() {
    let mut game = new_2p_game(42);
    do_all_flips(&mut game);

    let first_player = match game.get_action_needed() {
        ActionNeeded::ChooseDraw { player, .. } => player,
        other => panic!("Expected ChooseDraw, got {:?}", other),
    };

    // Draw from deck
    let result = game.apply_action(PlayerAction::DrawFromDeck).unwrap();
    match result {
        ActionNeeded::ChooseDeckDrawAction { player, drawn_card } => {
            assert_eq!(player, first_player);
            assert!((-2..=12).contains(&drawn_card));
        }
        other => panic!("Expected ChooseDeckDrawAction, got {:?}", other),
    }

    // Keep at position 2 (a non-cleared slot)
    let result = game
        .apply_action(PlayerAction::KeepDeckDraw { position: 2 })
        .unwrap();

    // Should advance to the next player's draw
    match result {
        ActionNeeded::ChooseDraw { player, .. } => {
            assert_ne!(player, first_player);
        }
        other => panic!("Expected ChooseDraw for next player, got {:?}", other),
    }
}

#[test]
fn draw_from_deck_then_discard_flip() {
    let mut game = new_2p_game(42);
    do_all_flips(&mut game);

    let first_player = match game.get_action_needed() {
        ActionNeeded::ChooseDraw { player, .. } => player,
        other => panic!("Expected ChooseDraw, got {:?}", other),
    };

    game.apply_action(PlayerAction::DrawFromDeck).unwrap();

    // Find a hidden position to flip
    let state = game.get_full_state();
    let board = &state.boards[first_player];
    let hidden_pos = board
        .iter()
        .position(|s| matches!(s, VisibleSlot::Hidden))
        .unwrap();

    let result = game
        .apply_action(PlayerAction::DiscardAndFlip { position: hidden_pos })
        .unwrap();

    match result {
        ActionNeeded::ChooseDraw { player, .. } => {
            assert_ne!(player, first_player);
        }
        other => panic!("Expected ChooseDraw for next player, got {:?}", other),
    }
}

#[test]
fn draw_from_discard_place() {
    let mut game = new_2p_game(42);
    do_all_flips(&mut game);

    let first_player = match game.get_action_needed() {
        ActionNeeded::ChooseDraw { player, .. } => player,
        other => panic!("Expected ChooseDraw, got {:?}", other),
    };

    let result = game
        .apply_action(PlayerAction::DrawFromDiscard { pile_index: 0 })
        .unwrap();

    match &result {
        ActionNeeded::ChooseDiscardDrawPlacement { player, drawn_card } => {
            assert_eq!(*player, first_player);
            assert!((-2..=12).contains(drawn_card));
        }
        other => panic!("Expected ChooseDiscardDrawPlacement, got {:?}", other),
    }

    let result = game
        .apply_action(PlayerAction::PlaceDiscardDraw { position: 2 })
        .unwrap();

    match result {
        ActionNeeded::ChooseDraw { player, .. } => {
            assert_ne!(player, first_player);
        }
        other => panic!("Expected ChooseDraw for next player, got {:?}", other),
    }
}

#[test]
fn full_game_with_strategy() {
    let mut game = InteractiveGame::new(
        Box::new(StandardRules),
        2,
        vec!["Alice".into(), "Bob".into()],
        99,
    )
    .unwrap();

    let strategy = RandomStrategy;
    let mut max_actions = 10_000;

    loop {
        let action_needed = game.get_action_needed();
        if let ActionNeeded::GameOver {
            final_scores,
            winners,
            ..
        } = action_needed
        {
            assert!(!winners.is_empty(), "winners should be populated");
            assert_eq!(final_scores.len(), 2, "should have scores for both players");
            return;
        }

        let action = game.get_bot_action(&strategy).unwrap();
        game.apply_action(action).unwrap();

        max_actions -= 1;
        assert!(max_actions > 0, "Game did not complete in time");
    }
}

#[test]
fn invalid_action_returns_error() {
    let mut game = new_2p_game(42);

    // We're in ChooseInitialFlips phase — DrawFromDeck should be invalid
    let result = game.apply_action(PlayerAction::DrawFromDeck);
    assert!(result.is_err(), "DrawFromDeck during flips should error");
}

#[test]
fn continue_to_next_round() {
    let mut game = new_2p_game(55);
    let strategy = RandomStrategy;
    let mut max_actions = 10_000;

    // Play until RoundOver
    loop {
        let action_needed = game.get_action_needed();
        match action_needed {
            ActionNeeded::RoundOver { round_number, .. } => {
                assert_eq!(round_number, 0, "first round should be round 0");
                break;
            }
            ActionNeeded::GameOver { .. } => {
                // Game ended in one round — still a valid outcome, just skip
                return;
            }
            _ => {}
        }

        let action = game.get_bot_action(&strategy).unwrap();
        game.apply_action(action).unwrap();

        max_actions -= 1;
        assert!(max_actions > 0, "Did not reach RoundOver in time");
    }

    // Continue to the next round
    let result = game
        .apply_action(PlayerAction::ContinueToNextRound)
        .unwrap();

    match result {
        ActionNeeded::ChooseInitialFlips { player, count } => {
            assert!(player < 2);
            assert_eq!(count, 2);
        }
        other => panic!(
            "Expected ChooseInitialFlips after ContinueToNextRound, got {:?}",
            other
        ),
    }
}
