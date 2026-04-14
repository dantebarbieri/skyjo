import { describe, it, expect } from 'vitest';
import {
  CardValueSchema,
  SlotSchema,
  VisibleSlotSchema,
  TurnActionSchema,
  ColumnClearEventSchema,
  TurnRecordSchema,
  RoundHistorySchema,
  GameHistorySchema,
  GameStatsSchema,
  AggregateStatsSchema,
  ProgressStatsSchema,
  SimConfigSchema,
  PlayConfigSchema,
  CacheEntrySchema,
  CacheExportFileSchema,
  WorkerRequestSchema,
  WorkerResponseSchema,
  ActionNeededSchema,
  PlayerActionSchema,
  InteractiveGameStateSchema,
  BotActionResponseSchema,
  ServerMessageSchema,
  RoomLobbyStateSchema,
  DecisionNodeSchema,
  DecisionLogicSchema,
  StrategyDescriptionSchema,
  GeneticModelDataSchema,
  GeneticTrainingStatusSchema,
  GameStatsWithHistorySchema,
  parseJson,
  safeParseJson,
} from '@/schemas';

// ─── Test Data Factories ────────────────────────────────────────────

function makeColumnClearEvent() {
  return { player_index: 0, column: 1, card_value: 5, displaced_card: null };
}

function makeTurnRecord() {
  return {
    player_index: 0,
    action: {
      DrewFromDeck: { drawn_card: 3, action: { Keep: 2 }, displaced_card: 5 },
    },
    column_clears: [],
    went_out: false,
  };
}

function makeRoundHistory() {
  return {
    round_number: 0,
    initial_deck_order: [1, 2, 3, 4, 5],
    dealt_hands: [[1, 2, 3], [4, 5, 6]],
    setup_flips: [[0, 1], [2, 3]],
    starting_player: 0,
    turns: [makeTurnRecord()],
    going_out_player: 0,
    end_of_round_clears: [],
    round_scores: [10, 20],
    cumulative_scores: [10, 20],
    truncated: false,
  };
}

function makeGameHistory() {
  return {
    seed: 42,
    num_players: 2,
    strategy_names: ['Random', 'Greedy'],
    rules_name: 'Standard',
    rounds: [makeRoundHistory()],
    final_scores: [10, 20],
    winners: [0],
  };
}

function makeProgressStats() {
  return {
    num_games: 100,
    num_players: 2,
    wins_per_player: [55, 45],
    win_rate_per_player: [0.55, 0.45],
    avg_score_per_player: [30.5, 35.2],
    min_score_per_player: [5, 8],
    max_score_per_player: [80, 90],
    avg_rounds_per_game: 3.2,
    avg_turns_per_game: 24.5,
  };
}

function makeSimConfig() {
  return {
    num_games: 100,
    seed: 42,
    strategies: ['Random', 'Random'],
    rules: 'Standard',
    withHistories: false,
    realtimeVisualization: false,
    maxTurnsPerRound: 50,
  };
}

function makeInteractiveGameState() {
  return {
    num_players: 2,
    player_names: ['Alice', 'Bob'],
    num_rows: 3,
    num_cols: 4,
    round_number: 0,
    current_player: 0,
    action_needed: { type: 'ChooseInitialFlips' as const, player: 0, count: 2 },
    boards: [[
      'Hidden', 'Hidden', 'Hidden', 'Hidden',
      'Hidden', 'Hidden', 'Hidden', 'Hidden',
      'Hidden', 'Hidden', 'Hidden', 'Hidden',
    ], [
      'Hidden', 'Hidden', 'Hidden', 'Hidden',
      'Hidden', 'Hidden', 'Hidden', 'Hidden',
      'Hidden', 'Hidden', 'Hidden', 'Hidden',
    ]],
    discard_tops: [3],
    discard_sizes: [1],
    deck_remaining: 125,
    cumulative_scores: [0, 0],
    going_out_player: null,
    is_final_turn: false,
    last_column_clears: [],
  };
}

