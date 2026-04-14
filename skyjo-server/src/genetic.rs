use std::path::PathBuf;
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use skyjo_core::game::Game;
use skyjo_core::rules::StandardRules;
use skyjo_core::strategy::Strategy;
use skyjo_core::{
    ARCHITECTURE_VERSION, ClearerStrategy, DefensiveStrategy, GENOME_SIZE, GeneticStrategy,
    GreedyStrategy, HIDDEN_SIZE, HIDDEN1_SIZE, HIDDEN2_SIZE, INPUT_GROUPS, INPUT_LABELS,
    INPUT_SIZE, OUTPUT_GROUPS, OUTPUT_LABELS, OUTPUT_SIZE, RandomStrategy, StatisticianStrategy,
};

// --- Configuration constants ---

pub const POPULATION_SIZE: usize = 100;
pub const GAMES_PER_INDIVIDUAL: usize = 30;
pub const TOURNAMENT_SIZE: usize = 5;
pub const BASE_MUTATION_RATE: f64 = 0.05;
pub const BASE_MUTATION_SIGMA: f32 = 0.3;
pub const BASE_RESET_RATE: f64 = 0.005;
pub const ELITISM_COUNT: usize = 5;
pub const NUM_OPPONENTS: usize = 3; // opponents per game (4-player games)
/// Number of recent generations to track for adaptive mutation.
pub const STAGNATION_WINDOW: usize = 20;
/// Periodic checkpoint interval (in generations).
pub const CHECKPOINT_INTERVAL: usize = 1000;
/// Maximum number of periodic checkpoints to retain (older ones are pruned).
pub const MAX_PERIODIC_CHECKPOINTS: usize = 10;

// --- Types ---

/// A saved snapshot of a generation's best genome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedGeneration {
    pub name: String,
    pub generation: usize,
    pub total_games_trained: usize,
    pub best_fitness: f64,
    pub genome: Vec<f32>,
    pub saved_at: String,
    /// Lineage hash identifying which training run produced this genome.
    #[serde(default)]
    pub lineage_hash: String,
    #[serde(default)]
    pub architecture_version: u32,
}

/// Summary of a saved generation (without genome, for listing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedGenerationInfo {
    pub name: String,
    pub generation: usize,
    pub total_games_trained: usize,
    pub best_fitness: f64,
    pub saved_at: String,
    #[serde(default)]
    pub lineage_hash: String,
    #[serde(default)]
    pub architecture_version: u32,
}

impl From<&SavedGeneration> for SavedGenerationInfo {
    fn from(sg: &SavedGeneration) -> Self {
        Self {
            name: sg.name.clone(),
            generation: sg.generation,
            total_games_trained: sg.total_games_trained,
            best_fitness: sg.best_fitness,
            saved_at: sg.saved_at.clone(),
            lineage_hash: sg.lineage_hash.clone(),
            architecture_version: sg.architecture_version,
        }
    }
}

/// Persistent model data, serialized to/from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneticModelData {
    pub best_genome: Vec<f32>,
    pub input_size: usize,
    pub hidden_size: usize,
    pub hidden1_size: usize,
    pub hidden2_size: usize,
    pub output_size: usize,
    pub generation: usize,
    pub total_games_trained: usize,
    pub input_labels: Vec<String>,
    pub output_labels: Vec<String>,
    pub input_groups: Vec<(String, usize, usize)>,
    pub output_groups: Vec<(String, usize, usize)>,
    #[serde(default)]
    pub saved_generations: Vec<SavedGeneration>,
    #[serde(default)]
    pub lineage_hash: String,
    #[serde(default)]
    pub architecture_version: u32,
}

impl GeneticModelData {
    fn from_state(state: &GeneticTrainingState) -> Self {
        Self {
            best_genome: state.best_genome.clone(),
            input_size: INPUT_SIZE,
            hidden_size: HIDDEN_SIZE,
            hidden1_size: HIDDEN1_SIZE,
            hidden2_size: HIDDEN2_SIZE,
            output_size: OUTPUT_SIZE,
            generation: state.generation,
            total_games_trained: state.total_games_trained,
            input_labels: INPUT_LABELS.iter().map(|s| s.to_string()).collect(),
            output_labels: OUTPUT_LABELS.iter().map(|s| s.to_string()).collect(),
            input_groups: INPUT_GROUPS
                .iter()
                .map(|(name, start, end)| (name.to_string(), *start, *end))
                .collect(),
            output_groups: OUTPUT_GROUPS
                .iter()
                .map(|(name, start, end)| (name.to_string(), *start, *end))
                .collect(),
            saved_generations: state.saved_generations.clone(),
            lineage_hash: state.lineage_hash.clone(),
            architecture_version: ARCHITECTURE_VERSION,
        }
    }
}

/// Training status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStatus {
    pub is_training: bool,
    pub generation: usize,
    pub total_games_trained: usize,
    pub best_fitness: f64,
    /// The generation that training started at (for progress calculation).
    pub training_start_generation: usize,
    /// The target generation (start + requested generations).
    pub training_target_generation: usize,
    /// Milliseconds elapsed since training started (server-tracked).
    pub training_elapsed_ms: u64,
    /// Milliseconds elapsed at the last generation completion (for stable ETA).
    pub training_last_gen_elapsed_ms: u64,
    /// Training mode: "generations", "until_generation", or "until_fitness".
    pub training_mode: String,
    /// For fitness-based training, the target fitness threshold.
    pub training_target_fitness: f64,
    /// Best fitness when training started (for ETA extrapolation in fitness mode).
    pub training_start_fitness: f64,
    /// Lineage hash identifying the current training run.
    pub lineage_hash: String,
    /// Current adaptive mutation rate.
    pub current_mutation_rate: f64,
    /// Current adaptive mutation sigma.
    pub current_mutation_sigma: f32,
}

