use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

use skyjo_core::{
    AggregateStats, ClearerStrategy, DefensiveStrategy, Game, GameHistory, GameStats,
    GreedyStrategy, InteractiveGame, PlayerAction, RandomStrategy, Rules, Simulator,
    SimulatorConfig, StandardRules, StatisticianStrategy, Strategy,
};

#[derive(Deserialize)]
struct WasmSimConfig {
    num_games: usize,
    seed: u64,
    strategies: Vec<String>,
    rules: Option<String>,
}

#[derive(Serialize)]
struct SimWithHistories {
    stats: AggregateStats,
    histories: Vec<GameHistory>,
}

#[derive(Deserialize)]
struct SingleGameConfig {
    seed: u64,
    strategies: Vec<String>,
    rules: Option<String>,
    max_turns_per_round: Option<usize>,
}

#[derive(Serialize)]
struct SingleGameResult {
    stats: GameStats,
    history: GameHistory,
}

fn make_strategy_factory(name: &str) -> Result<Box<dyn Fn() -> Box<dyn Strategy>>, String> {
    match name {
        "Random" => Ok(Box::new(|| Box::new(RandomStrategy))),
        "Greedy" => Ok(Box::new(|| Box::new(GreedyStrategy))),
        "Defensive" => Ok(Box::new(|| Box::new(DefensiveStrategy))),
        "Clearer" => Ok(Box::new(|| Box::new(ClearerStrategy))),
        "Statistician" => Ok(Box::new(|| Box::new(StatisticianStrategy))),
        _ => Err(format!("Unknown strategy: {name}")),
    }
}

fn make_rules_factory(name: &str) -> Result<Box<dyn Fn() -> Box<dyn Rules>>, String> {
    match name {
        "Standard" | "" => Ok(Box::new(|| Box::new(StandardRules))),
        _ => Err(format!("Unknown rules: {name}")),
    }
}

fn make_strategies(names: &[String]) -> Result<Vec<Box<dyn Strategy>>, String> {
    names.iter().map(|name| make_strategy(name)).collect()
}

fn make_strategy(name: &str) -> Result<Box<dyn Strategy>, String> {
    match name {
        "Random" => Ok(Box::new(RandomStrategy)),
        "Greedy" => Ok(Box::new(GreedyStrategy)),
        "Defensive" => Ok(Box::new(DefensiveStrategy)),
        "Clearer" => Ok(Box::new(ClearerStrategy)),
        "Statistician" => Ok(Box::new(StatisticianStrategy)),
        _ => Err(format!("Unknown strategy: {name}")),
    }
}

fn make_rules(name: &str) -> Result<Box<dyn Rules>, String> {
    match name {
        "Standard" | "" => Ok(Box::new(StandardRules)),
        _ => Err(format!("Unknown rules: {name}")),
    }
}

fn run_simulate(config_json: &str) -> Result<AggregateStats, String> {
    let config: WasmSimConfig =
        serde_json::from_str(config_json).map_err(|e| format!("Invalid config JSON: {e}"))?;

    let rules_name = config.rules.as_deref().unwrap_or("Standard");
    let rules_factory = make_rules_factory(rules_name)?;

    let strategy_factories: Vec<Box<dyn Fn() -> Box<dyn Strategy>>> = config
        .strategies
        .iter()
        .map(|name| make_strategy_factory(name))
        .collect::<Result<_, _>>()?;

    let sim_config = SimulatorConfig {
        num_games: config.num_games,
        base_seed: config.seed,
    };

    let simulator = Simulator::new(sim_config, rules_factory, strategy_factories);
    Ok(simulator.run_stats_only())
}

fn run_simulate_with_histories(config_json: &str) -> Result<SimWithHistories, String> {
    let config: WasmSimConfig =
        serde_json::from_str(config_json).map_err(|e| format!("Invalid config JSON: {e}"))?;

    let rules_name = config.rules.as_deref().unwrap_or("Standard");
    let rules_factory = make_rules_factory(rules_name)?;

    let strategy_factories: Vec<Box<dyn Fn() -> Box<dyn Strategy>>> = config
        .strategies
        .iter()
        .map(|name| make_strategy_factory(name))
        .collect::<Result<_, _>>()?;

    let sim_config = SimulatorConfig {
        num_games: config.num_games,
        base_seed: config.seed,
    };

    let simulator = Simulator::new(sim_config, rules_factory, strategy_factories);
    let (histories, stats) = simulator.run();
    Ok(SimWithHistories { stats, histories })
}

