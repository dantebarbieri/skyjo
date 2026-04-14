import { z } from 'zod';

// ─── Card & Slot Schemas ────────────────────────────────────────────

export const CardValueSchema = z.number().int().min(-2).max(12);

export const SlotSchema = z.union([
  z.object({ Hidden: CardValueSchema }),
  z.object({ Revealed: CardValueSchema }),
  z.literal('Cleared'),
]);

export const VisibleSlotSchema = z.union([
  z.literal('Hidden'),
  z.object({ Revealed: CardValueSchema }),
  z.literal('Cleared'),
]);

// ─── Turn Action Schemas ────────────────────────────────────────────

export const DeckDrawActionSchema = z.union([
  z.object({ Keep: z.number().int().nonnegative() }),
  z.object({ DiscardAndFlip: z.number().int().nonnegative() }),
]);

export const ColumnClearEventSchema = z.object({
  player_index: z.number().int().nonnegative(),
  column: z.number().int().nonnegative(),
  card_value: CardValueSchema,
  displaced_card: CardValueSchema.nullable(),
});

export const TurnActionSchema = z.union([
  z.object({
    DrewFromDeck: z.object({
      drawn_card: CardValueSchema,
      action: DeckDrawActionSchema,
      displaced_card: CardValueSchema.nullable(),
    }),
  }),
  z.object({
    DrewFromDiscard: z.object({
      pile_index: z.number().int().nonnegative(),
      drawn_card: CardValueSchema,
      placement: z.number().int().nonnegative(),
      displaced_card: CardValueSchema,
    }),
  }),
]);

export const TurnRecordSchema = z.object({
  player_index: z.number().int().nonnegative(),
  action: TurnActionSchema,
  column_clears: z.array(ColumnClearEventSchema),
  went_out: z.boolean(),
});

// ─── Game History Schemas ───────────────────────────────────────────

export const RoundHistorySchema = z.object({
  round_number: z.number().int().nonnegative(),
  initial_deck_order: z.array(CardValueSchema),
  dealt_hands: z.array(z.array(CardValueSchema)),
  setup_flips: z.array(z.array(z.number().int().nonnegative())),
  starting_player: z.number().int().nonnegative(),
  turns: z.array(TurnRecordSchema),
  going_out_player: z.number().int().nonnegative().nullable(),
  end_of_round_clears: z.array(ColumnClearEventSchema),
  round_scores: z.array(z.number()),
  cumulative_scores: z.array(z.number()),
  truncated: z.boolean(),
});

export const GameHistorySchema = z.object({
  seed: z.number(),
  num_players: z.number().int().positive(),
  strategy_names: z.array(z.string()),
  rules_name: z.string(),
  rounds: z.array(RoundHistorySchema),
  final_scores: z.array(z.number()),
  winners: z.array(z.number().int().nonnegative()),
});

export const GameStatsSchema = z.object({
  winners: z.array(z.number().int().nonnegative()),
  final_scores: z.array(z.number()),
  num_rounds: z.number().int().positive(),
  total_turns: z.number().int().nonnegative(),
});

// ─── Aggregate Stats Schemas ────────────────────────────────────────

export const AggregateStatsSchema = z.object({
  num_games: z.number().int().nonnegative(),
  num_players: z.number().int().positive(),
  wins_per_player: z.array(z.number().int().nonnegative()),
  win_rate_per_player: z.array(z.number()),
  avg_score_per_player: z.array(z.number()),
  min_score_per_player: z.array(z.number()),
  max_score_per_player: z.array(z.number()),
  avg_rounds_per_game: z.number(),
  avg_turns_per_game: z.number(),
  score_distributions: z.array(z.array(z.number())),
});

export const ProgressStatsSchema = z.object({
  num_games: z.number().int().nonnegative(),
  num_players: z.number().int().positive(),
  wins_per_player: z.array(z.number().int().nonnegative()),
  win_rate_per_player: z.array(z.number()),
  avg_score_per_player: z.array(z.number()),
  min_score_per_player: z.array(z.number()),
  max_score_per_player: z.array(z.number()),
  avg_rounds_per_game: z.number(),
  avg_turns_per_game: z.number(),
});

export const SimWithHistoriesSchema = z.object({
  stats: AggregateStatsSchema,
  histories: z.array(GameHistorySchema),
});

// ─── Config Schemas ─────────────────────────────────────────────────