/// Mutable training state, shared behind Arc<Mutex<>>.
pub struct GeneticTrainingState {
    pub population: Vec<Vec<f32>>,
    pub best_genome: Vec<f32>,
    pub best_fitness: f64,
    pub generation: usize,
    pub total_games_trained: usize,
    pub is_training: bool,
    pub training_start_generation: usize,
    pub training_target_generation: usize,
    pub training_started_at: Option<std::time::Instant>,
    /// Elapsed ms snapshot at the last generation completion.
    pub training_last_gen_elapsed_ms: u64,
    pub model_path: PathBuf,
    pub saved_generations: Vec<SavedGeneration>,
    /// Training mode: "generations", "until_generation", or "until_fitness".
    pub training_mode: String,
    /// For fitness-based training, the target fitness threshold.
    pub training_target_fitness: f64,
    /// Best fitness when training started (for ETA extrapolation).
    pub training_start_fitness: f64,
    /// Lineage hash identifying the current training run.
    pub lineage_hash: String,
    /// Rolling window of best fitness values for stagnation detection.
    pub fitness_history: Vec<f64>,
    /// Current adaptive mutation rate (starts at BASE_MUTATION_RATE).
    pub current_mutation_rate: f64,
    /// Current adaptive mutation sigma (starts at BASE_MUTATION_SIGMA).
    pub current_mutation_sigma: f32,
    /// Current adaptive reset rate (starts at BASE_RESET_RATE).
    pub current_reset_rate: f64,
}

impl GeneticTrainingState {
    /// Create a new training state with a random population.
    pub fn new_random(model_path: PathBuf) -> Self {
        let mut rng = StdRng::from_os_rng();
        let population: Vec<Vec<f32>> = (0..POPULATION_SIZE)
            .map(|_| random_genome(&mut rng))
            .collect();
        let best_genome = population[0].clone();
        let lineage_hash = compute_lineage_hash(&best_genome);
        Self {
            population,
            best_genome,
            best_fitness: f64::NEG_INFINITY,
            generation: 0,
            total_games_trained: 0,
            is_training: false,
            training_start_generation: 0,
            training_target_generation: 0,
            training_started_at: None,
            training_last_gen_elapsed_ms: 0,
            model_path,
            saved_generations: Vec::new(),
            training_mode: "generations".to_string(),
            training_target_fitness: 0.0,
            training_start_fitness: 0.0,
            lineage_hash,
            fitness_history: Vec::new(),
            current_mutation_rate: BASE_MUTATION_RATE,
            current_mutation_sigma: BASE_MUTATION_SIGMA,
            current_reset_rate: BASE_RESET_RATE,
        }
    }

    /// Load from a saved model file, or create a new random state.
    pub fn load_or_new(model_path: PathBuf) -> Self {
        if model_path.exists() {
            match std::fs::read_to_string(&model_path) {
                Ok(json) => match serde_json::from_str::<GeneticModelData>(&json) {
                    Ok(data) => {
                        if data.architecture_version != 0
                            && data.architecture_version != ARCHITECTURE_VERSION
                        {
                            tracing::warn!(
                                "Model architecture version {} does not match current version {}, creating new random model",
                                data.architecture_version,
                                ARCHITECTURE_VERSION
                            );
                            return Self::new_random(model_path);
                        }
                        if data.best_genome.len() != GENOME_SIZE {
                            tracing::warn!(
                                "Model genome size {} does not match expected {GENOME_SIZE}, creating new random model",
                                data.best_genome.len()
                            );
                            return Self::new_random(model_path);
                        }
                        tracing::info!(
                            "Loaded genetic model: generation {}, {} games trained, {} saved generations",
                            data.generation,
                            data.total_games_trained,
                            data.saved_generations.len(),
                        );
                        let mut rng = StdRng::from_os_rng();
                        // Rebuild population around the best genome
                        let mut population = Vec::with_capacity(POPULATION_SIZE);
                        population.push(data.best_genome.clone());
                        for _ in 1..POPULATION_SIZE {
                            let mut child = data.best_genome.clone();
                            mutate(
                                &mut child,
                                &mut rng,
                                BASE_MUTATION_RATE,
                                BASE_MUTATION_SIGMA,
                                BASE_RESET_RATE,
                            );
                            population.push(child);
                        }
                        // Backward compat: compute hash if not stored
                        let lineage_hash = if data.lineage_hash.is_empty() {
                            compute_lineage_hash(&data.best_genome)
                        } else {
                            data.lineage_hash
                        };
                        return Self {
                            population,
                            best_genome: data.best_genome,
                            best_fitness: f64::NEG_INFINITY, // will be re-evaluated
                            generation: data.generation,
                            total_games_trained: data.total_games_trained,
                            is_training: false,
                            training_start_generation: 0,
                            training_target_generation: 0,
                            training_started_at: None,
                            training_last_gen_elapsed_ms: 0,
                            model_path,
                            saved_generations: data.saved_generations,
                            training_mode: "generations".to_string(),
                            training_target_fitness: 0.0,
                            training_start_fitness: 0.0,
                            lineage_hash,
                            fitness_history: Vec::new(),
                            current_mutation_rate: BASE_MUTATION_RATE,
                            current_mutation_sigma: BASE_MUTATION_SIGMA,
                            current_reset_rate: BASE_RESET_RATE,
                        };
                    }
                    Err(e) => tracing::warn!("Failed to parse genetic model: {e}"),
                },
                Err(e) => tracing::warn!("Failed to read genetic model file: {e}"),
            }
        }
        tracing::info!("Creating new random genetic model");
        Self::new_random(model_path)
    }

    pub fn model_data(&self) -> GeneticModelData {
        GeneticModelData::from_state(self)
    }

    pub fn status(&self) -> TrainingStatus {
        let elapsed_ms = self
            .training_started_at
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        TrainingStatus {
            is_training: self.is_training,
            generation: self.generation,
            total_games_trained: self.total_games_trained,
            best_fitness: if self.best_fitness.is_finite() {
                self.best_fitness
            } else {
                0.0
            },
            training_start_generation: self.training_start_generation,
            training_target_generation: self.training_target_generation,
            training_elapsed_ms: elapsed_ms,
            training_last_gen_elapsed_ms: self.training_last_gen_elapsed_ms,
            training_mode: self.training_mode.clone(),
            training_target_fitness: self.training_target_fitness,
            training_start_fitness: self.training_start_fitness,
            lineage_hash: self.lineage_hash.clone(),
            current_mutation_rate: self.current_mutation_rate,
            current_mutation_sigma: self.current_mutation_sigma,
        }
    }

