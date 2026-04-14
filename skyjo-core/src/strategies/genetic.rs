use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::card::{CardValue, VisibleSlot};
use crate::strategy::{
    Complexity, ConceptReference, DecisionLogic, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, Strategy, StrategyDescription, StrategyView,
};

use super::common::{average_unknown_value, column_analysis, expected_score};

// --- Architecture constants ---

/// Architecture version for model compatibility checking.
/// v1 = original (48→32→39 tanh), v2 = improved (INPUT_SIZE→64→32→39 ReLU)
pub const ARCHITECTURE_VERSION: u32 = 2;

pub const INPUT_SIZE: usize = 62; // expanded features
pub const HIDDEN1_SIZE: usize = 64;
pub const HIDDEN2_SIZE: usize = 32;
pub const OUTPUT_SIZE: usize = 39;
/// Total number of f32 weights in the genome.
/// Layout: [W_ih1, b_h1, W_h1h2, b_h2, W_h2o, b_o]
pub const GENOME_SIZE: usize = INPUT_SIZE * HIDDEN1_SIZE + HIDDEN1_SIZE +     // input → hidden1
    HIDDEN1_SIZE * HIDDEN2_SIZE + HIDDEN2_SIZE +   // hidden1 → hidden2
    HIDDEN2_SIZE * OUTPUT_SIZE + OUTPUT_SIZE; // hidden2 → output

/// Deprecated alias for backward compatibility.
pub const HIDDEN_SIZE: usize = HIDDEN1_SIZE;

// Output slice ranges
const FLIP_START: usize = 0;
const DRAW_START: usize = 12;
const DECK_PLACE_START: usize = 14;
const DECK_KEEP_VS_DISCARD: usize = 26;
const DISCARD_PLACE_START: usize = 27;

/// Labels for each input feature, used by the frontend NN visualization.
pub const INPUT_LABELS: &[&str] = &[
    // Board slots 0-11: 3 features each (is_hidden, is_revealed, value/14)
    "Slot 0 Hidden",
    "Slot 0 Revealed",
    "Slot 0 Value",
    "Slot 1 Hidden",
    "Slot 1 Revealed",
    "Slot 1 Value",
    "Slot 2 Hidden",
    "Slot 2 Revealed",
    "Slot 2 Value",
    "Slot 3 Hidden",
    "Slot 3 Revealed",
    "Slot 3 Value",
    "Slot 4 Hidden",
    "Slot 4 Revealed",
    "Slot 4 Value",
    "Slot 5 Hidden",
    "Slot 5 Revealed",
    "Slot 5 Value",
    "Slot 6 Hidden",
    "Slot 6 Revealed",
    "Slot 6 Value",
    "Slot 7 Hidden",
    "Slot 7 Revealed",
    "Slot 7 Value",
    "Slot 8 Hidden",
    "Slot 8 Revealed",
    "Slot 8 Value",
    "Slot 9 Hidden",
    "Slot 9 Revealed",
    "Slot 9 Value",
    "Slot 10 Hidden",
    "Slot 10 Revealed",
    "Slot 10 Value",
    "Slot 11 Hidden",
    "Slot 11 Revealed",
    "Slot 11 Value",
    // Global features
    "Discard Top",
    "Deck Remaining",
    "Hidden Count",
    "Expected Score",
    "Best Opp Score",
    "Score Gap",
    "Final Turn",
    "Drawn Card",
    // Column match potential
    "Col 0 Match",
    "Col 1 Match",
    "Col 2 Match",
    "Col 3 Match",
    // Opponent hidden counts (7 max opponents)
    "Opp 0 Hidden Count",
    "Opp 1 Hidden Count",
    "Opp 2 Hidden Count",
    "Opp 3 Hidden Count",
    "Opp 4 Hidden Count",
    "Opp 5 Hidden Count",
    "Opp 6 Hidden Count",
    // Game state features
    "Discard Pile Depth",
    "Score Rank",
    "Opp 0 Near Done",
    "Opp 1 Near Done",
    "Opp 2 Near Done",
    "Opp 3 Near Done",
    "Opp 4 Near Done",
];

