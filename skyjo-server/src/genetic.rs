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
    ClearerStrategy, DefensiveStrategy, GeneticStrategy, GreedyStrategy, RandomStrategy,
    StatisticianStrategy, GENOME_SIZE, HIDDEN_SIZE, INPUT_GROUPS, INPUT_LABELS, INPUT_SIZE,
    OUTPUT_GROUPS, OUTPUT_LABELS, OUTPUT_SIZE,
};

// --- Configuration constants ---

pub const POPULATION_SIZE: usize = 50;
pub const GAMES_PER_INDIVIDUAL: usize = 10;
pub const TOURNAMENT_SIZE: usize = 3;
pub const MUTATION_RATE: f64 = 0.05;
pub const MUTATION_SIGMA: f32 = 0.3;
pub const RESET_RATE: f64 = 0.005;
pub const ELITISM_COUNT: usize = 2;
pub const NUM_OPPONENTS: usize = 3; // opponents per game (4-player games)

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
        }
    }
}

/// Persistent model data, serialized to/from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneticModelData {
    pub best_genome: Vec<f32>,
    pub input_size: usize,
    pub hidden_size: usize,
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
}

impl GeneticModelData {
    fn from_state(state: &GeneticTrainingState) -> Self {
        Self {
            best_genome: state.best_genome.clone(),
            input_size: INPUT_SIZE,
            hidden_size: HIDDEN_SIZE,
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
        }
    }

    /// Load from a saved model file, or create a new random state.
    pub fn load_or_new(model_path: PathBuf) -> Self {
        if model_path.exists() {
            match std::fs::read_to_string(&model_path) {
                Ok(json) => match serde_json::from_str::<GeneticModelData>(&json) {
                    Ok(data) => {
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
                            mutate(&mut child, &mut rng);
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
            best_fitness: if self.best_fitness.is_finite() { self.best_fitness } else { 0.0 },
            training_start_generation: self.training_start_generation,
            training_target_generation: self.training_target_generation,
            training_elapsed_ms: elapsed_ms,
            training_last_gen_elapsed_ms: self.training_last_gen_elapsed_ms,
            training_mode: self.training_mode.clone(),
            training_target_fitness: self.training_target_fitness,
            training_start_fitness: self.training_start_fitness,
            lineage_hash: self.lineage_hash.clone(),
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
            best_fitness: if self.best_fitness.is_finite() { self.best_fitness } else { 0.0 },
            genome: self.best_genome.clone(),
            saved_at: chrono_now(),
            lineage_hash: self.lineage_hash.clone(),
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
        self.saved_generations.iter().map(SavedGenerationInfo::from).collect()
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
        })
    }

    /// Import an external genome as a saved generation.
    pub fn import_generation(
        &mut self,
        name: String,
        genome: Vec<f32>,
        generation: usize,
        total_games_trained: usize,
        best_fitness: f64,
        lineage_hash: Option<String>,
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
            mutate(&mut child, &mut rng);
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
fn tournament_select(
    population: &[Vec<f32>],
    fitnesses: &[f64],
    rng: &mut impl Rng,
) -> Vec<f32> {
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

/// Uniform crossover: each gene randomly picked from parent_a or parent_b.
fn crossover(parent_a: &[f32], parent_b: &[f32], rng: &mut impl Rng) -> Vec<f32> {
    parent_a
        .iter()
        .zip(parent_b.iter())
        .map(|(&a, &b)| if rng.random_bool(0.5) { a } else { b })
        .collect()
}

/// Mutate a genome in place with Gaussian perturbation and occasional resets.
fn mutate(genome: &mut [f32], rng: &mut impl Rng) {
    for gene in genome.iter_mut() {
        let r: f64 = rng.random();
        if r < RESET_RATE {
            *gene = rng.random_range(-1.0f32..1.0);
        } else if r < RESET_RATE + MUTATION_RATE {
            // Box-Muller approximation for normal distribution
            let u1: f32 = rng.random_range(0.0001f32..1.0);
            let u2: f32 = rng.random_range(0.0f32..std::f32::consts::TAU);
            let normal = (-2.0 * u1.ln()).sqrt() * u2.cos();
            *gene += normal * MUTATION_SIGMA;
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
        Box::new(GeneticStrategy::new(
            population[idx].clone(),
            games_trained,
        ))
    } else if rng.random_bool(0.5) {
        Box::new(GreedyStrategy)
    } else {
        Box::new(RandomStrategy)
    }
}

/// Evaluate fitness for a single individual by playing games.
/// Returns negative average cumulative score (higher = better).
fn evaluate_individual(
    genome: &[f32],
    individual_idx: usize,
    population: &[Vec<f32>],
    base_seed: u64,
    games_trained: usize,
) -> f64 {
    let mut total_cum_score: i64 = 0;
    let mut rng = StdRng::seed_from_u64(base_seed);

    for game_idx in 0..GAMES_PER_INDIVIDUAL {
        let seed = base_seed.wrapping_add(game_idx as u64);

        // Build strategies: individual at position 0, opponents at 1..
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
                    let my_score = history.final_scores[0] as i64;
                    let _min_other = history.final_scores[1..]
                        .iter()
                        .copied()
                        .min()
                        .unwrap_or(i32::MAX) as i64;
                    let is_winner = history.winners.contains(&0);

                    // Penalty: if not cumulative winner (ties OK), double the score
                    let penalized = if !is_winner && my_score > 0 {
                        my_score * 2
                    } else {
                        my_score
                    };
                    total_cum_score += penalized;
                }
                Err(e) => {
                    tracing::warn!("Game play error during training: {e}");
                    total_cum_score += 200; // Penalty for broken games
                }
            },
            Err(e) => {
                tracing::warn!("Game creation error during training: {e}");
                total_cum_score += 200;
            }
        }
    }

    // Fitness = negative average score (lower score = higher fitness)
    -(total_cum_score as f64 / GAMES_PER_INDIVIDUAL as f64)
}

/// Run one generation of the genetic algorithm.
/// Returns (new_population, fitnesses, best_idx, games_played).
fn run_generation(
    population: &[Vec<f32>],
    generation_seed: u64,
    games_trained: usize,
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
        mutate(&mut child, &mut rng);
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
pub async fn train_generations(
    state: Arc<Mutex<GeneticTrainingState>>,
    num_generations: usize,
) {
    for generation_i in 0..num_generations {
        // Clone population from state (brief lock)
        let (population, generation_num, games_trained, target_fitness, mode) = {
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
            )
        };

        // Run the CPU-intensive evaluation without holding the lock
        let generation_seed = (generation_num as u64).wrapping_mul(7919) ^ 0xDEADBEEF;
        let (new_population, fitnesses, best_idx, games_played) =
            tokio::task::spawn_blocking(move || {
                run_generation(&population, generation_seed, games_trained)
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
    };
    state.saved_generations.push(saved);
    save_model(state);
    tracing::info!("Auto-saved training result: {name}");
}
