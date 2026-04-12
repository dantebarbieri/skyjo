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
