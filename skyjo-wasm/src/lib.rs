use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

use skyjo_core::{
    AggregateStats, Game, GameHistory, GameStats, RandomStrategy, GreedyStrategy,
    Rules, Simulator, SimulatorConfig, StandardRules, Strategy,
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
    names
        .iter()
        .map(|name| match name.as_str() {
            "Random" => Ok(Box::new(RandomStrategy) as Box<dyn Strategy>),
            "Greedy" => Ok(Box::new(GreedyStrategy) as Box<dyn Strategy>),
            _ => Err(format!("Unknown strategy: {name}")),
        })
        .collect()
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
    let history = game.play().map_err(|e| format!("Game play failed: {e:?}"))?;

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
    let history = game.play().map_err(|e| format!("Game play failed: {e:?}"))?;

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
    serde_json::to_string(&["Random", "Greedy"]).unwrap()
}

#[wasm_bindgen]
pub fn get_available_rules() -> String {
    serde_json::to_string(&["Standard"]).unwrap()
}