export const SimConfigSchema = z.object({
  num_games: z.number().int().positive(),
  seed: z.number(),
  strategies: z.array(z.string()).min(2),
  rules: z.string(),
  withHistories: z.boolean(),
  realtimeVisualization: z.boolean(),
  maxTurnsPerRound: z.number().int().positive(),
});

export const PlayerTypeSchema = z.union([
  z.literal('Human'),
  z.custom<`Bot:${string}`>(
    (val) => typeof val === 'string' && val.startsWith('Bot:'),
  ),
]);

export const PlayConfigSchema = z.object({
  num_players: z.number().int().min(2).max(8),
  player_names: z.array(z.string()),
  player_types: z.array(PlayerTypeSchema),
  rules: z.string(),
  seed: z.number(),
});

// ─── Cache Schemas ──────────────────────────────────────────────────

export const CacheEntrySchema = z.object({
  version: z.literal(1),
  key: z.string(),
  config: SimConfigSchema,
  stats: ProgressStatsSchema,
  gamesCompleted: z.number().int().nonnegative(),
  totalGames: z.number().int().nonnegative(),
  elapsedMs: z.number().nonnegative(),
  hasHistories: z.boolean(),
  savedAt: z.number(),
});

export const CacheExportFileSchema = z.object({
  format: z.literal('skyjo-sim-cache'),
  version: z.literal(1),
  config: SimConfigSchema,
  stats: ProgressStatsSchema,
  gamesCompleted: z.number().int().nonnegative(),
  totalGames: z.number().int().nonnegative(),
  elapsedMs: z.number().nonnegative(),
  histories: z.array(GameHistorySchema).nullable(),
  exportedAt: z.number(),
});

// ─── Worker Message Schemas ─────────────────────────────────────────

export const WorkerRequestSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('start'), config: SimConfigSchema }),
  z.object({ type: z.literal('pause') }),
  z.object({ type: z.literal('resume') }),
  z.object({ type: z.literal('stop') }),
  z.object({ type: z.literal('requestRealtimeGame') }),
  z.object({ type: z.literal('setGeneticGenome'), genome: z.array(z.number()), gamesTrained: z.number() }),
]);

export const WorkerResponseSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('ready') }),
  z.object({
    type: z.literal('progress'),
    stats: ProgressStatsSchema,
    gamesCompleted: z.number().int().nonnegative(),
    totalGames: z.number().int().nonnegative(),
    elapsedMs: z.number().nonnegative(),
  }),
  z.object({
    type: z.literal('complete'),
    stats: ProgressStatsSchema,
    gamesCompleted: z.number().int().nonnegative(),
    totalGames: z.number().int().nonnegative(),
    elapsedMs: z.number().nonnegative(),
    histories: z.array(GameHistorySchema).nullable(),
  }),
  z.object({ type: z.literal('realtimeGame'), history: GameHistorySchema }),
  z.object({ type: z.literal('error'), message: z.string() }),
]);

// ─── Interactive Game Schemas ───────────────────────────────────────

export const ActionNeededSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('ChooseInitialFlips'), player: z.number(), count: z.number() }),
  z.object({ type: z.literal('ChooseDraw'), player: z.number(), drawable_piles: z.array(z.number()) }),
  z.object({ type: z.literal('ChooseDeckDrawAction'), player: z.number(), drawn_card: CardValueSchema.nullable() }),
  z.object({ type: z.literal('ChooseDiscardDrawPlacement'), player: z.number(), drawn_card: CardValueSchema }),
  z.object({
    type: z.literal('RoundOver'),
    round_number: z.number(),
    round_scores: z.array(z.number()),
    raw_round_scores: z.array(z.number()),
    cumulative_scores: z.array(z.number()),
    going_out_player: z.number().nullable(),
    end_of_round_clears: z.array(ColumnClearEventSchema),
  }),
  z.object({
    type: z.literal('GameOver'),
    final_scores: z.array(z.number()),
    winners: z.array(z.number()),
    round_number: z.number(),
    round_scores: z.array(z.number()),
    raw_round_scores: z.array(z.number()),
    going_out_player: z.number().nullable(),
    end_of_round_clears: z.array(ColumnClearEventSchema),
  }),
]);

export const PlayerActionSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('InitialFlip'), position: z.number() }),
  z.object({ type: z.literal('DrawFromDeck') }),
  z.object({ type: z.literal('DrawFromDiscard'), pile_index: z.number() }),
  z.object({ type: z.literal('UndoDrawFromDiscard') }),
  z.object({ type: z.literal('KeepDeckDraw'), position: z.number() }),
  z.object({ type: z.literal('DiscardAndFlip'), position: z.number() }),
  z.object({ type: z.literal('PlaceDiscardDraw'), position: z.number() }),
  z.object({ type: z.literal('ContinueToNextRound') }),
]);