    /// Save the current generation as a named snapshot.
    pub fn save_generation(&mut self, name: Option<String>) -> Result<SavedGenerationInfo, String> {
        if self.generation == 0 {
            return Err("No generation to save (generation 0)".to_string());
        }
        let name = name.unwrap_or_else(|| format!("Gen {}", self.generation));

        // Check for duplicate name
        if self.saved_generations.iter().any(|sg| sg.name == name) {
            return Err(format!("A saved generation named '{name}' already exists"));
        }

        let saved = SavedGeneration {
            name: name.clone(),
            generation: self.generation,
            total_games_trained: self.total_games_trained,
            best_fitness: if self.best_fitness.is_finite() {
                self.best_fitness
            } else {
                0.0
            },
            genome: self.best_genome.clone(),
            saved_at: chrono_now(),
            lineage_hash: self.lineage_hash.clone(),
            architecture_version: ARCHITECTURE_VERSION,
        };
        let info = SavedGenerationInfo::from(&saved);
        self.saved_generations.push(saved);
        save_model(self);
        Ok(info)
    }

    /// Delete a saved generation by name.
    pub fn delete_saved_generation(&mut self, name: &str) -> Result<(), String> {
        let idx = self
            .saved_generations
            .iter()
            .position(|sg| sg.name == name)
            .ok_or_else(|| format!("No saved generation named '{name}'"))?;
        self.saved_generations.remove(idx);
        save_model(self);
        Ok(())
    }

    /// List all saved generations (without genomes).
    pub fn list_saved_generations(&self) -> Vec<SavedGenerationInfo> {
        self.saved_generations
            .iter()
            .map(SavedGenerationInfo::from)
            .collect()
    }

    /// Get a specific saved generation's full model data (with genome).
    pub fn get_saved_generation_model(&self, name: &str) -> Result<GeneticModelData, String> {
        let saved = self
            .saved_generations
            .iter()
            .find(|sg| sg.name == name)
            .ok_or_else(|| format!("No saved generation named '{name}'"))?;

        Ok(GeneticModelData {
            best_genome: saved.genome.clone(),
            input_size: INPUT_SIZE,
            hidden_size: HIDDEN_SIZE,
            hidden1_size: HIDDEN1_SIZE,
            hidden2_size: HIDDEN2_SIZE,
            output_size: OUTPUT_SIZE,
            generation: saved.generation,
            total_games_trained: saved.total_games_trained,
            input_labels: INPUT_LABELS.iter().map(|s| s.to_string()).collect(),
            output_labels: OUTPUT_LABELS.iter().map(|s| s.to_string()).collect(),
            input_groups: INPUT_GROUPS
                .iter()
                .map(|(n, s, e)| (n.to_string(), *s, *e))
                .collect(),
            output_groups: OUTPUT_GROUPS
                .iter()
                .map(|(n, s, e)| (n.to_string(), *s, *e))
                .collect(),
            saved_generations: Vec::new(), // Don't nest saved generations
            lineage_hash: saved.lineage_hash.clone(),
            architecture_version: saved.architecture_version,
        })
    }

    /// Import an external genome as a saved generation.
    #[allow(clippy::too_many_arguments)]
    pub fn import_generation(
        &mut self,
        name: String,
        genome: Vec<f32>,
        generation: usize,
        total_games_trained: usize,
        best_fitness: f64,
        lineage_hash: Option<String>,
        architecture_version: Option<u32>,
    ) -> Result<SavedGenerationInfo, String> {
        if genome.len() != GENOME_SIZE {
            return Err(format!(
                "Invalid genome size: expected {GENOME_SIZE}, got {}",
                genome.len()
            ));
        }
        if self.saved_generations.iter().any(|sg| sg.name == name) {
            return Err(format!("A saved generation named '{name}' already exists"));
        }
        let lineage_hash = lineage_hash.unwrap_or_else(|| compute_lineage_hash(&genome));
        let saved = SavedGeneration {
            name,
            generation,
            total_games_trained,
            best_fitness,
            genome,
            saved_at: chrono_now(),
            lineage_hash,
            architecture_version: architecture_version.unwrap_or(0),
        };
        let info = SavedGenerationInfo::from(&saved);
        self.saved_generations.push(saved);
        save_model(self);
        Ok(info)
    }

    /// Get genome for a specific saved generation (for constructing strategy).
    pub fn get_saved_genome(&self, name: &str) -> Option<(Vec<f32>, usize)> {
        self.saved_generations
            .iter()
            .find(|sg| sg.name == name)
            .map(|sg| (sg.genome.clone(), sg.total_games_trained))
    }

    /// Load a saved generation as the current model, rebuilding population around it.
    pub fn load_saved(&mut self, name: &str) -> Result<(), String> {
        let saved = self
            .saved_generations
            .iter()
            .find(|sg| sg.name == name)
            .ok_or_else(|| format!("No saved generation named '{name}'"))?
            .clone();
        let mut rng = StdRng::from_os_rng();
        let mut population = Vec::with_capacity(POPULATION_SIZE);
        population.push(saved.genome.clone());
        for _ in 1..POPULATION_SIZE {
            let mut child = saved.genome.clone();
            mutate(
                &mut child,
                &mut rng,
                BASE_MUTATION_RATE,
                BASE_MUTATION_SIGMA,
                BASE_RESET_RATE,
            );
            population.push(child);
        }
        self.population = population;
        self.best_genome = saved.genome;
        self.best_fitness = f64::NEG_INFINITY; // will be re-evaluated
        self.generation = saved.generation;
        self.total_games_trained = saved.total_games_trained;
        self.lineage_hash = if saved.lineage_hash.is_empty() {
            compute_lineage_hash(&self.best_genome)
        } else {
            saved.lineage_hash
        };
        self.fitness_history.clear();
        self.current_mutation_rate = BASE_MUTATION_RATE;
        self.current_mutation_sigma = BASE_MUTATION_SIGMA;
        self.current_reset_rate = BASE_RESET_RATE;
        save_model(self);
        Ok(())
    }

    /// Reset to a new random population (Generation 0) with a new lineage hash.
    /// Preserved saved generations are kept (they have their own lineage hashes).
    pub fn reset(&mut self) {
        let mut rng = StdRng::from_os_rng();
        self.population = (0..POPULATION_SIZE)
            .map(|_| random_genome(&mut rng))
            .collect();
        self.best_genome = self.population[0].clone();
        self.best_fitness = f64::NEG_INFINITY;
        self.generation = 0;
        self.total_games_trained = 0;
        self.lineage_hash = compute_lineage_hash(&self.best_genome);
        self.fitness_history.clear();
        self.current_mutation_rate = BASE_MUTATION_RATE;
        self.current_mutation_sigma = BASE_MUTATION_SIGMA;
        self.current_reset_rate = BASE_RESET_RATE;
        save_model(self);
    }
}

