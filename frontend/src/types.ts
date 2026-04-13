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
  displaced_card: CardValue | null;
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

// Interactive game types (for Play mode)

export type VisibleSlot =
  | "Hidden"
  | { Revealed: CardValue }
  | "Cleared";

export type ActionNeeded =
  | { type: 'ChooseInitialFlips'; player: number; count: number }
  | { type: 'ChooseDraw'; player: number; drawable_piles: number[] }
  | { type: 'ChooseDeckDrawAction'; player: number; drawn_card: CardValue }
  | { type: 'ChooseDiscardDrawPlacement'; player: number; drawn_card: CardValue }
  | { type: 'RoundOver'; round_number: number; round_scores: number[]; raw_round_scores: number[]; cumulative_scores: number[]; going_out_player: number | null; end_of_round_clears: ColumnClearEvent[] }
  | { type: 'GameOver'; final_scores: number[]; winners: number[]; round_number: number; round_scores: number[]; raw_round_scores: number[]; going_out_player: number | null; end_of_round_clears: ColumnClearEvent[] };

export type PlayerAction =
  | { type: 'InitialFlip'; position: number }
  | { type: 'DrawFromDeck' }
  | { type: 'DrawFromDiscard'; pile_index: number }
  | { type: 'UndoDrawFromDiscard' }
  | { type: 'KeepDeckDraw'; position: number }
  | { type: 'DiscardAndFlip'; position: number }
  | { type: 'PlaceDiscardDraw'; position: number }
  | { type: 'ContinueToNextRound' };

export interface InteractiveGameState {
  num_players: number;
  player_names: string[];
  num_rows: number;
  num_cols: number;
  round_number: number;
  current_player: number;
  action_needed: ActionNeeded;
  boards: VisibleSlot[][];
  discard_tops: (CardValue | null)[];
  discard_sizes: number[];
  deck_remaining: number;
  cumulative_scores: number[];
  going_out_player: number | null;
  is_final_turn: boolean;
  last_column_clears: ColumnClearEvent[];
}

export interface PlayConfig {
  num_players: number;
  player_names: string[];
  player_types: PlayerType[];
  rules: string;
  seed: number;
}

/** Player type: "Human" or "Bot:<StrategyName>" (e.g. "Bot:Random", "Bot:Greedy") */
export type PlayerType = 'Human' | `Bot:${string}`;

export type BotSpeed = 'slow' | 'normal' | 'fast' | 'instant';

export const BOT_SPEED_MS: Record<BotSpeed, number> = {
  slow: 1500,
  normal: 600,
  fast: 150,
  instant: 0,
};

export const BOT_SPEED_LABELS: Record<BotSpeed, string> = {
  slow: 'Slow',
  normal: 'Normal',
  fast: 'Fast',
  instant: 'Instant',
};

export interface BotActionResponse {
  action: PlayerAction;
  state: InteractiveGameState;
}