export const InteractiveGameStateSchema = z.object({
  num_players: z.number().int().min(2).max(8),
  player_names: z.array(z.string()),
  num_rows: z.number().int().positive(),
  num_cols: z.number().int().positive(),
  round_number: z.number().int().nonnegative(),
  current_player: z.number().int().nonnegative(),
  action_needed: ActionNeededSchema,
  boards: z.array(z.array(VisibleSlotSchema)),
  discard_tops: z.array(CardValueSchema.nullable()),
  discard_sizes: z.array(z.number().int().nonnegative()),
  deck_remaining: z.number().int().nonnegative(),
  cumulative_scores: z.array(z.number()),
  going_out_player: z.number().int().nonnegative().nullable(),
  is_final_turn: z.boolean(),
  last_column_clears: z.array(ColumnClearEventSchema),
});

export const BotActionResponseSchema = z.object({
  action: PlayerActionSchema,
  state: InteractiveGameStateSchema,
});

// ─── Online Game Schemas ────────────────────────────────────────────

export const PlayerSlotTypeSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('Human') }),
  z.object({ kind: z.literal('Bot'), strategy: z.string() }),
  z.object({ kind: z.literal('Empty') }),
]);

export const LobbyPlayerSchema = z.object({
  slot: z.number().int().nonnegative(),
  name: z.string(),
  player_type: PlayerSlotTypeSchema,
  connected: z.boolean(),
  shares_ip_with_host: z.boolean().optional(),
  disconnect_secs: z.number().optional(),
});

export const RoomLobbyStateSchema = z.object({
  room_code: z.string(),
  players: z.array(LobbyPlayerSchema),
  num_players: z.number().int().positive(),
  rules: z.string(),
  creator: z.number().int().nonnegative(),
  available_strategies: z.array(z.string()),
  available_rules: z.array(z.string()),
  idle_timeout_secs: z.number().nullable(),
  turn_timer_secs: z.number().nullable(),
  last_winners: z.array(z.number()),
  genetic_games_trained: z.number(),
  genetic_generation: z.number(),
});

export const ServerMessageSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('RoomState'), state: RoomLobbyStateSchema }),
  z.object({ type: z.literal('GameState'), state: InteractiveGameStateSchema, turn_deadline_secs: z.number().nullable().optional() }),
  z.object({ type: z.literal('ActionApplied'), player: z.number(), action: PlayerActionSchema, state: InteractiveGameStateSchema, turn_deadline_secs: z.number().nullable().optional() }),
  z.object({ type: z.literal('BotAction'), player: z.number(), action: PlayerActionSchema, state: InteractiveGameStateSchema, turn_deadline_secs: z.number().nullable().optional() }),
  z.object({ type: z.literal('TimeoutAction'), player: z.number(), action: PlayerActionSchema, state: InteractiveGameStateSchema }),
  z.object({ type: z.literal('PlayerJoined'), player_index: z.number(), name: z.string() }),
  z.object({ type: z.literal('PlayerLeft'), player_index: z.number() }),
  z.object({ type: z.literal('PlayerReconnected'), player_index: z.number() }),
  z.object({ type: z.literal('Kicked'), reason: z.string() }),
  z.object({ type: z.literal('Error'), code: z.string(), message: z.string() }),
  z.object({ type: z.literal('Pong') }),
]);

// ─── Strategy Description Schemas ───────────────────────────────────

export const PriorityRuleSchema = z.object({
  condition: z.string(),
  action: z.string(),
  detail: z.string().optional(),
});

type DecisionNodeType =
  | { type: 'Condition'; test: string; if_true: DecisionNodeType; if_false: DecisionNodeType }
  | { type: 'Action'; action: string; detail?: string }
  | { type: 'PriorityList'; rules: z.infer<typeof PriorityRuleSchema>[] };

export const DecisionNodeSchema: z.ZodType<DecisionNodeType> = z.lazy(() =>
  z.discriminatedUnion('type', [
    z.object({
      type: z.literal('Condition'),
      test: z.string(),
      if_true: DecisionNodeSchema,
      if_false: DecisionNodeSchema,
    }),
    z.object({ type: z.literal('Action'), action: z.string(), detail: z.string().optional() }),
    z.object({ type: z.literal('PriorityList'), rules: z.array(PriorityRuleSchema) }),
  ]),
);