/// Returns seconds since Unix epoch as a string timestamp.
fn chrono_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

/// Compute a short lineage hash from a genome (first 8 hex chars of FNV-1a hash).
fn compute_lineage_hash(genome: &[f32]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for &val in genome {
        let bytes = val.to_le_bytes();
        for &b in &bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3); // FNV prime
        }
    }
    format!("{:08x}", hash as u32) // Take lower 32 bits for 8 hex chars
}

// --- Genetic algorithm operations ---

fn random_genome(rng: &mut dyn RngCore) -> Vec<f32> {
    (0..GENOME_SIZE)
        .map(|_| {
            let bits = rng.next_u32();
            (bits as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

/// Tournament selection: pick the best of `TOURNAMENT_SIZE` random individuals.
fn tournament_select(population: &[Vec<f32>], fitnesses: &[f64], rng: &mut impl Rng) -> Vec<f32> {
    let mut best_idx = rng.random_range(0..population.len());
    let mut best_fit = fitnesses[best_idx];
    for _ in 1..TOURNAMENT_SIZE {
        let idx = rng.random_range(0..population.len());
        if fitnesses[idx] > best_fit {
            best_idx = idx;
            best_fit = fitnesses[idx];
        }
    }
    population[best_idx].clone()
}

/// BLX-α blend crossover: for each gene, sample uniformly from the interval
/// [min(a,b) - α*d, max(a,b) + α*d] where d = |a - b| and α = 0.5.
/// This explores beyond the parents' range, unlike simple arithmetic crossover.
fn crossover(parent_a: &[f32], parent_b: &[f32], rng: &mut impl Rng) -> Vec<f32> {
    const ALPHA: f32 = 0.5;
    parent_a
        .iter()
        .zip(parent_b.iter())
        .map(|(&a, &b)| {
            let lo = a.min(b);
            let hi = a.max(b);
            let d = hi - lo;
            let expanded_lo = lo - ALPHA * d;
            let expanded_hi = hi + ALPHA * d;
            rng.random_range(expanded_lo..=expanded_hi)
        })
        .collect()
}

/// Mutate a genome in place with Gaussian perturbation and occasional resets.
fn mutate(
    genome: &mut [f32],
    rng: &mut impl Rng,
    mutation_rate: f64,
    mutation_sigma: f32,
    reset_rate: f64,
) {
    for gene in genome.iter_mut() {
        let r: f64 = rng.random();
        if r < reset_rate {
            *gene = rng.random_range(-1.0f32..1.0);
        } else if r < reset_rate + mutation_rate {
            // Box-Muller approximation for normal distribution
            let u1: f32 = rng.random_range(0.0001f32..1.0);
            let u2: f32 = rng.random_range(0.0f32..std::f32::consts::TAU);
            let normal = (-2.0 * u1.ln()).sqrt() * u2.cos();
            *gene += normal * mutation_sigma;
        }
    }
}

/// Select an opponent strategy based on the weighted mix.
/// 40% Statistician, 20% Defensive/Clearer, 20% self-play, 20% Greedy/Random.
fn select_opponent(
    rng: &mut impl Rng,
    population: &[Vec<f32>],
    current_idx: usize,
    games_trained: usize,
) -> Box<dyn Strategy> {
    let r: f64 = rng.random();
    if r < 0.40 {
        Box::new(StatisticianStrategy)
    } else if r < 0.60 {
        if rng.random_bool(0.5) {
            Box::new(DefensiveStrategy)
        } else {
            Box::new(ClearerStrategy)
        }
    } else if r < 0.80 {
        // Self-play: pick a random other individual
        let mut idx = rng.random_range(0..population.len());
        if idx == current_idx && population.len() > 1 {
            idx = (idx + 1) % population.len();
        }
        Box::new(GeneticStrategy::new(population[idx].clone(), games_trained))
    } else if rng.random_bool(0.5) {
        Box::new(GreedyStrategy)
    } else {
        Box::new(RandomStrategy)
    }
}

/// Evaluate fitness for a single individual by playing games.
/// Returns a composite fitness score (higher = better).
fn evaluate_individual(
    genome: &[f32],
    individual_idx: usize,
    population: &[Vec<f32>],
    base_seed: u64,
    games_trained: usize,
) -> f64 {
    let mut total_score: f64 = 0.0;
    let mut total_wins: usize = 0;
    let mut total_score_diff: f64 = 0.0;
    let mut rng = StdRng::seed_from_u64(base_seed);

    for game_idx in 0..GAMES_PER_INDIVIDUAL {
        let seed = base_seed.wrapping_add(game_idx as u64);

        let mut strategies: Vec<Box<dyn Strategy>> = Vec::with_capacity(1 + NUM_OPPONENTS);
        strategies.push(Box::new(GeneticStrategy::new(
            genome.to_vec(),
            games_trained,
        )));
        for _ in 0..NUM_OPPONENTS {
            strategies.push(select_opponent(
                &mut rng,
                population,
                individual_idx,
                games_trained,
            ));
        }

        let rules = Box::new(StandardRules);
        match Game::new(rules, strategies, seed) {
            Ok(game) => match game.play() {
                Ok(history) => {
                    let my_score = history.final_scores[0] as f64;
                    let min_other = history.final_scores[1..]
                        .iter()
                        .copied()
                        .min()
                        .unwrap_or(i32::MAX) as f64;
                    let is_winner = history.winners.contains(&0);

                    // Base score penalty
                    let penalized = if !is_winner && my_score > 0.0 {
                        my_score * 2.0
                    } else {
                        my_score
                    };
                    total_score += penalized;

                    // Win bonus
                    if is_winner {
                        total_wins += 1;
                    }

                    // Score differential (how much better than best opponent)
                    total_score_diff += min_other - my_score;
                }
                Err(e) => {
                    tracing::warn!("Game play error during training: {e}");
                    total_score += 200.0;
                }
            },
            Err(e) => {
                tracing::warn!("Game creation error during training: {e}");
                total_score += 200.0;
            }
        }
    }

    let n = GAMES_PER_INDIVIDUAL as f64;
    // Fitness = -(avg score) + win bonus + score differential bonus
    let avg_score = total_score / n;
    let win_rate = total_wins as f64 / n;
    let avg_diff = total_score_diff / n;

    -avg_score + (win_rate * 10.0) + (avg_diff * 0.1)
}

/// Run one generation of the genetic algorithm.
/// Returns (new_population, fitnesses, best_idx, games_played).
fn run_generation(
    population: &[Vec<f32>],
    generation_seed: u64,
    games_trained: usize,
    mutation_rate: f64,
    mutation_sigma: f32,
    reset_rate: f64,
) -> (Vec<Vec<f32>>, Vec<f64>, usize, usize) {
    let mut rng = StdRng::seed_from_u64(generation_seed);

    // Evaluate fitness for each individual in parallel
    let fitnesses: Vec<f64> = population
        .par_iter()
        .enumerate()
        .map(|(idx, genome)| {
            let seed = generation_seed.wrapping_add((idx * 1000) as u64);
            evaluate_individual(genome, idx, population, seed, games_trained)
        })
        .collect();

    // Find best individual
    let best_idx = fitnesses
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);

    // Sort by fitness (descending) for elitism
    let mut indexed: Vec<(usize, f64)> = fitnesses.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Build next generation
    let mut next_population = Vec::with_capacity(population.len());

    // Elitism: keep top individuals unchanged
    for i in 0..ELITISM_COUNT.min(population.len()) {
        next_population.push(population[indexed[i].0].clone());
    }

    // Fill remaining with offspring
    while next_population.len() < population.len() {
        let parent_a = tournament_select(population, &fitnesses, &mut rng);
        let parent_b = tournament_select(population, &fitnesses, &mut rng);
        let mut child = crossover(&parent_a, &parent_b, &mut rng);
        mutate(
            &mut child,
            &mut rng,
            mutation_rate,
            mutation_sigma,
            reset_rate,
        );
        next_population.push(child);
    }

    let games_played = population.len() * GAMES_PER_INDIVIDUAL;
    (next_population, fitnesses, best_idx, games_played)
}

/// Auto-save at power-of-10 generation milestones (1, 10, 100, 1000, 10000, ...).
fn auto_save_milestone(state: &mut GeneticTrainingState) {
    let generation = state.generation;
    if generation == 0 {
        return;
    }
    // Check if generation is a power of 10
    let mut power = 1usize;
    while power <= generation {
        if power == generation {
            let name = format!("Gen {generation}");
            if state.saved_generations.iter().any(|sg| sg.name == name) {
                return;
            }
            let saved = SavedGeneration {
                name: name.clone(),
                generation,
                total_games_trained: state.total_games_trained,
                best_fitness: if state.best_fitness.is_finite() {
                    state.best_fitness
                } else {
                    0.0
                },
                genome: state.best_genome.clone(),
                saved_at: chrono_now(),
                lineage_hash: state.lineage_hash.clone(),
                architecture_version: ARCHITECTURE_VERSION,
            };
            state.saved_generations.push(saved);
            tracing::info!("Auto-saved milestone: {name}");
            return;
        }
        power = power.saturating_mul(10);
    }
}

/// Save the model to disk.
fn save_model(state: &GeneticTrainingState) {
    let data = state.model_data();
    match serde_json::to_string_pretty(&data) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&state.model_path, json) {
                tracing::error!("Failed to save genetic model: {e}");
            } else {
                tracing::info!(
                    "Saved genetic model: generation {}, {} games",
                    state.generation,
                    state.total_games_trained,
                );
            }
        }
        Err(e) => tracing::error!("Failed to serialize genetic model: {e}"),
    }
}