/// Labels for each output, used by the frontend NN visualization.
pub const OUTPUT_LABELS: &[&str] = &[
    // Initial flip scores (0-11)
    "Flip 0",
    "Flip 1",
    "Flip 2",
    "Flip 3",
    "Flip 4",
    "Flip 5",
    "Flip 6",
    "Flip 7",
    "Flip 8",
    "Flip 9",
    "Flip 10",
    "Flip 11",
    // Draw choice (12-13)
    "Draw Deck",
    "Draw Discard",
    // Deck draw placement scores (14-25)
    "Place 0",
    "Place 1",
    "Place 2",
    "Place 3",
    "Place 4",
    "Place 5",
    "Place 6",
    "Place 7",
    "Place 8",
    "Place 9",
    "Place 10",
    "Place 11",
    // Keep vs discard threshold (26)
    "Keep vs Discard",
    // Discard draw placement scores (27-38)
    "Discard Place 0",
    "Discard Place 1",
    "Discard Place 2",
    "Discard Place 3",
    "Discard Place 4",
    "Discard Place 5",
    "Discard Place 6",
    "Discard Place 7",
    "Discard Place 8",
    "Discard Place 9",
    "Discard Place 10",
    "Discard Place 11",
];

/// Grouped input labels for the frontend visualization (collapsed view).
pub const INPUT_GROUPS: &[(&str, usize, usize)] = &[
    ("Board Slots (12x3)", 0, 36),
    ("Discard Top", 36, 37),
    ("Deck Remaining", 37, 38),
    ("Hidden Count", 38, 39),
    ("Expected Score", 39, 40),
    ("Best Opp Score", 40, 41),
    ("Score Gap", 41, 42),
    ("Final Turn", 42, 43),
    ("Drawn Card", 43, 44),
    ("Column Matches (4)", 44, 48),
    ("Opp Hidden Counts (7)", 48, 55),
    ("Discard Pile Depth", 55, 56),
    ("Score Rank", 56, 57),
    ("Opp Near Done (5)", 57, 62),
];

/// Grouped output labels for the frontend visualization (collapsed view).
pub const OUTPUT_GROUPS: &[(&str, usize, usize)] = &[
    ("Initial Flips (12)", 0, 12),
    ("Draw Choice (2)", 12, 14),
    ("Deck Placement (12)", 14, 26),
    ("Keep vs Discard (1)", 26, 27),
    ("Discard Placement (12)", 27, 39),
];

// --- Neural Network ---

/// A feedforward neural network with two hidden layers and ReLU activation.
/// Genome layout: [W_ih1, b_h1, W_h1h2, b_h2, W_h2o, b_o]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralNetwork {
    pub genome: Vec<f32>,
}

impl NeuralNetwork {
    /// Create a new NN from a flat genome vector.
    /// Panics if genome length != GENOME_SIZE.
    pub fn from_genome(genome: Vec<f32>) -> Self {
        assert_eq!(
            genome.len(),
            GENOME_SIZE,
            "Genome must have exactly {GENOME_SIZE} weights, got {}",
            genome.len()
        );
        Self { genome }
    }

    /// Create a random NN using He initialization for ReLU layers.
    pub fn random(rng: &mut dyn RngCore) -> Self {
        let mut genome = Vec::with_capacity(GENOME_SIZE);

        // He initialization scale = sqrt(2 / fan_in)
        let scale_h1 = (2.0 / INPUT_SIZE as f32).sqrt();
        let scale_h2 = (2.0 / HIDDEN1_SIZE as f32).sqrt();
        let scale_out = (2.0 / HIDDEN2_SIZE as f32).sqrt();

        // W_ih1: INPUT_SIZE * HIDDEN1_SIZE weights
        for _ in 0..(INPUT_SIZE * HIDDEN1_SIZE) {
            genome.push(he_init_weight(rng, scale_h1));
        }
        // b_h1: HIDDEN1_SIZE biases (zero init)
        genome.extend(std::iter::repeat_n(0.0, HIDDEN1_SIZE));
        // W_h1h2: HIDDEN1_SIZE * HIDDEN2_SIZE weights
        for _ in 0..(HIDDEN1_SIZE * HIDDEN2_SIZE) {
            genome.push(he_init_weight(rng, scale_h2));
        }
        // b_h2: HIDDEN2_SIZE biases (zero init)
        genome.extend(std::iter::repeat_n(0.0, HIDDEN2_SIZE));
        // W_h2o: HIDDEN2_SIZE * OUTPUT_SIZE weights
        for _ in 0..(HIDDEN2_SIZE * OUTPUT_SIZE) {
            genome.push(he_init_weight(rng, scale_out));
        }
        // b_o: OUTPUT_SIZE biases (zero init)
        genome.extend(std::iter::repeat_n(0.0, OUTPUT_SIZE));

        Self { genome }
    }