export const DecisionLogicSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('Simple'), text: z.string() }),
  z.object({ type: z.literal('PriorityList'), rules: z.array(PriorityRuleSchema) }),
  z.object({ type: z.literal('DecisionTree'), root: DecisionNodeSchema }),
]);

export const PhaseDescriptionSchema = z.object({
  phase: z.enum(['InitialFlips', 'ChooseDraw', 'DeckDrawAction', 'DiscardDrawPlacement']),
  label: z.string(),
  logic: DecisionLogicSchema,
});

export const ConceptReferenceSchema = z.object({
  id: z.string(),
  label: z.string(),
  used_for: z.string(),
});

export const CommonConceptSchema = z.object({
  id: z.string(),
  label: z.string(),
  description: z.string(),
  formula: z.string().optional(),
});

export const StrategyDescriptionSchema = z.object({
  name: z.string(),
  summary: z.string(),
  complexity: z.enum(['Trivial', 'Low', 'Medium', 'High']),
  strengths: z.array(z.string()),
  weaknesses: z.array(z.string()),
  phases: z.array(PhaseDescriptionSchema),
  concepts: z.array(ConceptReferenceSchema),
});

export const StrategyDescriptionsDataSchema = z.object({
  strategies: z.array(StrategyDescriptionSchema),
  common_concepts: z.array(CommonConceptSchema),
});

// ─── Genetic Model Schemas ──────────────────────────────────────────

export const GeneticModelDataSchema = z.object({
  best_genome: z.array(z.number()),
  input_size: z.number().int().nonnegative(),
  hidden_size: z.number().int().nonnegative(),
  output_size: z.number().int().nonnegative(),
  generation: z.number().int().nonnegative(),
  total_games_trained: z.number().int().nonnegative(),
  input_labels: z.array(z.string()),
  output_labels: z.array(z.string()),
  input_groups: z.array(z.tuple([z.string(), z.number(), z.number()])),
  output_groups: z.array(z.tuple([z.string(), z.number(), z.number()])),
  lineage_hash: z.string(),
});

export const SavedGenerationInfoSchema = z.object({
  name: z.string(),
  generation: z.number().int().nonnegative(),
  total_games_trained: z.number().int().nonnegative(),
  best_fitness: z.number(),
  saved_at: z.string(),
  lineage_hash: z.string(),
});

export const GeneticTrainingStatusSchema = z.object({
  is_training: z.boolean(),
  generation: z.number().int().nonnegative(),
  total_games_trained: z.number().int().nonnegative(),
  best_fitness: z.number(),
  training_start_generation: z.number().int().nonnegative(),
  training_target_generation: z.number().int().nonnegative(),
  training_elapsed_ms: z.number().nonnegative(),
  training_last_gen_elapsed_ms: z.number().nonnegative(),
  training_mode: z.string(),
  training_target_fitness: z.number(),
  training_start_fitness: z.number(),
  lineage_hash: z.string(),
});

// ─── WASM Response Wrapper Schemas ──────────────────────────────────
// WASM functions return JSON that either has the data or { error: string }

export const WasmResultSchema = <T extends z.ZodType>(dataSchema: T) =>
  z.union([dataSchema, z.object({ error: z.string() })]);

export const GameStatsWithHistorySchema = z.object({
  stats: GameStatsSchema,
  history: GameHistorySchema,
});

// ─── Helper: parse JSON with schema ─────────────────────────────────

/** Parse a JSON string with a Zod schema. Throws on invalid JSON or schema mismatch. */
export function parseJson<T extends z.ZodType>(schema: T, json: string): z.infer<T> {
  return schema.parse(JSON.parse(json));
}

/** Safely parse a JSON string with a Zod schema. Returns { success, data, error }. */
export function safeParseJson<T extends z.ZodType>(
  schema: T,
  json: string,
): { success: true; data: z.infer<T> } | { success: false; error: z.ZodError } {
  try {
    const data = JSON.parse(json);
    const result = schema.safeParse(data);
    if (result.success) {
      return { success: true, data: result.data };
    }
    return { success: false, error: result.error as z.ZodError };
  } catch {
    return {
      success: false,
      error: new z.ZodError([
        {
          code: 'custom',
          message: 'Invalid JSON',
          path: [],
        },
      ]),
    };
  }
}