fn create_game(config: &SingleGameConfig) -> Result<Game, String> {
    let rules_name = config.rules.as_deref().unwrap_or("Standard");
    let rules = make_rules(rules_name)?;
    let strategies = make_strategies(&config.strategies)?;

    let mut game = Game::new(rules, strategies, config.seed)
        .map_err(|e| format!("Game creation failed: {e:?}"))?;
    if let Some(limit) = config.max_turns_per_round {
        game.set_max_turns_per_round(limit);
    }
    Ok(game)
}

fn run_single_game(config_json: &str) -> Result<GameStats, String> {
    let config: SingleGameConfig =
        serde_json::from_str(config_json).map_err(|e| format!("Invalid config JSON: {e}"))?;

    let game = create_game(&config)?;
    let history = game
        .play()
        .map_err(|e| format!("Game play failed: {e:?}"))?;

    Ok(GameStats {
        winners: history.winners,
        final_scores: history.final_scores,
        num_rounds: history.rounds.len(),
        total_turns: history.rounds.iter().map(|r| r.turns.len()).sum(),
    })
}

fn run_single_game_with_history(config_json: &str) -> Result<SingleGameResult, String> {
    let config: SingleGameConfig =
        serde_json::from_str(config_json).map_err(|e| format!("Invalid config JSON: {e}"))?;

    let game = create_game(&config)?;
    let history = game
        .play()
        .map_err(|e| format!("Game play failed: {e:?}"))?;

    let stats = GameStats {
        winners: history.winners.clone(),
        final_scores: history.final_scores.clone(),
        num_rounds: history.rounds.len(),
        total_turns: history.rounds.iter().map(|r| r.turns.len()).sum(),
    };

    Ok(SingleGameResult { stats, history })
}