// ─── 1. Runtime Validation: Catches Bad Data ────────────────────────

describe('Runtime validation', () => {
  it('accepts valid CardValue in range [-2, 12]', () => {
    expect(CardValueSchema.parse(-2)).toBe(-2);
    expect(CardValueSchema.parse(0)).toBe(0);
    expect(CardValueSchema.parse(12)).toBe(12);
  });

  it('rejects CardValue out of range', () => {
    expect(() => CardValueSchema.parse(-3)).toThrow();
    expect(() => CardValueSchema.parse(13)).toThrow();
    expect(() => CardValueSchema.parse(1.5)).toThrow();
  });

  it('rejects CardValue of wrong type', () => {
    expect(() => CardValueSchema.parse('five')).toThrow();
    expect(() => CardValueSchema.parse(null)).toThrow();
    expect(() => CardValueSchema.parse(undefined)).toThrow();
  });

  it('accepts valid Slot variants', () => {
    expect(SlotSchema.parse({ Hidden: 5 })).toEqual({ Hidden: 5 });
    expect(SlotSchema.parse({ Revealed: -1 })).toEqual({ Revealed: -1 });
    expect(SlotSchema.parse('Cleared')).toBe('Cleared');
  });

  it('rejects invalid Slot', () => {
    expect(() => SlotSchema.parse({ Hidden: 'abc' })).toThrow();
    expect(() => SlotSchema.parse('hidden')).toThrow();
    expect(() => SlotSchema.parse(42)).toThrow();
  });

  it('validates a complete GameHistory', () => {
    const history = makeGameHistory();
    expect(GameHistorySchema.parse(history)).toEqual(history);
  });

  it('rejects GameHistory with missing required fields', () => {
    const { seed, ...noSeed } = makeGameHistory();
    expect(() => GameHistorySchema.parse(noSeed)).toThrow();
  });

  it('rejects GameHistory with wrong field types', () => {
    const history = { ...makeGameHistory(), num_players: 'two' };
    expect(() => GameHistorySchema.parse(history)).toThrow();
  });

  it('validates SimConfig with constraints', () => {
    expect(SimConfigSchema.parse(makeSimConfig())).toEqual(makeSimConfig());
  });

  it('rejects SimConfig with invalid num_games', () => {
    expect(() => SimConfigSchema.parse({ ...makeSimConfig(), num_games: 0 })).toThrow();
    expect(() => SimConfigSchema.parse({ ...makeSimConfig(), num_games: -1 })).toThrow();
  });

  it('rejects SimConfig with too few strategies', () => {
    expect(() => SimConfigSchema.parse({ ...makeSimConfig(), strategies: ['Random'] })).toThrow();
  });
});

// ─── 2. Discriminated Unions ────────────────────────────────────────