/// Run training for a given number of generations.
/// This is designed to be called from `tokio::task::spawn_blocking`.
pub async fn train_generations(state: Arc<Mutex<GeneticTrainingState>>, num_generations: usize) {
    for generation_i in 0..num_generations {
        // Clone population from state (brief lock)
        let (
            population,
            generation_num,
            games_trained,
            target_fitness,
            mode,
            mutation_rate,
            mutation_sigma,
            reset_rate,
        ) = {
            let s = state.lock().await;
            if !s.is_training {
                tracing::info!("Training was stopped, ending at generation {generation_i}");
                return;
            }
            (
                s.population.clone(),
                s.generation,
                s.total_games_trained,
                s.training_target_fitness,
                s.training_mode.clone(),
                s.current_mutation_rate,
                s.current_mutation_sigma,
                s.current_reset_rate,
            )
        };

        // Run the CPU-intensive evaluation without holding the lock
        let generation_seed = (generation_num as u64).wrapping_mul(7919) ^ 0xDEADBEEF;
        let (new_population, fitnesses, best_idx, games_played) =
            tokio::task::spawn_blocking(move || {
                run_generation(
                    &population,
                    generation_seed,
                    games_trained,
                    mutation_rate,
                    mutation_sigma,
                    reset_rate,
                )
            })
            .await
            .expect("Training task panicked");

        // Write back results (brief lock)
        let should_stop = {
            let mut s = state.lock().await;
            let best_fitness = fitnesses[best_idx];

            if best_fitness > s.best_fitness || s.generation == 0 {
                s.best_genome = new_population[0].clone(); // elite[0] is the best from this gen
                s.best_fitness = best_fitness;
            }

            // Capture the first real fitness as the training start baseline
            // (before training, best_fitness may be NEG_INFINITY → 0.0 placeholder)
            if generation_i == 0 {
                s.training_start_fitness = s.best_fitness;
            }

            s.population = new_population;
            s.generation += 1;
            s.total_games_trained += games_played;
            s.training_last_gen_elapsed_ms = s
                .training_started_at
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or(0);

            auto_save_milestone(&mut s);
            update_adaptive_mutation(&mut s);

            // Periodic checkpoint
            if s.generation % CHECKPOINT_INTERVAL == 0 {
                auto_save_checkpoint(&mut s);
            }

            tracing::info!(
                "Generation {} complete: best_fitness={:.2}, total_games={}",
                s.generation,
                s.best_fitness,
                s.total_games_trained,
            );

            save_model(&s);

            // Check fitness-based early stop
            mode == "until_fitness" && s.best_fitness >= target_fitness
        };

        if should_stop {
            let mut s = state.lock().await;
            s.is_training = false;
            s.training_started_at = None;
            auto_save_training_result(&mut s);
            tracing::info!(
                "Fitness target {} reached at generation {}",
                target_fitness,
                s.generation
            );
            return;
        }
    }

    // Mark training as complete
    {
        let mut s = state.lock().await;
        s.is_training = false;
        s.training_started_at = None;
        auto_save_training_result(&mut s);
        tracing::info!("Training complete after {num_generations} generations");
    }
}

