use skyjo_core::*;

use rand::rngs::StdRng;
use rand::SeedableRng;

fn make_view(board: Vec<VisibleSlot>, discard_top: Option<CardValue>) -> StrategyView {
    let mut discard_piles = vec![Vec::new()];
    if let Some(val) = discard_top {
        discard_piles[0].push(val);
    }
    StrategyView {
        my_index: 0,
        my_board: board,
        num_rows: 3,
        num_cols: 4,
        opponent_boards: vec![],
        opponent_indices: vec![],
        discard_piles,
        deck_remaining: 50,
        cumulative_scores: vec![0, 0],
        is_final_turn: false,
    }
}

// ── Greedy tests ────────────────────────────────────────────────────

#[test]
fn greedy_initial_flips_valid() {
    let strategy = GreedyStrategy;
    let board = vec![VisibleSlot::Hidden; 12];
    let view = make_view(board, None);
    let mut rng = StdRng::seed_from_u64(1);

    let flips = strategy.choose_initial_flips(&view, 2, &mut rng);
    assert_eq!(flips.len(), 2);
    assert_ne!(flips[0], flips[1]);
    for &pos in &flips {
        assert!(pos < 12);
        assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
    }
}

#[test]
fn greedy_takes_low_discard() {
    let strategy = GreedyStrategy;
    let mut board = vec![VisibleSlot::Hidden; 12];
    board[0] = VisibleSlot::Revealed(10);
    let view = make_view(board, Some(0));
    let mut rng = StdRng::seed_from_u64(1);

    let choice = strategy.choose_draw(&view, &mut rng);
    assert_eq!(choice, DrawChoice::DrawFromDiscard(0));
}

#[test]
fn greedy_ignores_high_discard() {
    let strategy = GreedyStrategy;
    let mut board = vec![VisibleSlot::Hidden; 12];
    board[0] = VisibleSlot::Revealed(3);
    let view = make_view(board, Some(8));
    let mut rng = StdRng::seed_from_u64(1);

    let choice = strategy.choose_draw(&view, &mut rng);
    assert_eq!(choice, DrawChoice::DrawFromDeck);
}

#[test]
fn greedy_keeps_low_deck_draw() {
    let strategy = GreedyStrategy;
    let mut board = vec![VisibleSlot::Hidden; 12];
    board[3] = VisibleSlot::Revealed(10);
    let view = make_view(board, None);
    let mut rng = StdRng::seed_from_u64(1);

    let action = strategy.choose_deck_draw_action(&view, -1, &mut rng);
    assert_eq!(action, DeckDrawAction::Keep(3));
}

#[test]
fn greedy_discards_high_deck_draw() {
    let strategy = GreedyStrategy;
    let mut board = vec![VisibleSlot::Hidden; 12];
    board[0] = VisibleSlot::Revealed(2);
    board[1] = VisibleSlot::Revealed(2);
    // Positions 2..12 are Hidden
    let view = make_view(board, None);
    let mut rng = StdRng::seed_from_u64(1);

    let action = strategy.choose_deck_draw_action(&view, 12, &mut rng);
    match action {
        DeckDrawAction::DiscardAndFlip(pos) => {
            assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
        }
        _ => panic!("Expected DiscardAndFlip for a high card draw"),
    }
}

#[test]
fn greedy_discard_placement_replaces_highest() {
    let strategy = GreedyStrategy;
    let mut board = vec![VisibleSlot::Hidden; 12];
    board[0] = VisibleSlot::Revealed(3);
    board[1] = VisibleSlot::Revealed(10);
    let view = make_view(board, Some(1));
    let mut rng = StdRng::seed_from_u64(1);

    let pos = strategy.choose_discard_draw_placement(&view, 1, &mut rng);
    assert_eq!(pos, 1, "Should replace the position with value 10");
}

// ── Random tests ────────────────────────────────────────────────────

#[test]
fn random_initial_flips_valid() {
    let strategy = RandomStrategy;
    let board = vec![VisibleSlot::Hidden; 12];
    let view = make_view(board, None);
    let mut rng = StdRng::seed_from_u64(42);

    let flips = strategy.choose_initial_flips(&view, 2, &mut rng);
    assert_eq!(flips.len(), 2);
    assert_ne!(flips[0], flips[1]);
    for &pos in &flips {
        assert!(pos < 12);
        assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
    }
}