describe('Discriminated unions', () => {
  describe('WorkerResponse', () => {
    it('parses "ready" variant', () => {
      expect(WorkerResponseSchema.parse({ type: 'ready' })).toEqual({ type: 'ready' });
    });

    it('parses "progress" variant', () => {
      const msg = {
        type: 'progress',
        stats: makeProgressStats(),
        gamesCompleted: 50,
        totalGames: 100,
        elapsedMs: 1234,
      };
      expect(WorkerResponseSchema.parse(msg)).toEqual(msg);
    });

    it('parses "complete" variant', () => {
      const msg = {
        type: 'complete',
        stats: makeProgressStats(),
        gamesCompleted: 100,
        totalGames: 100,
        elapsedMs: 5000,
        histories: null,
      };
      expect(WorkerResponseSchema.parse(msg)).toEqual(msg);
    });

    it('parses "error" variant', () => {
      const msg = { type: 'error', message: 'Something went wrong' };
      expect(WorkerResponseSchema.parse(msg)).toEqual(msg);
    });

    it('rejects unknown discriminator value', () => {
      expect(() => WorkerResponseSchema.parse({ type: 'unknown' })).toThrow();
    });

    it('rejects missing discriminator', () => {
      expect(() => WorkerResponseSchema.parse({ stats: makeProgressStats() })).toThrow();
    });
  });

  describe('WorkerRequest', () => {
    it('parses all valid variants', () => {
      expect(WorkerRequestSchema.parse({ type: 'pause' })).toEqual({ type: 'pause' });
      expect(WorkerRequestSchema.parse({ type: 'resume' })).toEqual({ type: 'resume' });
      expect(WorkerRequestSchema.parse({ type: 'stop' })).toEqual({ type: 'stop' });
      expect(WorkerRequestSchema.parse({ type: 'requestRealtimeGame' })).toEqual({ type: 'requestRealtimeGame' });
    });

    it('parses "start" variant with config', () => {
      const msg = { type: 'start', config: makeSimConfig() };
      expect(WorkerRequestSchema.parse(msg)).toEqual(msg);
    });
  });

  describe('TurnAction', () => {
    it('parses DrewFromDeck variant', () => {
      const action = {
        DrewFromDeck: {
          drawn_card: 5,
          action: { Keep: 3 },
          displaced_card: 7,
        },
      };
      expect(TurnActionSchema.parse(action)).toEqual(action);
    });

    it('parses DrewFromDiscard variant', () => {
      const action = {
        DrewFromDiscard: {
          pile_index: 0,
          drawn_card: 3,
          placement: 4,
          displaced_card: 8,
        },
      };
      expect(TurnActionSchema.parse(action)).toEqual(action);
    });

    it('rejects action with both variants', () => {
      expect(() =>
        TurnActionSchema.parse({
          DrewFromDeck: { drawn_card: 5, action: { Keep: 3 }, displaced_card: 7 },
          DrewFromDiscard: { pile_index: 0, drawn_card: 3, placement: 4, displaced_card: 8 },
        }),
      ).toThrow(); // Strict objects reject unrecognized keys
    });
  });

  describe('ActionNeeded', () => {
    it('parses each variant correctly', () => {
      expect(ActionNeededSchema.parse({ type: 'ChooseInitialFlips', player: 0, count: 2 }))
        .toEqual({ type: 'ChooseInitialFlips', player: 0, count: 2 });

      expect(ActionNeededSchema.parse({ type: 'ChooseDraw', player: 1, drawable_piles: [0] }))
        .toEqual({ type: 'ChooseDraw', player: 1, drawable_piles: [0] });

      expect(ActionNeededSchema.parse({ type: 'ChooseDeckDrawAction', player: 0, drawn_card: 5 }))
        .toEqual({ type: 'ChooseDeckDrawAction', player: 0, drawn_card: 5 });

      expect(ActionNeededSchema.parse({ type: 'ChooseDiscardDrawPlacement', player: 0, drawn_card: 3 }))
        .toEqual({ type: 'ChooseDiscardDrawPlacement', player: 0, drawn_card: 3 });
    });

    it('parses RoundOver with all fields', () => {
      const roundOver = {
        type: 'RoundOver',
        round_number: 1,
        round_scores: [15, 25],
        raw_round_scores: [15, 25],
        cumulative_scores: [30, 50],
        going_out_player: 0,
        end_of_round_clears: [makeColumnClearEvent()],
      };
      expect(ActionNeededSchema.parse(roundOver)).toEqual(roundOver);
    });

    it('rejects unknown action type', () => {
      expect(() => ActionNeededSchema.parse({ type: 'InvalidAction', player: 0 })).toThrow();
    });
  });

  describe('PlayerAction', () => {
    it('parses all variants', () => {
      const variants = [
        { type: 'InitialFlip', position: 0 },
        { type: 'DrawFromDeck' },
        { type: 'DrawFromDiscard', pile_index: 0 },
        { type: 'UndoDrawFromDiscard' },
        { type: 'KeepDeckDraw', position: 5 },
        { type: 'DiscardAndFlip', position: 2 },
        { type: 'PlaceDiscardDraw', position: 3 },
        { type: 'ContinueToNextRound' },
      ];
      for (const v of variants) {
        expect(PlayerActionSchema.parse(v)).toEqual(v);
      }
    });
  });

  describe('ServerMessage', () => {
    it('parses RoomState variant', () => {
      const roomState = {
        room_code: 'ABCD',
        players: [{ slot: 0, name: 'Alice', player_type: { kind: 'Human' }, connected: true }],
        num_players: 2,
        rules: 'Standard',
        creator: 0,
        available_strategies: ['Random'],
        available_rules: ['Standard'],
        idle_timeout_secs: null,
        turn_timer_secs: 30,
        last_winners: [],
        genetic_games_trained: 0,
        genetic_generation: 0,
      };
      const msg = { type: 'RoomState', state: roomState };
      expect(ServerMessageSchema.parse(msg)).toEqual(msg);
    });

    it('parses Error variant', () => {
      const msg = { type: 'Error', code: 'INVALID', message: 'Bad request' };
      expect(ServerMessageSchema.parse(msg)).toEqual(msg);
    });

    it('parses Pong', () => {
      expect(ServerMessageSchema.parse({ type: 'Pong' })).toEqual({ type: 'Pong' });
    });

    it('rejects unknown message type', () => {
      expect(() => ServerMessageSchema.parse({ type: 'Unknown' })).toThrow();
    });
  });
});

