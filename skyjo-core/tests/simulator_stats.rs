use skyjo_core::*;

fn run_simulation(num_games: usize, seed: u64) -> AggregateStats {
    let config = SimulatorConfig {
        num_games,
        base_seed: seed,
    };
    let simulator = Simulator::new(
        config,
        Box::new(|| Box::new(StandardRules) as Box<dyn Rules>),
        vec![
            Box::new(|| Box::new(RandomStrategy) as Box<dyn Strategy>),
            Box::new(|| Box::new(RandomStrategy) as Box<dyn Strategy>),
        ],
    );
    simulator.run_stats_only()
}

#[test]
fn wins_sum_correctly() {
    let stats = run_simulation(50, 123);
    let total_wins: usize = stats.wins_per_player.iter().sum();
    // Each game has at least one winner; ties count each winner.
    assert!(
        total_wins >= stats.num_games,
        "Total wins ({total_wins}) should be >= num_games ({})",
        stats.num_games
    );
}

#[test]
fn win_rates_bounded() {
    let stats = run_simulation(50, 456);
    for (i, &rate) in stats.win_rate_per_player.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&rate),
            "Player {i} win rate {rate} out of [0, 1]"
        );
    }
}

#[test]
fn score_stats_consistent() {
    let stats = run_simulation(50, 789);
    for i in 0..stats.num_players {
        let min = stats.min_score_per_player[i] as f64;
        let avg = stats.avg_score_per_player[i];
        let max = stats.max_score_per_player[i] as f64;
        assert!(
            min <= avg && avg <= max,
            "Player {i}: min ({min}) <= avg ({avg}) <= max ({max}) violated"
        );
    }
}

#[test]
fn determinism() {
    let stats1 = run_simulation(30, 42);
    let stats2 = run_simulation(30, 42);

    assert_eq!(stats1.num_games, stats2.num_games);
    assert_eq!(stats1.wins_per_player, stats2.wins_per_player);
    assert_eq!(stats1.win_rate_per_player, stats2.win_rate_per_player);
    assert_eq!(stats1.avg_score_per_player, stats2.avg_score_per_player);
    assert_eq!(stats1.min_score_per_player, stats2.min_score_per_player);
    assert_eq!(stats1.max_score_per_player, stats2.max_score_per_player);
    assert_eq!(stats1.avg_rounds_per_game, stats2.avg_rounds_per_game);
    assert_eq!(stats1.avg_turns_per_game, stats2.avg_turns_per_game);
    assert_eq!(stats1.score_distributions, stats2.score_distributions);
}

#[test]
fn different_seeds_differ() {
    let stats1 = run_simulation(50, 1000);
    let stats2 = run_simulation(50, 2000);

    // With 50 games and different seeds, it is extremely unlikely that
    // both score distributions and win counts are identical.
    let differ = stats1.score_distributions != stats2.score_distributions
        || stats1.wins_per_player != stats2.wins_per_player;
    assert!(differ, "Two runs with different seeds should almost certainly differ");
}

#[test]
fn score_distributions_populated() {
    let stats = run_simulation(50, 555);
    assert_eq!(stats.score_distributions.len(), stats.num_players);
    for (i, dist) in stats.score_distributions.iter().enumerate() {
        assert_eq!(
            dist.len(),
            stats.num_games,
            "Player {i} score_distributions length should equal num_games"
        );
    }
}