/// Auto-save when training finishes (any mode). Skips if the generation was
/// already saved (e.g. by `auto_save_milestone` at a power-of-10 boundary).
fn auto_save_training_result(state: &mut GeneticTrainingState) {
    if state.generation == 0 {
        return;
    }
    let name = format!("Gen {}", state.generation);
    if state.saved_generations.iter().any(|sg| sg.name == name) {
        return;
    }
    let saved = SavedGeneration {
        name: name.clone(),
        generation: state.generation,
        total_games_trained: state.total_games_trained,
        best_fitness: if state.best_fitness.is_finite() {
            state.best_fitness
        } else {
            0.0
        },
        genome: state.best_genome.clone(),
        saved_at: chrono_now(),
        lineage_hash: state.lineage_hash.clone(),
        architecture_version: ARCHITECTURE_VERSION,
    };
    state.saved_generations.push(saved);
    save_model(state);
    tracing::info!("Auto-saved training result: {name}");
}

/// Update adaptive mutation parameters based on fitness history.
fn update_adaptive_mutation(state: &mut GeneticTrainingState) {
    // Add current best fitness to history
    if state.best_fitness.is_finite() {
        state.fitness_history.push(state.best_fitness);
    }

    // Need at least STAGNATION_WINDOW entries to assess
    if state.fitness_history.len() < STAGNATION_WINDOW {
        return;
    }

    // Keep only the last STAGNATION_WINDOW entries
    let len = state.fitness_history.len();
    if len > STAGNATION_WINDOW {
        state.fitness_history.drain(..len - STAGNATION_WINDOW);
    }

    // Calculate improvement over the window
    let oldest = state.fitness_history[0];
    let newest = *state.fitness_history.last().unwrap();
    let improvement_pct = if oldest.abs() > 0.01 {
        ((newest - oldest) / oldest.abs()) * 100.0
    } else {
        (newest - oldest) * 100.0
    };

    // Stagnation: less than 1% improvement over window
    if improvement_pct < 1.0 {
        // Increase mutation (bounded)
        state.current_mutation_rate = (state.current_mutation_rate * 1.2).min(0.15);
        state.current_mutation_sigma = (state.current_mutation_sigma * 1.2).min(1.0);
        state.current_reset_rate = (state.current_reset_rate * 1.5).min(0.02);
    } else {
        // Good progress — decrease toward baseline
        state.current_mutation_rate = (state.current_mutation_rate * 0.95).max(BASE_MUTATION_RATE);
        state.current_mutation_sigma =
            (state.current_mutation_sigma * 0.95).max(BASE_MUTATION_SIGMA);
        state.current_reset_rate = (state.current_reset_rate * 0.95).max(BASE_RESET_RATE);
    }
}

/// Auto-save at periodic checkpoint intervals.
fn auto_save_checkpoint(state: &mut GeneticTrainingState) {
    if state.generation == 0 {
        return;
    }
    let name = format!("Gen {}", state.generation);
    if state.saved_generations.iter().any(|sg| sg.name == name) {
        return;
    }
    let saved = SavedGeneration {
        name: name.clone(),
        generation: state.generation,
        total_games_trained: state.total_games_trained,
        best_fitness: if state.best_fitness.is_finite() {
            state.best_fitness
        } else {
            0.0
        },
        genome: state.best_genome.clone(),
        saved_at: chrono_now(),
        lineage_hash: state.lineage_hash.clone(),
        architecture_version: ARCHITECTURE_VERSION,
    };
    state.saved_generations.push(saved);
    tracing::info!("Periodic checkpoint saved: {name}");

    // Prune old periodic checkpoints (keep only the most recent ones).
    // A periodic checkpoint has a name like "Gen 1000", "Gen 2000", etc.
    // We keep milestones (power-of-10), training results, and user-saved generations.
    let mut periodic: Vec<usize> = state
        .saved_generations
        .iter()
        .enumerate()
        .filter(|(_, sg)| {
            // Periodic checkpoints match "Gen N" where N is a multiple of CHECKPOINT_INTERVAL
            // but NOT a power of 10 (those are milestones we always keep)
            if let Some(rest) = sg.name.strip_prefix("Gen ")
                && let Ok(gen_num) = rest.parse::<usize>()
            {
                let is_checkpoint = gen_num % CHECKPOINT_INTERVAL == 0;
                let is_milestone = is_power_of_10(gen_num);
                return is_checkpoint && !is_milestone;
            }
            false
        })
        .map(|(i, _)| i)
        .collect();

    // Remove oldest periodic checkpoints beyond the cap
    if periodic.len() > MAX_PERIODIC_CHECKPOINTS {
        let to_remove = periodic.len() - MAX_PERIODIC_CHECKPOINTS;
        // Remove from the front (oldest), but indices shift as we remove
        periodic.truncate(to_remove);
        // Remove in reverse order to keep indices valid
        for &idx in periodic.iter().rev() {
            state.saved_generations.remove(idx);
        }
        tracing::info!("Pruned {to_remove} old periodic checkpoint(s)");
    }
}

