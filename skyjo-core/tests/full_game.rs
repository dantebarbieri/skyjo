use skyjo_core::*;

#[test]
fn game_terminates_with_random_strategy() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> = vec![
        Box::new(RandomStrategy),
        Box::new(RandomStrategy),
        Box::new(RandomStrategy),
        Box::new(RandomStrategy),
    ];
    let game = Game::new(rules, strategies, 123).unwrap();
    let history = game.play().unwrap();

    assert!(!history.rounds.is_empty());
    assert_eq!(history.final_scores.len(), 4);
    assert!(!history.winners.is_empty());

    // At least one player should have >= 100 cumulative at the end
    assert!(history.final_scores.iter().any(|&s| s >= 100));

    // Winners should have the minimum score
    let min_score = *history.final_scores.iter().min().unwrap();
    for &w in &history.winners {
        assert_eq!(history.final_scores[w], min_score);
    }
}

#[test]
fn cumulative_scores_match_sum_of_rounds() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> = vec![
        Box::new(RandomStrategy),
        Box::new(RandomStrategy),
        Box::new(RandomStrategy),
    ];
    let game = Game::new(rules, strategies, 456).unwrap();
    let history = game.play().unwrap();

    // Verify cumulative scores match sum of round scores
    let num_players = history.num_players;
    let mut expected_cumulative = vec![0i32; num_players];
    for round in &history.rounds {
        for (i, &score) in round.round_scores.iter().enumerate() {
            expected_cumulative[i] += score;
        }
        assert_eq!(round.cumulative_scores, expected_cumulative);
    }
    assert_eq!(history.final_scores, expected_cumulative);
}

#[test]
fn history_has_all_fields_populated() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> =
        vec![Box::new(RandomStrategy), Box::new(RandomStrategy)];
    let game = Game::new(rules, strategies, 789).unwrap();
    let history = game.play().unwrap();

    assert_eq!(history.seed, 789);
    assert_eq!(history.num_players, 2);
    assert_eq!(history.strategy_names, vec!["Random", "Random"]);
    assert_eq!(history.rules_name, "Standard");

    for (i, round) in history.rounds.iter().enumerate() {
        assert_eq!(round.round_number, i);
        assert_eq!(round.dealt_hands.len(), 2);
        assert_eq!(round.setup_flips.len(), 2);
        assert!(!round.turns.is_empty());
        assert_eq!(round.round_scores.len(), 2);
        assert_eq!(round.cumulative_scores.len(), 2);
    }
}

#[test]
fn two_player_game_works() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> =
        vec![Box::new(RandomStrategy), Box::new(RandomStrategy)];
    let game = Game::new(rules, strategies, 100).unwrap();
    let history = game.play().unwrap();
    assert_eq!(history.num_players, 2);
    assert!(!history.rounds.is_empty());
}

#[test]
fn eight_player_game_works() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> = (0..8).map(|_| -> Box<dyn Strategy> { Box::new(RandomStrategy) }).collect();
    let game = Game::new(rules, strategies, 200).unwrap();
    let history = game.play().unwrap();
    assert_eq!(history.num_players, 8);
    assert!(!history.rounds.is_empty());
}