#[test]
fn random_determinism() {
    let strategy = RandomStrategy;
    let board = vec![VisibleSlot::Hidden; 12];
    let view = make_view(board, Some(5));

    let mut rng1 = StdRng::seed_from_u64(99);
    let mut rng2 = StdRng::seed_from_u64(99);

    let flips1 = strategy.choose_initial_flips(&view, 2, &mut rng1);
    let draw1 = strategy.choose_draw(&view, &mut rng1);

    let flips2 = strategy.choose_initial_flips(&view, 2, &mut rng2);
    let draw2 = strategy.choose_draw(&view, &mut rng2);

    assert_eq!(flips1, flips2);
    assert_eq!(draw1, draw2);
}

#[test]
fn random_draw_returns_valid_choice() {
    let strategy = RandomStrategy;
    let board = vec![VisibleSlot::Hidden; 12];
    let view = make_view(board, Some(5));
    let mut rng = StdRng::seed_from_u64(7);

    for _ in 0..20 {
        let choice = strategy.choose_draw(&view, &mut rng);
        match choice {
            DrawChoice::DrawFromDeck => {}
            DrawChoice::DrawFromDiscard(pile) => {
                assert!(pile < view.discard_piles.len());
                assert!(!view.discard_piles[pile].is_empty());
            }
        }
    }
}

// ── Edge-case tests ─────────────────────────────────────────────────

#[test]
fn strategy_no_hidden_cards() {
    let board: Vec<VisibleSlot> = (0..12).map(|i| VisibleSlot::Revealed(i % 6)).collect();
    let view = make_view(board, Some(5));
    let mut rng = StdRng::seed_from_u64(1);

    // Greedy should not panic with no hidden cards
    let greedy = GreedyStrategy;
    let _ = greedy.choose_draw(&view, &mut rng);
    let action = greedy.choose_deck_draw_action(&view, 0, &mut rng);
    match action {
        DeckDrawAction::Keep(pos) => assert!(pos < 12),
        DeckDrawAction::DiscardAndFlip(_) => panic!("No hidden cards to flip"),
    }

    // Random should not panic either
    let random = RandomStrategy;
    let _ = random.choose_draw(&view, &mut rng);
    let action = random.choose_deck_draw_action(&view, 5, &mut rng);
    match action {
        DeckDrawAction::Keep(pos) => assert!(pos < 12),
        DeckDrawAction::DiscardAndFlip(_) => panic!("No hidden cards to flip"),
    }
}

#[test]
fn strategy_with_cleared_slots() {
    let mut board = vec![VisibleSlot::Hidden; 12];
    board[0] = VisibleSlot::Cleared;
    board[1] = VisibleSlot::Cleared;
    board[2] = VisibleSlot::Cleared;
    board[3] = VisibleSlot::Revealed(8);
    let view = make_view(board, Some(2));
    let mut rng = StdRng::seed_from_u64(1);

    // Greedy discard placement should never target a cleared slot
    let greedy = GreedyStrategy;
    let pos = greedy.choose_discard_draw_placement(&view, 2, &mut rng);
    assert!(!matches!(view.my_board[pos], VisibleSlot::Cleared));

    // Random discard placement should never target a cleared slot
    let random = RandomStrategy;
    for _ in 0..20 {
        let pos = random.choose_discard_draw_placement(&view, 2, &mut rng);
        assert!(!matches!(view.my_board[pos], VisibleSlot::Cleared));
    }
}

#[test]
fn strategy_custom_grid() {
    // 2 rows x 3 cols = 6 slots
    let board = vec![
        VisibleSlot::Hidden,
        VisibleSlot::Hidden,
        VisibleSlot::Revealed(5),
        VisibleSlot::Revealed(9),
        VisibleSlot::Hidden,
        VisibleSlot::Hidden,
    ];
    let mut discard_piles = vec![Vec::new()];
    discard_piles[0].push(1);
    let view = StrategyView {
        my_index: 0,
        my_board: board,
        num_rows: 2,
        num_cols: 3,
        opponent_boards: vec![],
        opponent_indices: vec![],
        discard_piles,
        deck_remaining: 40,
        cumulative_scores: vec![0, 0],
        is_final_turn: false,
    };
    let mut rng = StdRng::seed_from_u64(1);

    // Greedy should work on 2x3 grid
    let greedy = GreedyStrategy;
    let flips = greedy.choose_initial_flips(&view, 2, &mut rng);
    assert_eq!(flips.len(), 2);
    for &pos in &flips {
        assert!(pos < 6);
        assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
    }
    let draw = greedy.choose_draw(&view, &mut rng);
    // Discard is 1, highest revealed is 9 → should take discard
    assert_eq!(draw, DrawChoice::DrawFromDiscard(0));

    // Random should work on 2x3 grid
    let random = RandomStrategy;
    let flips = random.choose_initial_flips(&view, 2, &mut rng);
    assert_eq!(flips.len(), 2);
    for &pos in &flips {
        assert!(pos < 6);
        assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
    }
}