/// Check if a number is a power of 10.
fn is_power_of_10(n: usize) -> bool {
    if n == 0 {
        return false;
    }
    let mut v = n;
    while v.is_multiple_of(10) {
        v /= 10;
    }
    v == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use skyjo_core::GENOME_SIZE;

    /// Create a GeneticTrainingState with a unique temp directory model path.
    fn test_state(name: &str) -> GeneticTrainingState {
        let dir = std::env::temp_dir().join(format!("skyjo_test_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let model_path = dir.join("model.json");
        GeneticTrainingState::new_random(model_path)
    }

    /// Cleanup helper: remove the parent directory of the model path.
    fn cleanup(state: &GeneticTrainingState) {
        if let Some(parent) = state.model_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    // ---- State Management ----

    #[test]
    fn new_random_creates_valid_population() {
        let state = test_state("new_random");
        assert_eq!(state.population.len(), POPULATION_SIZE);
        for genome in &state.population {
            assert_eq!(genome.len(), GENOME_SIZE);
        }
        assert_eq!(state.generation, 0);
        assert_eq!(state.total_games_trained, 0);
        assert!(!state.is_training);
        cleanup(&state);
    }

    #[test]
    fn status_returns_correct_initial_state() {
        let state = test_state("status_initial");
        let status = state.status();
        assert!(!status.is_training);
        assert_eq!(status.generation, 0);
        assert_eq!(status.total_games_trained, 0);
        assert_eq!(status.best_fitness, 0.0); // NEG_INFINITY maps to 0.0
        assert_eq!(status.training_mode, "generations");
        assert!(!status.lineage_hash.is_empty());
        cleanup(&state);
    }

    #[test]
    fn model_data_returns_genome_of_correct_size() {
        let state = test_state("model_data");
        let data = state.model_data();
        assert_eq!(data.best_genome.len(), GENOME_SIZE);
        assert_eq!(data.input_size, INPUT_SIZE);
        assert_eq!(data.hidden_size, HIDDEN_SIZE);
        assert_eq!(data.output_size, OUTPUT_SIZE);
        assert_eq!(data.generation, 0);
        assert!(!data.input_labels.is_empty());
        assert!(!data.output_labels.is_empty());
        cleanup(&state);
    }

    #[test]
    fn reset_creates_new_random_population() {
        let mut state = test_state("reset");
        let original_genome = state.best_genome.clone();
        let original_hash = state.lineage_hash.clone();
        state.reset();
        assert_eq!(state.population.len(), POPULATION_SIZE);
        assert_eq!(state.generation, 0);
        assert_eq!(state.total_games_trained, 0);
        // The new best genome should (almost certainly) differ from the original
        // and the lineage hash should change.
        let new_genome_differs = state.best_genome != original_genome;
        let new_hash_differs = state.lineage_hash != original_hash;
        // At least one should differ (extremely unlikely both are identical)
        assert!(
            new_genome_differs || new_hash_differs,
            "Reset should produce a different population"
        );
        cleanup(&state);
    }

    // ---- Saved Generations ----

    /// Helper: advance state to generation 1 so save_generation works.
    fn advance_to_gen1(state: &mut GeneticTrainingState) {
        state.generation = 1;
        state.total_games_trained = POPULATION_SIZE * GAMES_PER_INDIVIDUAL;
        state.best_fitness = -50.0;
    }

    #[test]
    fn save_generation_stores_and_retrieves() {
        let mut state = test_state("save_retrieve");
        advance_to_gen1(&mut state);
        let info = state
            .save_generation(Some("test_save".to_string()))
            .unwrap();
        assert_eq!(info.name, "test_save");
        assert_eq!(info.generation, 1);

        let model = state.get_saved_generation_model("test_save").unwrap();
        assert_eq!(model.best_genome.len(), GENOME_SIZE);
        assert_eq!(model.generation, 1);
        cleanup(&state);
    }

    #[test]
    fn list_saved_generations_returns_saved_items() {
        let mut state = test_state("list_saved");
        advance_to_gen1(&mut state);
        assert!(state.list_saved_generations().is_empty());

        state.save_generation(Some("save_a".to_string())).unwrap();
        state.generation = 2; // allow a second save
        state.save_generation(Some("save_b".to_string())).unwrap();

        let list = state.list_saved_generations();
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"save_a"));
        assert!(names.contains(&"save_b"));
        cleanup(&state);
    }

    #[test]
    fn delete_saved_generation_removes_correctly() {
        let mut state = test_state("delete_saved");
        advance_to_gen1(&mut state);
        state
            .save_generation(Some("to_delete".to_string()))
            .unwrap();
        assert_eq!(state.list_saved_generations().len(), 1);

        state.delete_saved_generation("to_delete").unwrap();
        assert!(state.list_saved_generations().is_empty());
        cleanup(&state);
    }

    #[test]
    fn get_saved_generation_model_returns_correct_data() {
        let mut state = test_state("get_saved_model");
        advance_to_gen1(&mut state);
        state
            .save_generation(Some("model_test".to_string()))
            .unwrap();

        let model = state.get_saved_generation_model("model_test").unwrap();
        assert_eq!(model.best_genome, state.best_genome);
        assert_eq!(model.generation, 1);
        assert_eq!(model.input_size, INPUT_SIZE);

        // Nonexistent name should fail
        assert!(state.get_saved_generation_model("nonexistent").is_err());
        cleanup(&state);
    }

    #[test]
    fn save_generation_fails_with_duplicate_name() {
        let mut state = test_state("dup_name");
        advance_to_gen1(&mut state);
        state.save_generation(Some("dup".to_string())).unwrap();

        let result = state.save_generation(Some("dup".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
        cleanup(&state);
    }

    #[test]
    fn delete_nonexistent_fails_gracefully() {
        let mut state = test_state("delete_nonexist");
        let result = state.delete_saved_generation("nope");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No saved generation"));
        cleanup(&state);
    }

    #[test]
    fn import_generation_works_with_valid_genome() {
        let mut state = test_state("import");
        let genome = vec![0.5_f32; GENOME_SIZE];
        let info = state
            .import_generation(
                "imported".to_string(),
                genome.clone(),
                42,
                10000,
                -30.0,
                Some("abcd1234".to_string()),
                None,
            )
            .unwrap();
        assert_eq!(info.name, "imported");
        assert_eq!(info.generation, 42);
        assert_eq!(info.total_games_trained, 10000);
        assert_eq!(info.lineage_hash, "abcd1234");

        let model = state.get_saved_generation_model("imported").unwrap();
        assert_eq!(model.best_genome, genome);
        cleanup(&state);
    }

    #[test]
    fn import_generation_rejects_wrong_genome_size() {
        let mut state = test_state("import_bad_size");
        let bad_genome = vec![0.0_f32; 10]; // Wrong size
        let result = state.import_generation("bad".to_string(), bad_genome, 0, 0, 0.0, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid genome size"));
        cleanup(&state);
    }

    #[test]
    fn import_generation_rejects_duplicate_name() {
        let mut state = test_state("import_dup");
        let genome = vec![0.5_f32; GENOME_SIZE];
        state
            .import_generation("dup".to_string(), genome.clone(), 1, 100, -20.0, None, None)
            .unwrap();
        let result = state.import_generation("dup".to_string(), genome, 2, 200, -10.0, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
        cleanup(&state);
    }

    #[test]
    fn get_saved_genome_returns_genome_and_games() {
        let mut state = test_state("get_saved_genome");
        let genome = vec![0.3_f32; GENOME_SIZE];
        state
            .import_generation(
                "sg_test".to_string(),
                genome.clone(),
                5,
                5000,
                -25.0,
                None,
                None,
            )
            .unwrap();

        let (g, games) = state.get_saved_genome("sg_test").unwrap();
        assert_eq!(g, genome);
        assert_eq!(games, 5000);

        assert!(state.get_saved_genome("nonexistent").is_none());
        cleanup(&state);
    }

    // ---- GA Operators (accessible from within the module) ----

    #[test]
    fn crossover_produces_genome_of_correct_size() {
        let mut rng = StdRng::seed_from_u64(42);
        let parent_a: Vec<f32> = (0..GENOME_SIZE).map(|i| i as f32).collect();
        let parent_b: Vec<f32> = (0..GENOME_SIZE).map(|i| -(i as f32)).collect();
        let child = crossover(&parent_a, &parent_b, &mut rng);
        assert_eq!(child.len(), GENOME_SIZE);
        // BLX-α: each gene should be within the expanded interval [min - α*d, max + α*d]
        for (i, &gene) in child.iter().enumerate() {
            let a = parent_a[i];
            let b = parent_b[i];
            let lo = a.min(b);
            let hi = a.max(b);
            let d = hi - lo;
            let expanded_lo = lo - 0.5 * d;
            let expanded_hi = hi + 0.5 * d;
            assert!(
                gene >= expanded_lo && gene <= expanded_hi,
                "Gene {i} should be within BLX-α range [{expanded_lo}, {expanded_hi}], got {gene}"
            );
        }
    }

    #[test]
    fn mutate_does_not_change_genome_size() {
        let mut rng = StdRng::seed_from_u64(99);
        let mut genome = vec![0.0_f32; GENOME_SIZE];
        let original = genome.clone();
        mutate(
            &mut genome,
            &mut rng,
            BASE_MUTATION_RATE,
            BASE_MUTATION_SIGMA,
            BASE_RESET_RATE,
        );
        assert_eq!(genome.len(), GENOME_SIZE);
        // At least some genes should have mutated (statistically near-certain)
        let changed = genome
            .iter()
            .zip(original.iter())
            .filter(|(a, b)| a != b)
            .count();
        // With BASE_MUTATION_RATE=0.05 + BASE_RESET_RATE=0.005, about 5.5% should change
        assert!(changed > 0, "Some genes should have been mutated");
    }

    #[test]
    fn tournament_select_returns_genome_from_population() {
        let mut rng = StdRng::seed_from_u64(123);
        let population: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; GENOME_SIZE]).collect();
        let fitnesses: Vec<f64> = (0..10).map(|i| i as f64).collect();

        let selected = tournament_select(&population, &fitnesses, &mut rng);
        assert_eq!(selected.len(), GENOME_SIZE);
        // The selected genome should be one of the population members
        assert!(
            population.contains(&selected),
            "Selected genome must come from the population"
        );
    }

    #[test]
    fn compute_lineage_hash_is_deterministic() {
        let genome = vec![1.0_f32; GENOME_SIZE];
        let hash1 = compute_lineage_hash(&genome);
        let hash2 = compute_lineage_hash(&genome);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 8); // 8 hex chars
    }

    #[test]
    fn compute_lineage_hash_differs_for_different_genomes() {
        let genome_a = vec![1.0_f32; GENOME_SIZE];
        let genome_b = vec![2.0_f32; GENOME_SIZE];
        let hash_a = compute_lineage_hash(&genome_a);
        let hash_b = compute_lineage_hash(&genome_b);
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn random_genome_has_correct_size() {
        let mut rng = StdRng::seed_from_u64(0);
        let genome = random_genome(&mut rng);
        assert_eq!(genome.len(), GENOME_SIZE);
        // Values should be in [-1, 1]
        for &val in &genome {
            assert!(
                (-1.0..=1.0).contains(&val),
                "Genome values should be in [-1, 1], got {val}"
            );
        }
    }

    // ---- Load/Save to Disk ----

    #[test]
    fn load_or_new_with_nonexistent_path_creates_new_state() {
        let dir = std::env::temp_dir().join(format!("skyjo_test_load_new_{}", std::process::id()));
        let model_path = dir.join("nonexistent_subdir").join("model.json");
        let state = GeneticTrainingState::load_or_new(model_path);
        assert_eq!(state.population.len(), POPULATION_SIZE);
        assert_eq!(state.generation, 0);
        cleanup(&state);
    }

    #[test]
    fn save_and_load_round_trips_correctly() {
        let mut state = test_state("round_trip");
        advance_to_gen1(&mut state);
        state
            .save_generation(Some("round_trip_save".to_string()))
            .unwrap();
        let original_genome = state.best_genome.clone();
        let original_gen = state.generation;

        // Load the saved generation
        state.load_saved("round_trip_save").unwrap();
        assert_eq!(state.best_genome, original_genome);
        assert_eq!(state.generation, original_gen);
        assert_eq!(state.population.len(), POPULATION_SIZE);
        // First individual should be the saved genome
        assert_eq!(state.population[0], original_genome);
        cleanup(&state);
    }

    #[test]
    fn load_or_new_loads_existing_model_from_disk() {
        let mut state = test_state("load_existing");
        advance_to_gen1(&mut state);
        state.best_genome = vec![0.42_f32; GENOME_SIZE];
        save_model(&state);
        let model_path = state.model_path.clone();

        // Load from the same path
        let loaded = GeneticTrainingState::load_or_new(model_path);
        assert_eq!(loaded.generation, 1);
        assert_eq!(loaded.best_genome, vec![0.42_f32; GENOME_SIZE]);
        assert_eq!(loaded.population.len(), POPULATION_SIZE);
        cleanup(&state);
    }

    #[test]
    fn save_generation_at_gen0_fails() {
        let mut state = test_state("gen0_fail");
        let result = state.save_generation(Some("fail".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("generation 0"));
        cleanup(&state);
    }

    #[test]
    fn load_saved_nonexistent_fails() {
        let mut state = test_state("load_nonexist");
        let result = state.load_saved("ghost");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No saved generation"));
        cleanup(&state);
    }

    #[test]
    fn save_generation_default_name() {
        let mut state = test_state("default_name");
        advance_to_gen1(&mut state);
        let info = state.save_generation(None).unwrap();
        assert_eq!(info.name, "Gen 1");
        cleanup(&state);
    }
}