    /// Run a forward pass through the network.
    pub fn forward(&self, inputs: &[f32]) -> Vec<f32> {
        assert_eq!(inputs.len(), INPUT_SIZE);

        let g = &self.genome;
        // Layer boundary offsets
        let w1_end = INPUT_SIZE * HIDDEN1_SIZE;
        let b1_end = w1_end + HIDDEN1_SIZE;
        let w2_end = b1_end + HIDDEN1_SIZE * HIDDEN2_SIZE;
        let b2_end = w2_end + HIDDEN2_SIZE;
        let w3_end = b2_end + HIDDEN2_SIZE * OUTPUT_SIZE;
        // b3 starts at w3_end

        // Hidden layer 1: h1 = ReLU(W1 * x + b1)
        let mut hidden1 = Vec::with_capacity(HIDDEN1_SIZE);
        for j in 0..HIDDEN1_SIZE {
            let mut sum = g[w1_end + j]; // bias
            for i in 0..INPUT_SIZE {
                sum += g[j * INPUT_SIZE + i] * inputs[i];
            }
            hidden1.push(sum.max(0.0)); // ReLU
        }

        // Hidden layer 2: h2 = ReLU(W2 * h1 + b2)
        let mut hidden2 = Vec::with_capacity(HIDDEN2_SIZE);
        for k in 0..HIDDEN2_SIZE {
            let mut sum = g[w2_end + k]; // bias
            for j in 0..HIDDEN1_SIZE {
                sum += g[b1_end + k * HIDDEN1_SIZE + j] * hidden1[j];
            }
            hidden2.push(sum.max(0.0)); // ReLU
        }

        // Output layer: o = W3 * h2 + b3 (no activation — raw scores)
        let mut output = Vec::with_capacity(OUTPUT_SIZE);
        for m in 0..OUTPUT_SIZE {
            let mut sum = g[w3_end + m]; // bias
            for k in 0..HIDDEN2_SIZE {
                sum += g[b2_end + m * HIDDEN2_SIZE + k] * hidden2[k];
            }
            output.push(sum);
        }

        output
    }
}

/// Generate a weight using He initialization (normal distribution with given scale).
fn he_init_weight(rng: &mut dyn RngCore, scale: f32) -> f32 {
    // Box-Muller transform for normal distribution
    let u1 = (rng.next_u32() as f32 / u32::MAX as f32).max(0.0001);
    let u2 = rng.next_u32() as f32 / u32::MAX as f32 * std::f32::consts::TAU;
    let normal = (-2.0 * u1.ln()).sqrt() * u2.cos();
    normal * scale
}

// --- Feature extraction ---