fn to_json_or_error<F: FnOnce() -> Result<T, String>, T: serde::Serialize>(f: F) -> String {
    match f() {
        Ok(val) => serde_json::to_string(&val).unwrap(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

#[wasm_bindgen]
pub fn simulate(config_json: &str) -> String {
    to_json_or_error(|| run_simulate(config_json))
}

#[wasm_bindgen]
pub fn simulate_with_histories(config_json: &str) -> String {
    to_json_or_error(|| run_simulate_with_histories(config_json))
}

/// Run a single game, returning GameStats (no history).
/// Config: { seed, strategies: string[], rules?: string }
#[wasm_bindgen]
pub fn simulate_one(config_json: &str) -> String {
    to_json_or_error(|| run_single_game(config_json))
}

/// Run a single game, returning both GameStats and full GameHistory.
/// Config: { seed, strategies: string[], rules?: string }
#[wasm_bindgen]
pub fn simulate_one_with_history(config_json: &str) -> String {
    to_json_or_error(|| run_single_game_with_history(config_json))
}

#[wasm_bindgen]
pub fn get_available_strategies() -> String {
    serde_json::to_string(&["Random", "Greedy", "Defensive", "Clearer", "Statistician"]).unwrap()
}

#[wasm_bindgen]
pub fn get_available_rules() -> String {
    serde_json::to_string(&["Standard"]).unwrap()
}

#[derive(Serialize)]
struct RulesInfo {
    name: String,
    grid: String,
    initial_flips: usize,
    end_threshold: i32,
    discard_piles: String,
    column_clear: usize,
    going_out_penalty: String,
    reshuffle_on_empty: bool,
    deck_size: usize,
}

#[wasm_bindgen]
pub fn get_rules_info(rules_name: &str) -> String {
    let rules = match make_rules(rules_name) {
        Ok(r) => r,
        Err(e) => return serde_json::json!({ "error": e }).to_string(),
    };

    let penalty_desc = {
        // Probe the penalty function to describe its behavior
        let doubled = rules.apply_going_out_penalty(10, 5, false);
        let solo_lowest = rules.apply_going_out_penalty(10, 15, true);
        let negative = rules.apply_going_out_penalty(-5, -10, false);
        if doubled == 20 && solo_lowest == 10 && negative == -5 {
            "Score doubled if not solo lowest (non-positive exempt)".to_string()
        } else {
            format!(
                "Custom (10 not lowest={doubled}, 10 solo lowest={solo_lowest}, -5 not lowest={negative})"
            )
        }
    };

    let info = RulesInfo {
        name: rules.name().to_string(),
        grid: format!("{} x {}", rules.num_rows(), rules.num_cols()),
        initial_flips: rules.initial_flips(),
        end_threshold: rules.end_threshold(),
        discard_piles: if rules.discard_pile_count(4) == 1 {
            "1 (shared)".to_string()
        } else {
            format!("{} (per-player)", rules.discard_pile_count(4))
        },
        column_clear: rules.column_clear_threshold(),
        going_out_penalty: penalty_desc,
        reshuffle_on_empty: rules.reshuffle_on_empty_deck(),
        deck_size: rules.build_deck().len(),
    };

    serde_json::to_string(&info).unwrap()
}

// --- Interactive Game API ---

thread_local! {
    static INTERACTIVE_GAMES: RefCell<HashMap<u32, InteractiveGame>> = RefCell::new(HashMap::new());
    static NEXT_GAME_ID: RefCell<u32> = const { RefCell::new(1) };
}

#[derive(Deserialize)]
struct InteractiveGameConfig {
    num_players: usize,
    player_names: Vec<String>,
    rules: Option<String>,
    seed: u64,
}

#[derive(Serialize)]
struct InteractiveGameCreated {
    game_id: u32,
    state: skyjo_core::InteractiveGameState,
}

fn create_interactive_game_inner(config_json: &str) -> Result<InteractiveGameCreated, String> {
    let config: InteractiveGameConfig =
        serde_json::from_str(config_json).map_err(|e| format!("Invalid config JSON: {e}"))?;

    let rules_name = config.rules.as_deref().unwrap_or("Standard");
    let rules = make_rules(rules_name)?;

    let game = InteractiveGame::new(rules, config.num_players, config.player_names, config.seed)
        .map_err(|e| format!("Game creation failed: {e}"))?;

    let state = game.get_full_state();

    let game_id = NEXT_GAME_ID.with(|id| {
        let current = *id.borrow();
        *id.borrow_mut() = current + 1;
        current
    });

    INTERACTIVE_GAMES.with(|games| {
        games.borrow_mut().insert(game_id, game);
    });

    Ok(InteractiveGameCreated { game_id, state })
}

#[wasm_bindgen]
pub fn create_interactive_game(config_json: &str) -> String {
    to_json_or_error(|| create_interactive_game_inner(config_json))
}

#[derive(Serialize)]
struct GameStateResponse {
    state: skyjo_core::InteractiveGameState,
}

#[wasm_bindgen]
pub fn get_game_state(game_id: u32, player_index: usize) -> String {
    to_json_or_error(|| {
        INTERACTIVE_GAMES.with(|games| {
            let games = games.borrow();
            let game = games.get(&game_id).ok_or("Game not found")?;
            let state = if player_index == usize::MAX {
                game.get_full_state()
            } else {
                game.get_player_state(player_index)
            };
            Ok(GameStateResponse { state })
        })
    })
}

#[derive(Serialize)]
struct ActionResponse {
    state: skyjo_core::InteractiveGameState,
}

#[wasm_bindgen]
pub fn apply_action(game_id: u32, action_json: &str) -> String {
    to_json_or_error(|| {
        let action: PlayerAction =
            serde_json::from_str(action_json).map_err(|e| format!("Invalid action JSON: {e}"))?;

        INTERACTIVE_GAMES.with(|games| {
            let mut games = games.borrow_mut();
            let game = games.get_mut(&game_id).ok_or("Game not found")?;
            game.apply_action(action)
                .map_err(|e| format!("Action failed: {e}"))?;
            let state = game.get_full_state();
            Ok(ActionResponse { state })
        })
    })
}

#[derive(Serialize)]
struct BotActionResponse {
    action: PlayerAction,
    state: skyjo_core::InteractiveGameState,
}

/// Compute and apply the action a bot strategy would take, returning both the action and new state.
#[wasm_bindgen]
pub fn apply_bot_action(game_id: u32, strategy_name: &str) -> String {
    to_json_or_error(|| {
        let strategy = make_strategy(strategy_name)?;

        INTERACTIVE_GAMES.with(|games| {
            let mut games = games.borrow_mut();
            let game = games.get_mut(&game_id).ok_or("Game not found")?;
            let action = game
                .get_bot_action(&*strategy)
                .map_err(|e| format!("Bot action failed: {e}"))?;
            game.apply_action(action.clone())
                .map_err(|e| format!("Applying bot action failed: {e}"))?;
            let state = game.get_full_state();
            Ok(BotActionResponse { action, state })
        })
    })
}

#[wasm_bindgen]
pub fn destroy_interactive_game(game_id: u32) -> String {
    INTERACTIVE_GAMES.with(|games| {
        let removed = games.borrow_mut().remove(&game_id).is_some();
        serde_json::json!({ "removed": removed }).to_string()
    })
}
