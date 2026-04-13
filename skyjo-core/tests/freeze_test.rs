use skyjo_core::{Game, GreedyStrategy, StandardRules, Strategy, Rules};

/// Regression test: 100 games with 4 Greedy players should all complete.
/// Previously, certain seeds caused infinite loops due to SmallRng using
/// different algorithms on 32-bit (WASM) vs 64-bit (native) targets.
#[test]
fn four_greedy_completes_100_games() {
    for seed in 0u64..100 {
        let rules: Box<dyn Rules> = Box::new(StandardRules);
        let strategies: Vec<Box<dyn Strategy>> = vec![
            Box::new(GreedyStrategy),
            Box::new(GreedyStrategy),
            Box::new(GreedyStrategy),
            Box::new(GreedyStrategy),
        ];
        let game = Game::new(rules, strategies, seed).unwrap();
        let history = game.play().unwrap();
        // Sanity check: games shouldn't have an absurd number of rounds
        assert!(
            history.rounds.len() <= 50,
            "seed {} took {} rounds",
            seed,
            history.rounds.len()
        );
    }
}
