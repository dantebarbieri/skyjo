use skyjo_core::*;

#[test]
fn game_history_round_trips_through_json() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> =
        vec![Box::new(RandomStrategy), Box::new(RandomStrategy)];
    let game = Game::new(rules, strategies, 42).unwrap();
    let history = game.play().unwrap();

    let json = serde_json::to_string(&history).unwrap();
    let deserialized: GameHistory = serde_json::from_str(&json).unwrap();

    assert_eq!(history.seed, deserialized.seed);
    assert_eq!(history.num_players, deserialized.num_players);
    assert_eq!(history.strategy_names, deserialized.strategy_names);
    assert_eq!(history.final_scores, deserialized.final_scores);
    assert_eq!(history.winners, deserialized.winners);
    assert_eq!(history.rounds.len(), deserialized.rounds.len());
}

#[test]
fn round_history_round_trips() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> =
        vec![Box::new(RandomStrategy), Box::new(RandomStrategy)];
    let game = Game::new(rules, strategies, 77).unwrap();
    let history = game.play().unwrap();

    let round = &history.rounds[0];
    let json = serde_json::to_string(round).unwrap();
    let deserialized: skyjo_core::history::RoundHistory = serde_json::from_str(&json).unwrap();

    assert_eq!(round.round_number, deserialized.round_number);
    assert_eq!(round.starting_player, deserialized.starting_player);
    assert_eq!(round.dealt_hands, deserialized.dealt_hands);
    assert_eq!(round.setup_flips, deserialized.setup_flips);
    assert_eq!(round.turns.len(), deserialized.turns.len());
    assert_eq!(round.round_scores, deserialized.round_scores);
    assert_eq!(round.cumulative_scores, deserialized.cumulative_scores);
    assert_eq!(round.going_out_player, deserialized.going_out_player);
}

#[test]
fn interactive_game_state_round_trips() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let names = vec!["Alice".to_string(), "Bob".to_string()];
    let game = InteractiveGame::new(rules, 2, names, 42).unwrap();
    let state = game.get_full_state();

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: InteractiveGameState = serde_json::from_str(&json).unwrap();

    assert_eq!(state.num_players, deserialized.num_players);
    assert_eq!(state.player_names, deserialized.player_names);
    assert_eq!(state.num_rows, deserialized.num_rows);
    assert_eq!(state.num_cols, deserialized.num_cols);
    assert_eq!(state.round_number, deserialized.round_number);
    assert_eq!(state.current_player, deserialized.current_player);
    assert_eq!(state.deck_remaining, deserialized.deck_remaining);
    assert_eq!(state.cumulative_scores, deserialized.cumulative_scores);
}

#[test]
fn column_clear_event_round_trips() {
    let event = skyjo_core::history::ColumnClearEvent {
        player_index: 1,
        column: 2,
        card_value: 7,
        displaced_card: Some(3),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: skyjo_core::history::ColumnClearEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(event.player_index, deserialized.player_index);
    assert_eq!(event.column, deserialized.column);
    assert_eq!(event.card_value, deserialized.card_value);
    assert_eq!(event.displaced_card, deserialized.displaced_card);
}
