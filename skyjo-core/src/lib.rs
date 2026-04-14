pub mod board;
pub mod card;
pub mod error;
pub mod game;
pub mod history;
pub mod interactive;
pub mod rules;
pub mod simulator;
pub mod strategies;
pub mod strategy;

pub use board::PlayerBoard;
pub use card::{CardValue, Slot, VisibleSlot, standard_deck};
pub use error::{Result, SkyjoError};
pub use game::{DEFAULT_MAX_TURNS_PER_ROUND, Game};
pub use history::GameHistory;
pub use interactive::{ActionNeeded, InteractiveGame, InteractiveGameState, PlayerAction};
pub use rules::{Rules, StandardRules};
pub use simulator::{AggregateStats, GameStats, Simulator, SimulatorConfig};
pub use strategies::common::common_concepts;
pub use strategies::genetic::{
    self as genetic_nn, GENOME_SIZE, HIDDEN_SIZE, INPUT_GROUPS, INPUT_LABELS, INPUT_SIZE,
    NeuralNetwork, OUTPUT_GROUPS, OUTPUT_LABELS, OUTPUT_SIZE,
};
pub use strategies::{
    ClearerStrategy, DefensiveStrategy, GamblerStrategy, GeneticStrategy, GreedyStrategy,
    MimicStrategy, RandomStrategy, RusherStrategy, SaboteurStrategy, StatisticianStrategy,
    SurvivorStrategy,
};
pub use strategy::{
    Complexity, ConceptReference, DecisionLogic, DecisionNode, DeckDrawAction, DrawChoice, Phase,
    PhaseDescription, PriorityRule, Strategy, StrategyDescription, StrategyView,
};

#[cfg(test)]
mod describe_tests {
    use super::*;

    #[test]
    fn all_strategies_describe_and_serialize() {
        let strategies: Vec<Box<dyn Strategy>> = vec![
            Box::new(RandomStrategy),
            Box::new(GreedyStrategy),
            Box::new(DefensiveStrategy),
            Box::new(ClearerStrategy),
            Box::new(StatisticianStrategy),
            Box::new(RusherStrategy),
            Box::new(GamblerStrategy),
            Box::new(SurvivorStrategy),
            Box::new(MimicStrategy),
            Box::new(SaboteurStrategy),
            Box::new(GeneticStrategy::new(vec![0.0; GENOME_SIZE], 0)),
        ];
        for s in &strategies {
            let desc = s.describe();
            assert_eq!(desc.name, s.name());
            assert!(!desc.summary.is_empty());
            assert_eq!(desc.phases.len(), 4, "{} should have 4 phases", desc.name);
            // Verify serialization round-trips
            let json = serde_json::to_string(&desc).unwrap();
            let parsed: StrategyDescription = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.name, desc.name);
        }
    }

    #[test]
    fn common_concepts_serializes() {
        let concepts = common_concepts();
        assert!(concepts.len() >= 5);
        let json = serde_json::to_string(&concepts).unwrap();
        assert!(json.contains("card_counting"));
        assert!(json.contains("average_unknown"));
    }
}