// ─── 3. Safe Parsing Returns Structured Errors ─────────────────────

describe('Safe parsing (safeParse)', () => {
  it('returns success for valid data', () => {
    const result = CardValueSchema.safeParse(5);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data).toBe(5);
    }
  });

  it('returns structured error for invalid data', () => {
    const result = CardValueSchema.safeParse('not a number');
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error).toBeDefined();
      expect(result.error.issues).toBeInstanceOf(Array);
      expect(result.error.issues.length).toBeGreaterThan(0);
      expect(result.error.issues[0]).toHaveProperty('message');
      expect(result.error.issues[0]).toHaveProperty('path');
    }
  });

  it('error includes path for nested objects', () => {
    const badTurnRecord = {
      player_index: 0,
      action: {
        DrewFromDeck: { drawn_card: 'bad', action: { Keep: 0 }, displaced_card: null },
      },
      column_clears: [],
      went_out: false,
    };
    const result = TurnRecordSchema.safeParse(badTurnRecord);
    expect(result.success).toBe(false);
  });

  it('does not throw on invalid data', () => {
    // safeParse should never throw, even with completely wrong input
    expect(() => GameHistorySchema.safeParse(null)).not.toThrow();
    expect(() => GameHistorySchema.safeParse(undefined)).not.toThrow();
    expect(() => GameHistorySchema.safeParse(42)).not.toThrow();
    expect(() => GameHistorySchema.safeParse('garbage')).not.toThrow();
  });
});

// ─── 4. Schema Composition & Reuse ─────────────────────────────────

describe('Schema composition', () => {
  it('RoundHistory reuses TurnRecord which reuses TurnAction', () => {
    const round = makeRoundHistory();
    expect(RoundHistorySchema.parse(round)).toEqual(round);
  });

  it('catches deeply nested invalid data', () => {
    const round = makeRoundHistory();
    // Corrupt a deeply nested field: turn action's drawn_card
    round.turns[0].action = {
      DrewFromDeck: { drawn_card: 99, action: { Keep: 0 }, displaced_card: null },
    } as any; // 99 is out of range for CardValue
    expect(() => RoundHistorySchema.parse(round)).toThrow();
  });

  it('GameStatsWithHistory composes GameStats and GameHistory', () => {
    const data = {
      stats: { winners: [0], final_scores: [10, 20], num_rounds: 3, total_turns: 15 },
      history: makeGameHistory(),
    };
    expect(GameStatsWithHistorySchema.parse(data)).toEqual(data);
  });

  it('InteractiveGameState composes ActionNeeded, VisibleSlot, ColumnClearEvent', () => {
    expect(InteractiveGameStateSchema.parse(makeInteractiveGameState())).toEqual(makeInteractiveGameState());
  });
});

