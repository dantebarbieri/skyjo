use serde::{Deserialize, Serialize};

use crate::game::Game;
use crate::history::GameHistory;
use crate::rules::Rules;
use crate::strategy::Strategy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorConfig {
    pub num_games: usize,
    pub base_seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub winners: Vec<usize>,
    pub final_scores: Vec<i32>,
    pub num_rounds: usize,
    pub total_turns: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    pub num_games: usize,
    pub num_players: usize,
    pub wins_per_player: Vec<usize>,
    pub win_rate_per_player: Vec<f64>,
    pub avg_score_per_player: Vec<f64>,
    pub min_score_per_player: Vec<i32>,
    pub max_score_per_player: Vec<i32>,
    pub avg_rounds_per_game: f64,
    pub avg_turns_per_game: f64,
    pub score_distributions: Vec<Vec<i32>>,
}

pub struct Simulator {
    config: SimulatorConfig,
    rules_factory: Box<dyn Fn() -> Box<dyn Rules>>,
    strategy_factories: Vec<Box<dyn Fn() -> Box<dyn Strategy>>>,
}

impl Simulator {
    pub fn new(
        config: SimulatorConfig,
        rules_factory: Box<dyn Fn() -> Box<dyn Rules>>,
        strategy_factories: Vec<Box<dyn Fn() -> Box<dyn Strategy>>>,
    ) -> Self {
        Simulator {
            config,
            rules_factory,
            strategy_factories,
        }
    }

    /// Run all games, returning full histories and aggregate stats.
    pub fn run(&self) -> (Vec<GameHistory>, AggregateStats) {
        let num_players = self.strategy_factories.len();
        let mut histories = Vec::with_capacity(self.config.num_games);
        let mut all_stats = Vec::with_capacity(self.config.num_games);

        for i in 0..self.config.num_games {
            let seed = self.config.base_seed.wrapping_add(i as u64);
            let rules = (self.rules_factory)();
            let strategies: Vec<Box<dyn Strategy>> =
                self.strategy_factories.iter().map(|f| f()).collect();
            let game = Game::new(rules, strategies, seed).unwrap();
            let history = game.play().unwrap();
            let stats = GameStats {
                winners: history.winners.clone(),
                final_scores: history.final_scores.clone(),
                num_rounds: history.rounds.len(),
                total_turns: history.rounds.iter().map(|r| r.turns.len()).sum(),
            };
            all_stats.push(stats);
            histories.push(history);
        }

        let aggregate = Self::compute_aggregate(&all_stats, num_players);
        (histories, aggregate)
    }

    /// Run games without keeping histories (memory-efficient for large batches).
    pub fn run_stats_only(&self) -> AggregateStats {
        let num_players = self.strategy_factories.len();
        let mut all_stats = Vec::with_capacity(self.config.num_games);

        for i in 0..self.config.num_games {
            let seed = self.config.base_seed.wrapping_add(i as u64);
            let rules = (self.rules_factory)();
            let strategies: Vec<Box<dyn Strategy>> =
                self.strategy_factories.iter().map(|f| f()).collect();
            let game = Game::new(rules, strategies, seed).unwrap();
            let history = game.play().unwrap();
            let stats = GameStats {
                winners: history.winners.clone(),
                final_scores: history.final_scores.clone(),
                num_rounds: history.rounds.len(),
                total_turns: history.rounds.iter().map(|r| r.turns.len()).sum(),
            };
            all_stats.push(stats);
        }

        Self::compute_aggregate(&all_stats, num_players)
    }

    fn compute_aggregate(stats: &[GameStats], num_players: usize) -> AggregateStats {
        let num_games = stats.len();

        let mut wins_per_player = vec![0usize; num_players];
        let mut score_sums = vec![0i64; num_players];
        let mut min_scores = vec![i32::MAX; num_players];
        let mut max_scores = vec![i32::MIN; num_players];
        let mut score_distributions: Vec<Vec<i32>> = vec![Vec::new(); num_players];
        let mut total_rounds: usize = 0;
        let mut total_turns: usize = 0;

        for game in stats {
            for &winner in &game.winners {
                wins_per_player[winner] += 1;
            }
            for (p, &score) in game.final_scores.iter().enumerate() {
                score_sums[p] += score as i64;
                min_scores[p] = min_scores[p].min(score);
                max_scores[p] = max_scores[p].max(score);
                score_distributions[p].push(score);
            }
            total_rounds += game.num_rounds;
            total_turns += game.total_turns;
        }

        AggregateStats {
            num_games,
            num_players,
            wins_per_player: wins_per_player.clone(),
            win_rate_per_player: wins_per_player
                .iter()
                .map(|&w| w as f64 / num_games as f64)
                .collect(),
            avg_score_per_player: score_sums
                .iter()
                .map(|&s| s as f64 / num_games as f64)
                .collect(),
            min_score_per_player: min_scores,
            max_score_per_player: max_scores,
            avg_rounds_per_game: total_rounds as f64 / num_games as f64,
            avg_turns_per_game: total_turns as f64 / num_games as f64,
            score_distributions,
        }
    }
}