/// Extract a fixed-size feature vector from a StrategyView.
/// `drawn_card` is Some when the bot has drawn a card and needs to decide what to do with it.
pub fn extract_features(view: &StrategyView, drawn_card: Option<CardValue>) -> Vec<f32> {
    let mut features = Vec::with_capacity(INPUT_SIZE);

    // Board slots (12 * 3 = 36 features)
    let num_slots = view.num_rows * view.num_cols;
    for i in 0..num_slots {
        match &view.my_board[i] {
            VisibleSlot::Hidden => {
                features.push(1.0); // is_hidden
                features.push(0.0); // is_revealed
                features.push(0.0); // value
            }
            VisibleSlot::Revealed(v) => {
                features.push(0.0);
                features.push(1.0);
                features.push(*v as f32 / 14.0);
            }
            VisibleSlot::Cleared => {
                features.push(0.0);
                features.push(0.0);
                features.push(0.0);
            }
        }
    }
    // Pad if board is smaller than 12 slots (shouldn't happen with standard rules)
    while features.len() < 36 {
        features.push(0.0);
    }

    // Discard pile top
    let discard_top = view.discard_top(0).map(|v| v as f32 / 14.0).unwrap_or(0.0);
    features.push(discard_top);

    // Deck remaining (normalized)
    features.push(view.deck_remaining as f32 / 150.0);

    // Own hidden count
    let hidden_count = view
        .my_board
        .iter()
        .filter(|s| matches!(s, VisibleSlot::Hidden))
        .count();
    features.push(hidden_count as f32 / 12.0);

    // Expected scores
    let avg_unknown = average_unknown_value(view);
    let my_expected = expected_score(&view.my_board, avg_unknown);
    features.push((my_expected / 100.0) as f32);

    // Best opponent expected score
    let best_opp = view
        .opponent_boards
        .iter()
        .map(|b| expected_score(b, avg_unknown))
        .fold(f64::MAX, f64::min);
    let best_opp = if best_opp == f64::MAX { 0.0 } else { best_opp };
    features.push((best_opp / 100.0) as f32);

    // Cumulative score gap
    let my_cum = view
        .cumulative_scores
        .get(view.my_index)
        .copied()
        .unwrap_or(0);
    let best_opp_cum = view
        .cumulative_scores
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != view.my_index)
        .map(|(_, &s)| s)
        .min()
        .unwrap_or(0);
    features.push((my_cum - best_opp_cum) as f32 / 100.0);

    // Is final turn
    features.push(if view.is_final_turn { 1.0 } else { 0.0 });

    // Drawn card value
    features.push(drawn_card.map(|v| v as f32 / 14.0).unwrap_or(0.0));

    // Column match potential (4 columns)
    let cols = column_analysis(view);
    for col in 0..4 {
        if let Some(info) = cols.get(col) {
            let match_count = info
                .partial_match
                .map(|(_, count)| count)
                .unwrap_or_else(|| {
                    if info.revealed_values.len() == 1 {
                        1
                    } else {
                        0
                    }
                });
            features.push(match_count as f32 / 3.0);
        } else {
            features.push(0.0);
        }
    }

    // Opponent hidden counts (7 slots, padded with 0 for fewer opponents)
    for opp_idx in 0..7 {
        if let Some(board) = view.opponent_boards.get(opp_idx) {
            let hidden = board
                .iter()
                .filter(|s| matches!(s, VisibleSlot::Hidden))
                .count();
            features.push(hidden as f32 / 12.0);
        } else {
            features.push(0.0);
        }
    }

    // Discard pile depth (normalized)
    let discard_depth: usize = view.discard_piles.iter().map(|p| p.len()).sum();
    features.push(discard_depth as f32 / 150.0);

    // Score rank (0.0 = best, 1.0 = worst among players)
    let my_cum_score = view
        .cumulative_scores
        .get(view.my_index)
        .copied()
        .unwrap_or(0);
    let num_players = view.cumulative_scores.len();
    let rank = view
        .cumulative_scores
        .iter()
        .filter(|&&s| s < my_cum_score)
        .count();
    features.push(if num_players > 1 {
        rank as f32 / (num_players - 1) as f32
    } else {
        0.0
    });

    // Opponent "near done" signals (5 closest opponents, ratio of revealed cards)
    let mut opp_revealed_ratios: Vec<f32> = view
        .opponent_boards
        .iter()
        .map(|board| {
            let total = board.len();
            let revealed = board
                .iter()
                .filter(|s| !matches!(s, VisibleSlot::Hidden))
                .count();
            if total > 0 {
                revealed as f32 / total as f32
            } else {
                0.0
            }
        })
        .collect();
    opp_revealed_ratios.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    for i in 0..5 {
        features.push(opp_revealed_ratios.get(i).copied().unwrap_or(0.0));
    }

    assert_eq!(features.len(), INPUT_SIZE);
    features
}

// --- GeneticStrategy ---

/// A strategy powered by a neural network whose weights are evolved via genetic algorithm.
pub struct GeneticStrategy {
    nn: NeuralNetwork,
    games_trained: usize,
}

