use skyjo_core::*;

/// Run a game with 8 players (consuming 8×12 = 96 cards per round from a 150-card deck),
/// which makes it very likely the deck will be exhausted and require a reshuffle.
/// The test verifies the game completes without panicking.
#[test]
fn eight_player_game_survives_deck_exhaustion() {
    // Try multiple seeds to increase likelihood of hitting reshuffle
    for seed in 0..10u64 {
        let strategies: Vec<Box<dyn Strategy>> =
            (0..8).map(|_| Box::new(RandomStrategy) as _).collect();
        let game = Game::new(Box::new(StandardRules), strategies, seed).unwrap();
        let history = game.play().unwrap();
        assert!(!history.rounds.is_empty());
        assert!(!history.winners.is_empty());
        assert_eq!(history.final_scores.len(), 8);
    }
}

/// Use greedy strategies with 8 players for another reshuffle scenario.
/// Greedy games tend to last longer, increasing deck pressure.
#[test]
fn eight_player_greedy_survives_reshuffle() {
    for seed in 0..5u64 {
        let strategies: Vec<Box<dyn Strategy>> =
            (0..8).map(|_| Box::new(GreedyStrategy) as _).collect();
        let game = Game::new(Box::new(StandardRules), strategies, seed).unwrap();
        let history = game.play().unwrap();
        assert!(!history.rounds.is_empty());
        assert!(!history.winners.is_empty());
    }
}

/// Verify that a game with many rounds (by using a seed that produces low scores)
/// can handle multiple reshuffles across rounds.
#[test]
fn multi_round_game_handles_reshuffles() {
    let strategies: Vec<Box<dyn Strategy>> =
        (0..6).map(|_| Box::new(RandomStrategy) as _).collect();
    let game = Game::new(Box::new(StandardRules), strategies, 12345).unwrap();
    let history = game.play().unwrap();
    // With 6 players, each round uses 6×12 = 72 cards just for dealing,
    // plus 1 for initial discard = 73 out of 150.
    // Within a round, draws further deplete the deck.
    assert!(history.rounds.len() >= 1);
    assert_eq!(history.final_scores.len(), 6);
}