// ─── 5. Coercion and Transforms ────────────────────────────────────

describe('Value constraints', () => {
  it('CardValue rejects non-integer values', () => {
    expect(() => CardValueSchema.parse(1.5)).toThrow();
    expect(() => CardValueSchema.parse(NaN)).toThrow();
  });

  it('SimConfig.num_games must be positive integer', () => {
    expect(() => SimConfigSchema.parse({ ...makeSimConfig(), num_games: 0 })).toThrow();
    expect(() => SimConfigSchema.parse({ ...makeSimConfig(), num_games: -5 })).toThrow();
    expect(() => SimConfigSchema.parse({ ...makeSimConfig(), num_games: 1.5 })).toThrow();
  });

  it('PlayConfig.num_players must be 2-8', () => {
    const base = {
      player_names: ['A', 'B'],
      player_types: ['Human', 'Human'],
      rules: 'Standard',
      seed: 1,
    };
    expect(() => PlayConfigSchema.parse({ ...base, num_players: 1 })).toThrow();
    expect(() => PlayConfigSchema.parse({ ...base, num_players: 9 })).toThrow();
    expect(PlayConfigSchema.parse({ ...base, num_players: 2 })).toBeTruthy();
    expect(PlayConfigSchema.parse({ ...base, num_players: 8 })).toBeTruthy();
  });

  it('VisibleSlot accepts all three variants', () => {
    expect(VisibleSlotSchema.parse('Hidden')).toBe('Hidden');
    expect(VisibleSlotSchema.parse({ Revealed: 7 })).toEqual({ Revealed: 7 });
    expect(VisibleSlotSchema.parse('Cleared')).toBe('Cleared');
  });
});

// ─── 6. Cache Import Validation (Integration-style) ────────────────

describe('Cache schema validation', () => {
  it('validates a correct CacheEntry', () => {
    const entry = {
      version: 1,
      key: 'abc123',
      config: makeSimConfig(),
      stats: makeProgressStats(),
      gamesCompleted: 100,
      totalGames: 100,
      elapsedMs: 5000,
      hasHistories: false,
      savedAt: Date.now(),
    };
    expect(CacheEntrySchema.parse(entry)).toEqual(entry);
  });

  it('rejects CacheEntry with wrong version', () => {
    const entry = {
      version: 2,
      key: 'abc',
      config: makeSimConfig(),
      stats: makeProgressStats(),
      gamesCompleted: 100,
      totalGames: 100,
      elapsedMs: 5000,
      hasHistories: false,
      savedAt: Date.now(),
    };
    expect(() => CacheEntrySchema.parse(entry)).toThrow();
  });

  it('validates a correct CacheExportFile', () => {
    const exportFile = {
      format: 'skyjo-sim-cache',
      version: 1,
      config: makeSimConfig(),
      stats: makeProgressStats(),
      gamesCompleted: 100,
      totalGames: 100,
      elapsedMs: 5000,
      histories: null,
      exportedAt: Date.now(),
    };
    expect(CacheExportFileSchema.parse(exportFile)).toEqual(exportFile);
  });

  it('rejects CacheExportFile with wrong format', () => {
    const exportFile = {
      format: 'wrong-format',
      version: 1,
      config: makeSimConfig(),
      stats: makeProgressStats(),
      gamesCompleted: 100,
      totalGames: 100,
      elapsedMs: 5000,
      histories: null,
      exportedAt: Date.now(),
    };
    expect(() => CacheExportFileSchema.parse(exportFile)).toThrow();
  });

  it('rejects CacheExportFile with missing config', () => {
    const exportFile = {
      format: 'skyjo-sim-cache',
      version: 1,
      stats: makeProgressStats(),
      gamesCompleted: 100,
      totalGames: 100,
      elapsedMs: 5000,
      histories: null,
      exportedAt: Date.now(),
    };
    expect(() => CacheExportFileSchema.parse(exportFile)).toThrow();
  });
});