impl GeneticStrategy {
    pub fn new(genome: Vec<f32>, games_trained: usize) -> Self {
        Self {
            nn: NeuralNetwork::from_genome(genome),
            games_trained,
        }
    }

    /// Helper: pick the index with the highest output score among valid positions.
    fn best_valid_position(&self, outputs: &[f32], start: usize, valid: &[usize]) -> usize {
        valid
            .iter()
            .copied()
            .max_by(|&a, &b| {
                outputs[start + a]
                    .partial_cmp(&outputs[start + b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0)
    }

    /// Helper: collect hidden position indices from the board.
    fn hidden_positions(view: &StrategyView) -> Vec<usize> {
        view.my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, VisibleSlot::Hidden))
            .map(|(i, _)| i)
            .collect()
    }

    /// Helper: collect non-cleared position indices.
    fn non_cleared_positions(view: &StrategyView) -> Vec<usize> {
        view.my_board
            .iter()
            .enumerate()
            .filter(|(_, s)| !matches!(s, VisibleSlot::Cleared))
            .map(|(i, _)| i)
            .collect()
    }
}

impl Strategy for GeneticStrategy {
    fn name(&self) -> &str {
        "Genetic"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: "Genetic".into(),
            summary: format!(
                "A neural network strategy evolved through {} games of neuroevolution. \
                 Uses a two-hidden-layer feedforward network ({INPUT_SIZE} inputs, \
                 {HIDDEN1_SIZE}+{HIDDEN2_SIZE} hidden neurons with ReLU activation, \
                 {OUTPUT_SIZE} outputs) to evaluate board positions and make decisions. \
                 The network's weights are evolved via a genetic algorithm with tournament \
                 selection, crossover, and mutation — no gradient-based training.",
                self.games_trained
            ),
            complexity: Complexity::High,
            strengths: vec![
                "Learns from experience rather than hand-coded rules".into(),
                "Can discover non-obvious strategies through evolution".into(),
                "Improves over time as more games are trained".into(),
            ],
            weaknesses: vec![
                "Quality depends on training — may play poorly before sufficient evolution".into(),
                "Decisions are not human-interpretable (black box)".into(),
                "Requires server connection for training; plays offline with cached weights".into(),
            ],
            phases: vec![
                PhaseDescription {
                    phase: Phase::InitialFlips,
                    label: "Initial Flips".into(),
                    logic: DecisionLogic::Simple {
                        text: "Neural network scores all 12 board positions. \
                               Picks the positions with the highest activation scores \
                               among hidden slots."
                            .into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::ChooseDraw,
                    label: "Draw Decision".into(),
                    logic: DecisionLogic::Simple {
                        text: "Two output neurons compete: 'Draw Deck' vs 'Draw Discard'. \
                               The higher activation wins. The network sees the discard \
                               pile top value and board state as context."
                            .into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::DeckDrawAction,
                    label: "Deck Draw Action".into(),
                    logic: DecisionLogic::Simple {
                        text: "A 'Keep vs Discard' threshold neuron decides: if positive, \
                               keep the drawn card and place it at the position with the \
                               highest placement score. If negative, discard the card and \
                               flip the hidden card with the highest flip score."
                            .into(),
                    },
                },
                PhaseDescription {
                    phase: Phase::DiscardDrawPlacement,
                    label: "Discard Draw Placement".into(),
                    logic: DecisionLogic::Simple {
                        text: "12 output neurons score each board position. \
                               The drawn card is placed at the non-cleared position \
                               with the highest activation."
                            .into(),
                    },
                },
            ],
            concepts: vec![
                ConceptReference {
                    id: "neural_network".into(),
                    label: "Neural Network".into(),
                    used_for: "All decisions — the network transforms board state features \
                               into action scores via learned weights"
                        .into(),
                },
                ConceptReference {
                    id: "expected_score".into(),
                    label: "Expected Score".into(),
                    used_for: "Input feature — provides the network with an estimate of \
                               current board score for both self and opponents"
                        .into(),
                },
                ConceptReference {
                    id: "column_analysis".into(),
                    label: "Column Analysis".into(),
                    used_for: "Input feature — column match potential helps the network \
                               evaluate column-clearing opportunities"
                        .into(),
                },
            ],
        }
    }

    fn choose_initial_flips(
        &self,
        view: &StrategyView,
        count: usize,
        _rng: &mut dyn RngCore,
    ) -> Vec<usize> {
        let features = extract_features(view, None);
        let outputs = self.nn.forward(&features);

        let hidden = Self::hidden_positions(view);
        let mut scored: Vec<(usize, f32)> = hidden
            .into_iter()
            .map(|i| (i, outputs[FLIP_START + i]))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(count).map(|(i, _)| i).collect()
    }

    fn choose_draw(&self, view: &StrategyView, _rng: &mut dyn RngCore) -> DrawChoice {
        let features = extract_features(view, None);
        let outputs = self.nn.forward(&features);

        let deck_score = outputs[DRAW_START];
        let discard_score = outputs[DRAW_START + 1];

        // Only draw from discard if there's a card there
        if discard_score > deck_score && view.discard_top(0).is_some() {
            DrawChoice::DrawFromDiscard(0)
        } else {
            DrawChoice::DrawFromDeck
        }
    }

    fn choose_deck_draw_action(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> DeckDrawAction {
        let features = extract_features(view, Some(drawn_card));
        let outputs = self.nn.forward(&features);

        let keep_threshold = outputs[DECK_KEEP_VS_DISCARD];

        if keep_threshold > 0.0 {
            // Keep: place at best non-cleared position
            let valid = Self::non_cleared_positions(view);
            if valid.is_empty() {
                // Fallback: shouldn't happen, but discard and flip
                let hidden = Self::hidden_positions(view);
                DeckDrawAction::DiscardAndFlip(hidden.first().copied().unwrap_or(0))
            } else {
                let pos = self.best_valid_position(&outputs, DECK_PLACE_START, &valid);
                DeckDrawAction::Keep(pos)
            }
        } else {
            // Discard and flip a hidden card
            let hidden = Self::hidden_positions(view);
            if hidden.is_empty() {
                // All revealed — keep at best non-cleared position instead
                let valid = Self::non_cleared_positions(view);
                let pos = self.best_valid_position(&outputs, DECK_PLACE_START, &valid);
                DeckDrawAction::Keep(pos)
            } else {
                let pos = self.best_valid_position(&outputs, DECK_PLACE_START, &hidden);
                DeckDrawAction::DiscardAndFlip(pos)
            }
        }
    }

    fn choose_discard_draw_placement(
        &self,
        view: &StrategyView,
        drawn_card: CardValue,
        _rng: &mut dyn RngCore,
    ) -> usize {
        let features = extract_features(view, Some(drawn_card));
        let outputs = self.nn.forward(&features);

        let valid = Self::non_cleared_positions(view);
        if valid.is_empty() {
            0 // Shouldn't happen
        } else {
            self.best_valid_position(&outputs, DISCARD_PLACE_START, &valid)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_view() -> StrategyView {
        StrategyView {
            my_index: 0,
            my_board: vec![
                VisibleSlot::Revealed(3),
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Revealed(5),
                VisibleSlot::Hidden,
                VisibleSlot::Revealed(-1),
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
                VisibleSlot::Revealed(8),
                VisibleSlot::Hidden,
                VisibleSlot::Hidden,
            ],
            num_rows: 3,
            num_cols: 4,
            opponent_boards: vec![vec![VisibleSlot::Hidden; 12]],
            opponent_indices: vec![1],
            discard_piles: vec![vec![4]],
            deck_remaining: 120,
            cumulative_scores: vec![0, 10],
            is_final_turn: false,
        }
    }

    #[test]
    fn genome_size_consistent() {
        assert_eq!(
            GENOME_SIZE,
            INPUT_SIZE * HIDDEN1_SIZE
                + HIDDEN1_SIZE
                + HIDDEN1_SIZE * HIDDEN2_SIZE
                + HIDDEN2_SIZE
                + HIDDEN2_SIZE * OUTPUT_SIZE
                + OUTPUT_SIZE
        );
    }

    #[test]
    fn label_counts_match() {
        assert_eq!(INPUT_LABELS.len(), INPUT_SIZE);
        assert_eq!(OUTPUT_LABELS.len(), OUTPUT_SIZE);
    }

    #[test]
    fn group_ranges_cover_all() {
        // Input groups should span 0..INPUT_SIZE
        assert_eq!(INPUT_GROUPS.first().unwrap().1, 0);
        assert_eq!(INPUT_GROUPS.last().unwrap().2, INPUT_SIZE);
        for window in INPUT_GROUPS.windows(2) {
            assert_eq!(window[0].2, window[1].1, "Input groups have a gap");
        }

        // Output groups should span 0..OUTPUT_SIZE
        assert_eq!(OUTPUT_GROUPS.first().unwrap().1, 0);
        assert_eq!(OUTPUT_GROUPS.last().unwrap().2, OUTPUT_SIZE);
        for window in OUTPUT_GROUPS.windows(2) {
            assert_eq!(window[0].2, window[1].1, "Output groups have a gap");
        }
    }

    #[test]
    fn forward_pass_produces_correct_output_size() {
        let mut rng = rand::rng();
        let nn = NeuralNetwork::random(&mut rng);
        let inputs = vec![0.5; INPUT_SIZE];
        let outputs = nn.forward(&inputs);
        assert_eq!(outputs.len(), OUTPUT_SIZE);
    }

    #[test]
    fn forward_pass_deterministic() {
        let mut rng = rand::rng();
        let nn = NeuralNetwork::random(&mut rng);
        let inputs = vec![0.5; INPUT_SIZE];
        let out1 = nn.forward(&inputs);
        let out2 = nn.forward(&inputs);
        assert_eq!(out1, out2);
    }

    #[test]
    fn extract_features_correct_size() {
        let view = make_test_view();
        let features = extract_features(&view, None);
        assert_eq!(features.len(), INPUT_SIZE);

        let features_with_card = extract_features(&view, Some(3));
        assert_eq!(features_with_card.len(), INPUT_SIZE);
    }

    #[test]
    fn strategy_makes_valid_decisions() {
        let mut rng = rand::rng();
        let nn = NeuralNetwork::random(&mut rng);
        let strategy = GeneticStrategy::new(nn.genome, 0);
        let view = make_test_view();

        // Initial flips: should return 2 hidden positions
        let flips = strategy.choose_initial_flips(&view, 2, &mut rng);
        assert_eq!(flips.len(), 2);
        for &pos in &flips {
            assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
        }
        // No duplicates
        assert_ne!(flips[0], flips[1]);

        // Draw choice: should be valid
        let draw = strategy.choose_draw(&view, &mut rng);
        match draw {
            DrawChoice::DrawFromDeck => {}
            DrawChoice::DrawFromDiscard(pile) => {
                assert!(view.discard_top(pile).is_some());
            }
        }

        // Deck draw action with a drawn card
        let action = strategy.choose_deck_draw_action(&view, 5, &mut rng);
        match action {
            DeckDrawAction::Keep(pos) => {
                assert!(!matches!(view.my_board[pos], VisibleSlot::Cleared));
            }
            DeckDrawAction::DiscardAndFlip(pos) => {
                assert!(matches!(view.my_board[pos], VisibleSlot::Hidden));
            }
        }

        // Discard draw placement
        let pos = strategy.choose_discard_draw_placement(&view, 2, &mut rng);
        assert!(!matches!(view.my_board[pos], VisibleSlot::Cleared));
    }

    #[test]
    fn zero_genome_still_works() {
        let genome = vec![0.0; GENOME_SIZE];
        let strategy = GeneticStrategy::new(genome, 0);
        let view = make_test_view();
        let mut rng = rand::rng();

        // All outputs will be 0 (ReLU(0)=0, then 0*0+0=0). Should still pick valid positions.
        let flips = strategy.choose_initial_flips(&view, 2, &mut rng);
        assert_eq!(flips.len(), 2);

        let _draw = strategy.choose_draw(&view, &mut rng);
        let _action = strategy.choose_deck_draw_action(&view, 3, &mut rng);
        let _pos = strategy.choose_discard_draw_placement(&view, 3, &mut rng);
    }
}
