use skyjo_core::*;

#[test]
fn mixed_strategies_game_runs() {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> = vec![
        Box::new(RandomStrategy),
        Box::new(GreedyStrategy),
        Box::new(RandomStrategy),
        Box::new(GreedyStrategy),
    ];
    let game = Game::new(rules, strategies, 42).unwrap();
    let history = game.play().unwrap();

    assert_eq!(history.num_players, 4);
    assert_eq!(
        history.strategy_names,
        vec!["Random", "Greedy", "Random", "Greedy"]
    );
    assert!(!history.rounds.is_empty());
    assert!(!history.winners.is_empty());
}

#[test]
fn greedy_beats_random_over_many_games() {
    let sim = Simulator::new(
        SimulatorConfig {
            num_games: 100,
            base_seed: 1000,
        },
        Box::new(|| Box::new(StandardRules)),
        vec![
            Box::new(|| -> Box<dyn Strategy> { Box::new(GreedyStrategy) }),
            Box::new(|| -> Box<dyn Strategy> { Box::new(GreedyStrategy) }),
            Box::new(|| -> Box<dyn Strategy> { Box::new(RandomStrategy) }),
            Box::new(|| -> Box<dyn Strategy> { Box::new(RandomStrategy) }),
        ],
    );

    let stats = sim.run_stats_only();

    // Greedy players (0,1) should generally win more than random players (2,3)
    let greedy_wins: usize = stats.wins_per_player[0] + stats.wins_per_player[1];
    let random_wins: usize = stats.wins_per_player[2] + stats.wins_per_player[3];

    // This should be very likely over 100 games — greedy is much better than random
    assert!(
        greedy_wins > random_wins,
        "Expected greedy ({greedy_wins}) to win more than random ({random_wins})"
    );
}
