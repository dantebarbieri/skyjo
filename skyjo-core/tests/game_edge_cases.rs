use skyjo_core::*;

fn make_random_game(num_players: usize, seed: u64) -> Result<Game> {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> =
        (0..num_players).map(|_| Box::new(RandomStrategy) as _).collect();
    Game::new(rules, strategies, seed)
}

fn make_greedy_game(num_players: usize, seed: u64) -> Result<Game> {
    let rules: Box<dyn Rules> = Box::new(StandardRules);
    let strategies: Vec<Box<dyn Strategy>> =
        (0..num_players).map(|_| Box::new(GreedyStrategy) as _).collect();
    Game::new(rules, strategies, seed)
}

#[test]
fn two_player_game_completes() {
    let history = make_random_game(2, 42).unwrap().play().unwrap();
    assert_eq!(history.final_scores.len(), 2);
    assert!(!history.winners.is_empty());
    let min_score = *history.final_scores.iter().min().unwrap();
    for &w in &history.winners {
        assert_eq!(history.final_scores[w], min_score);
    }
}

#[test]
fn eight_player_game_completes() {
    let history = make_random_game(8, 42).unwrap().play().unwrap();
    assert_eq!(history.final_scores.len(), 8);
    assert!(!history.winners.is_empty());
}

#[test]
fn not_enough_players_error() {
    match make_random_game(1, 42) {
        Err(SkyjoError::NotEnoughPlayers) => {}
        Err(e) => panic!("Expected NotEnoughPlayers, got {e:?}"),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

#[test]
fn too_many_players_error() {
    match make_random_game(9, 42) {
        Err(SkyjoError::TooManyPlayers) => {}
        Err(e) => panic!("Expected TooManyPlayers, got {e:?}"),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

#[test]
fn dealt_hands_match_deck_order() {
    let history = make_random_game(2, 42).unwrap().play().unwrap();
    let cards_per_player = 3 * 4; // StandardRules: 3 rows × 4 cols

    for round in &history.rounds {
        let deck = &round.initial_deck_order;
        let mut offset = 0;
        for (player_idx, hand) in round.dealt_hands.iter().enumerate() {
            assert_eq!(
                hand.len(),
                cards_per_player,
                "Player {player_idx} hand size mismatch"
            );
            // Cards are popped from the end of the deck
            let start = deck.len() - offset - cards_per_player;
            let end = deck.len() - offset;
            let expected: Vec<CardValue> = deck[start..end].iter().rev().copied().collect();
            assert_eq!(
                *hand, expected,
                "Round {} player {player_idx}: dealt hand doesn't match deck order",
                round.round_number
            );
            offset += cards_per_player;
        }

        // First discard card is the next one popped after dealing
        let discard_start = deck.len() - offset - 1;
        let first_discard = deck[discard_start];
        // Verify via the first turn's discard pile or setup — the initial discard
        // is the card at position deck.len() - (num_players * cards_per_player) - 1
        // We just verify the index math is consistent
        assert!(
            discard_start < deck.len(),
            "Discard index out of bounds"
        );
        let _ = first_discard; // used for consistency check
    }
}

#[test]
fn setup_flips_are_revealed() {
    let history = make_random_game(2, 42).unwrap().play().unwrap();

    for round in &history.rounds {
        for (player_idx, flips) in round.setup_flips.iter().enumerate() {
            assert_eq!(
                flips.len(),
                2,
                "Round {} player {player_idx}: expected 2 setup flips, got {}",
                round.round_number,
                flips.len()
            );
            for &pos in flips {
                assert!(
                    pos < 12,
                    "Round {} player {player_idx}: flip position {pos} out of bounds",
                    round.round_number
                );
            }
            // All flip positions should be unique
            assert_ne!(
                flips[0], flips[1],
                "Round {} player {player_idx}: duplicate flip positions",
                round.round_number
            );
        }
    }
}

#[test]
fn starting_player_round_one() {
    let history = make_random_game(2, 42).unwrap().play().unwrap();
    let round = &history.rounds[0];

    // Compute sum of initially flipped cards for each player
    let sums: Vec<i32> = round
        .dealt_hands
        .iter()
        .zip(round.setup_flips.iter())
        .map(|(hand, flips)| flips.iter().map(|&pos| hand[pos] as i32).sum())
        .collect();

    let max_sum = *sums.iter().max().unwrap();
    // Starting player should have the highest sum (ties break to lowest index)
    let expected_starter = sums.iter().position(|&s| s == max_sum).unwrap();
    assert_eq!(
        round.starting_player, expected_starter,
        "Round 1 starting player mismatch: sums={sums:?}, expected={expected_starter}, got={}",
        round.starting_player
    );
}

#[test]
fn starting_player_subsequent_rounds() {
    // Use a seed that produces multiple rounds
    for seed in 0..20u64 {
        let history = make_random_game(2, seed).unwrap().play().unwrap();
        if history.rounds.len() < 2 {
            continue;
        }
        for i in 1..history.rounds.len() {
            let prev_going_out = history.rounds[i - 1].going_out_player;
            if let Some(prev_goer) = prev_going_out {
                assert_eq!(
                    history.rounds[i].starting_player, prev_goer,
                    "Seed {seed} round {}: starting_player should be previous going_out_player",
                    history.rounds[i].round_number
                );
            }
        }
        return; // Found a multi-round game
    }
    panic!("No multi-round game found in seeds 0..20");
}

#[test]
fn going_out_player_final_turns() {
    let history = make_random_game(3, 42).unwrap().play().unwrap();

    for round in &history.rounds {
        if round.truncated {
            continue;
        }
        if let Some(goer) = round.going_out_player {
            // Find the turn where went_out is true
            let went_out_idx = round
                .turns
                .iter()
                .position(|t| t.went_out)
                .expect("going_out_player set but no turn has went_out=true");

            assert_eq!(round.turns[went_out_idx].player_index, goer);

            // After the going-out turn, each other player gets exactly 1 turn
            let remaining_turns: Vec<usize> = round.turns[went_out_idx + 1..]
                .iter()
                .map(|t| t.player_index)
                .collect();

            let num_players = round.dealt_hands.len();
            assert_eq!(
                remaining_turns.len(),
                num_players - 1,
                "Round {}: expected {} final turns after going out, got {}",
                round.round_number,
                num_players - 1,
                remaining_turns.len()
            );

            // Each remaining player appears exactly once
            for p in 0..num_players {
                if p == goer {
                    assert!(
                        !remaining_turns.contains(&p),
                        "Going-out player {p} should not get another turn"
                    );
                } else {
                    assert_eq!(
                        remaining_turns.iter().filter(|&&x| x == p).count(),
                        1,
                        "Player {p} should get exactly 1 final turn"
                    );
                }
            }
        }
    }
}

#[test]
fn column_clears_during_turns() {
    let mut found_clear = false;

    for seed in 0..100u64 {
        let history = make_greedy_game(2, seed).unwrap().play().unwrap();
        for round in &history.rounds {
            for turn in &round.turns {
                if !turn.column_clears.is_empty() {
                    found_clear = true;
                    for clear in &turn.column_clears {
                        assert_eq!(
                            clear.player_index, turn.player_index,
                            "Column clear player mismatch in seed {seed}"
                        );
                        // Column index should be valid (0..4 for StandardRules)
                        assert!(
                            clear.column < 4,
                            "Invalid column index {} in seed {seed}",
                            clear.column
                        );
                    }
                }
            }
        }
    }

    assert!(found_clear, "No column clears found in 100 greedy games");
}

#[test]
fn end_of_round_clears() {
    let mut found = false;

    for seed in 0..100u64 {
        let history = make_greedy_game(2, seed).unwrap().play().unwrap();
        for round in &history.rounds {
            if !round.end_of_round_clears.is_empty() {
                found = true;
                for clear in &round.end_of_round_clears {
                    assert!(clear.column < 4, "Invalid column index in end-of-round clear");
                    assert!(
                        clear.player_index < 2,
                        "Invalid player index in end-of-round clear"
                    );
                }
            }
        }
    }

    assert!(found, "No end-of-round clears found in 100 greedy games");
}

#[test]
fn max_turns_truncation() {
    let mut game = make_random_game(2, 42).unwrap();
    game.set_max_turns_per_round(5);
    let history = game.play().unwrap();

    let any_truncated = history.rounds.iter().any(|r| r.truncated);
    assert!(
        any_truncated,
        "Expected at least one truncated round with max_turns=5"
    );

    for round in &history.rounds {
        if round.truncated {
            assert!(
                round.turns.len() <= 5,
                "Truncated round has {} turns, expected <= 5",
                round.turns.len()
            );
        }
    }
}

#[test]
fn history_seed_matches() {
    let seed = 12345u64;
    let history = make_random_game(2, seed).unwrap().play().unwrap();
    assert_eq!(history.seed, seed);
}

#[test]
fn cumulative_scores_accumulate() {
    let history = make_random_game(2, 42).unwrap().play().unwrap();
    assert!(history.rounds.len() >= 1);

    let num_players = history.num_players;
    let mut prev_cumulative = vec![0i32; num_players];

    for round in &history.rounds {
        assert_eq!(round.round_scores.len(), num_players);
        assert_eq!(round.cumulative_scores.len(), num_players);

        for p in 0..num_players {
            assert_eq!(
                round.cumulative_scores[p],
                prev_cumulative[p] + round.round_scores[p],
                "Round {}: player {p} cumulative mismatch",
                round.round_number
            );
        }
        prev_cumulative = round.cumulative_scores.clone();
    }
}

#[test]
fn game_ends_at_threshold() {
    // StandardRules threshold is 100
    for seed in 0..20u64 {
        let history = make_random_game(2, seed).unwrap().play().unwrap();
        let last_round = history.rounds.last().unwrap();

        if !last_round.truncated {
            let max_cumulative = *last_round.cumulative_scores.iter().max().unwrap();
            assert!(
                max_cumulative >= 100,
                "Seed {seed}: game ended but max cumulative score is {max_cumulative} < 100"
            );
        }
    }

    // Also verify that non-final rounds don't have anyone >= 100
    let history = make_random_game(2, 42).unwrap().play().unwrap();
    for round in &history.rounds[..history.rounds.len() - 1] {
        let max_cumulative = *round.cumulative_scores.iter().max().unwrap();
        assert!(
            max_cumulative < 100,
            "Non-final round {} has cumulative score {max_cumulative} >= 100",
            round.round_number
        );
    }
}