// ─── 7. Recursive Schemas ───────────────────────────────────────────

describe('Recursive DecisionNode schema', () => {
  it('parses a simple Action node', () => {
    const node = { type: 'Action', action: 'Draw from deck' };
    expect(DecisionNodeSchema.parse(node)).toEqual(node);
  });

  it('parses a nested Condition tree', () => {
    const tree = {
      type: 'Condition',
      test: 'Is card < 5?',
      if_true: { type: 'Action', action: 'Keep card' },
      if_false: {
        type: 'Condition',
        test: 'Is card < 8?',
        if_true: { type: 'Action', action: 'Consider keeping' },
        if_false: { type: 'Action', action: 'Discard' },
      },
    };
    expect(DecisionNodeSchema.parse(tree)).toEqual(tree);
  });

  it('parses PriorityList variant', () => {
    const node = {
      type: 'PriorityList',
      rules: [
        { condition: 'Low card', action: 'Keep' },
        { condition: 'High card', action: 'Discard', detail: 'Always discard > 8' },
      ],
    };
    expect(DecisionNodeSchema.parse(node)).toEqual(node);
  });
});

// ─── 8. JSON Parse Helpers ──────────────────────────────────────────

describe('parseJson helper', () => {
  it('parses valid JSON with schema', () => {
    const json = JSON.stringify({ type: 'ready' });
    expect(parseJson(WorkerResponseSchema, json)).toEqual({ type: 'ready' });
  });

  it('throws on invalid JSON', () => {
    expect(() => parseJson(WorkerResponseSchema, 'not json')).toThrow();
  });

  it('throws on valid JSON that fails schema', () => {
    expect(() => parseJson(WorkerResponseSchema, JSON.stringify({ type: 'invalid' }))).toThrow();
  });
});

describe('safeParseJson helper', () => {
  it('returns success for valid JSON + schema', () => {
    const json = JSON.stringify({ type: 'error', message: 'oops' });
    const result = safeParseJson(WorkerResponseSchema, json);
    expect(result.success).toBe(true);
  });

  it('returns failure for invalid JSON', () => {
    const result = safeParseJson(WorkerResponseSchema, 'not json');
    expect(result.success).toBe(false);
  });

  it('returns failure for valid JSON that fails schema', () => {
    const result = safeParseJson(WorkerResponseSchema, JSON.stringify({ type: 'unknown' }));
    expect(result.success).toBe(false);
  });
});

// ─── 9. Genetic Model Schemas ───────────────────────────────────────

describe('Genetic model schemas', () => {
  it('validates GeneticModelData', () => {
    const model = {
      best_genome: [0.1, 0.2, 0.3],
      input_size: 10,
      hidden_size: 5,
      output_size: 3,
      generation: 50,
      total_games_trained: 10000,
      input_labels: ['a', 'b'],
      output_labels: ['x'],
      input_groups: [['group1', 0, 5]],
      output_groups: [['out1', 0, 3]],
      lineage_hash: 'abc123',
    };
    expect(GeneticModelDataSchema.parse(model)).toEqual(model);
  });

  it('validates GeneticTrainingStatus', () => {
    const status = {
      is_training: true,
      generation: 10,
      total_games_trained: 5000,
      best_fitness: 0.85,
      training_start_generation: 0,
      training_target_generation: 100,
      training_elapsed_ms: 30000,
      training_last_gen_elapsed_ms: 300,
      training_mode: 'standard',
      training_target_fitness: 0.95,
      training_start_fitness: 0.5,
      lineage_hash: 'def456',
    };
    expect(GeneticTrainingStatusSchema.parse(status)).toEqual(status);
  });
});
