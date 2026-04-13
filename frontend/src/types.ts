export type CardValue = number;

export type Slot =
  | { Hidden: CardValue }
  | { Revealed: CardValue }
  | "Cleared";

export type DeckDrawAction =
  | { Keep: number }
  | { DiscardAndFlip: number };

export interface ColumnClearEvent {
  player_index: number;
  column: number;
  card_value: CardValue;
}

export type TurnAction =
  | {
      DrewFromDeck: {
        drawn_card: CardValue;
        action: DeckDrawAction;
        displaced_card: CardValue | null;
      };
    }
  | {
      DrewFromDiscard: {
        pile_index: number;
        drawn_card: CardValue;
        placement: number;
        displaced_card: CardValue;
      };
    };

export interface TurnRecord {
  player_index: number;
  action: TurnAction;
  column_clears: ColumnClearEvent[];
  went_out: boolean;
}

export interface RoundHistory {
  round_number: number;
  initial_deck_order: CardValue[];
  dealt_hands: CardValue[][];
  setup_flips: number[][];
  starting_player: number;
  turns: TurnRecord[];
  going_out_player: number | null;
  end_of_round_clears: ColumnClearEvent[];
  round_scores: number[];
  cumulative_scores: number[];
  truncated: boolean;
}

export interface GameHistory {
  seed: number;
  num_players: number;
  strategy_names: string[];
  rules_name: string;
  rounds: RoundHistory[];
  final_scores: number[];
  winners: number[];
}

export interface GameStats {
  winners: number[];
  final_scores: number[];
  num_rounds: number;
  total_turns: number;
}

export interface AggregateStats {
  num_games: number;
  num_players: number;
  wins_per_player: number[];
  win_rate_per_player: number[];
  avg_score_per_player: number[];
  min_score_per_player: number[];
  max_score_per_player: number[];
  avg_rounds_per_game: number;
  avg_turns_per_game: number;
  score_distributions: number[][];
}

export interface SimWithHistories {
  stats: AggregateStats;
  histories: GameHistory[];
}

// Cache types

export interface CacheEntry {
  version: 1;
  key: string;
  config: SimConfig;
  stats: ProgressStats;
  gamesCompleted: number;
  totalGames: number;
  elapsedMs: number;
  hasHistories: boolean;
  savedAt: number;
}

export interface CacheExportFile {
  format: 'skyjo-sim-cache';
  version: 1;
  config: SimConfig;
  stats: ProgressStats;
  gamesCompleted: number;
  totalGames: number;
  elapsedMs: number;
  histories: GameHistory[] | null;
  exportedAt: number;
}

// Worker message types

export interface SimConfig {
  num_games: number;
  seed: number;
  strategies: string[];
  rules: string;
  withHistories: boolean;
  realtimeVisualization: boolean;
  maxTurnsPerRound: number;
}

export type WorkerRequest =
  | { type: 'start'; config: SimConfig }
  | { type: 'pause' }
  | { type: 'resume' }
  | { type: 'stop' }
  | { type: 'requestRealtimeGame' };

export interface ProgressStats {
  num_games: number;
  num_players: number;
  wins_per_player: number[];
  win_rate_per_player: number[];
  avg_score_per_player: number[];
  min_score_per_player: number[];
  max_score_per_player: number[];
  avg_rounds_per_game: number;
  avg_turns_per_game: number;
}

export type WorkerResponse =
  | { type: 'ready' }
  | { type: 'progress'; stats: ProgressStats; gamesCompleted: number; totalGames: number; elapsedMs: number }
  | { type: 'complete'; stats: ProgressStats; gamesCompleted: number; totalGames: number; elapsedMs: number; histories: GameHistory[] | null }
  | { type: 'realtimeGame'; history: GameHistory }
  | { type: 'error'; message: string };
