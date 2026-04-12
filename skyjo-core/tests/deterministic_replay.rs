use skyjo_core::*;

#[test]
fn same_seed_produces_identical_history() {
    let seed = 42u64;

    let make_game = || {
        let rules: Box<dyn Rules> = Box::new(StandardRules);
        let strategies: Vec<Box<dyn Strategy>> = vec![
            Box::new(RandomStrategy),
            Box::new(RandomStrategy),
            Box::new(RandomStrategy),
            Box::new(RandomStrategy),
        ];
        Game::new(rules, strategies, seed).unwrap()
    };

    let history1 = make_game().play().unwrap();
    let history2 = make_game().play().unwrap();

    // Verify identical results
    assert_eq!(history1.rounds.len(), history2.rounds.len());
    assert_eq!(history1.final_scores, history2.final_scores);
    assert_eq!(history1.winners, history2.winners);

    for (r1, r2) in history1.rounds.iter().zip(history2.rounds.iter()) {
        assert_eq!(r1.initial_deck_order, r2.initial_deck_order);
        assert_eq!(r1.dealt_hands, r2.dealt_hands);
        assert_eq!(r1.setup_flips, r2.setup_flips);
        assert_eq!(r1.starting_player, r2.starting_player);
        assert_eq!(r1.turns.len(), r2.turns.len());
        assert_eq!(r1.going_out_player, r2.going_out_player);
        assert_eq!(r1.round_scores, r2.round_scores);
        assert_eq!(r1.cumulative_scores, r2.cumulative_scores);
    }
}

#[test]
fn different_seeds_produce_different_histories() {
    let history1 = {
        let rules: Box<dyn Rules> = Box::new(StandardRules);
        let strategies: Vec<Box<dyn Strategy>> =
            vec![Box::new(RandomStrategy), Box::new(RandomStrategy)];
        Game::new(rules, strategies, 1).unwrap().play().unwrap()
    };
    let history2 = {
        let rules: Box<dyn Rules> = Box::new(StandardRules);
        let strategies: Vec<Box<dyn Strategy>> =
            vec![Box::new(RandomStrategy), Box::new(RandomStrategy)];
        Game::new(rules, strategies, 2).unwrap().play().unwrap()
    };

    // Very unlikely to be identical with different seeds
    assert_ne!(history1.final_scores, history2.final_scores);
}
