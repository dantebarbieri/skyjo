import { z } from 'zod';
import {
  CardValueSchema,
  SlotSchema,
  DeckDrawActionSchema,
  ColumnClearEventSchema,
  TurnActionSchema,
  TurnRecordSchema,
  RoundHistorySchema,
  GameHistorySchema,
  GameStatsSchema,
  AggregateStatsSchema,
  ProgressStatsSchema,
  SimWithHistoriesSchema,
  SimConfigSchema,
  PlayerTypeSchema,
  PlayConfigSchema,
  CacheEntrySchema,
  CacheExportFileSchema,
  WorkerRequestSchema,
  WorkerResponseSchema,
  VisibleSlotSchema,
  ActionNeededSchema,
  PlayerActionSchema,
  InteractiveGameStateSchema,
  BotActionResponseSchema,
  StrategyDescriptionSchema,
  PhaseDescriptionSchema,
  DecisionLogicSchema,
  PriorityRuleSchema,
  DecisionNodeSchema,
  ConceptReferenceSchema,
  CommonConceptSchema,
  StrategyDescriptionsDataSchema,
  GeneticModelDataSchema,
  SavedGenerationInfoSchema,
  GeneticTrainingStatusSchema,
  GamePlayerSummarySchema,
  GameSummarySchema,
  GameListResponseSchema,
  RoundScoreDetailSchema,
  RoundDetailSchema,
  GamePlayerDetailSchema,
  GameDetailSchema,
} from './schemas';

// All types are inferred from Zod schemas — single source of truth

export type CardValue = z.infer<typeof CardValueSchema>;
export type Slot = z.infer<typeof SlotSchema>;
export type DeckDrawAction = z.infer<typeof DeckDrawActionSchema>;
export type ColumnClearEvent = z.infer<typeof ColumnClearEventSchema>;
export type TurnAction = z.infer<typeof TurnActionSchema>;
export type TurnRecord = z.infer<typeof TurnRecordSchema>;
export type RoundHistory = z.infer<typeof RoundHistorySchema>;
export type GameHistory = z.infer<typeof GameHistorySchema>;
export type GameStats = z.infer<typeof GameStatsSchema>;
export type AggregateStats = z.infer<typeof AggregateStatsSchema>;
export type ProgressStats = z.infer<typeof ProgressStatsSchema>;
export type SimWithHistories = z.infer<typeof SimWithHistoriesSchema>;

// Cache types
export type CacheEntry = z.infer<typeof CacheEntrySchema>;
export type CacheExportFile = z.infer<typeof CacheExportFileSchema>;

// Worker message types
export type SimConfig = z.infer<typeof SimConfigSchema>;
export type WorkerRequest = z.infer<typeof WorkerRequestSchema>;
export type WorkerResponse = z.infer<typeof WorkerResponseSchema>;

// Interactive game types (for Play mode)
export type VisibleSlot = z.infer<typeof VisibleSlotSchema>;
export type ActionNeeded = z.infer<typeof ActionNeededSchema>;
export type PlayerAction = z.infer<typeof PlayerActionSchema>;
export type InteractiveGameState = z.infer<typeof InteractiveGameStateSchema>;
export type PlayConfig = z.infer<typeof PlayConfigSchema>;

/** Player type: "Human" or "Bot:<StrategyName>" (e.g. "Bot:Random", "Bot:Greedy") */
export type PlayerType = z.infer<typeof PlayerTypeSchema>;

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

export type BotActionResponse = z.infer<typeof BotActionResponseSchema>;

// Strategy description types (from Rust describe() trait method)
export type StrategyDescription = z.infer<typeof StrategyDescriptionSchema>;
export type PhaseDescription = z.infer<typeof PhaseDescriptionSchema>;
export type DecisionLogic = z.infer<typeof DecisionLogicSchema>;
export type PriorityRule = z.infer<typeof PriorityRuleSchema>;
export type DecisionNode = z.infer<typeof DecisionNodeSchema>;
export type ConceptReference = z.infer<typeof ConceptReferenceSchema>;
export type CommonConcept = z.infer<typeof CommonConceptSchema>;
export type StrategyDescriptionsData = z.infer<typeof StrategyDescriptionsDataSchema>;

// Genetic model types
export type GeneticModelData = z.infer<typeof GeneticModelDataSchema>;
export type SavedGenerationInfo = z.infer<typeof SavedGenerationInfoSchema>;
export type GeneticTrainingStatus = z.infer<typeof GeneticTrainingStatusSchema>;

// Leaderboard / Game History types
export type GamePlayerSummary = z.infer<typeof GamePlayerSummarySchema>;
export type GameSummary = z.infer<typeof GameSummarySchema>;
export type GameListResponse = z.infer<typeof GameListResponseSchema>;
export type RoundScoreDetail = z.infer<typeof RoundScoreDetailSchema>;
export type RoundDetail = z.infer<typeof RoundDetailSchema>;
export type GamePlayerDetail = z.infer<typeof GamePlayerDetailSchema>;
export type GameDetail = z.infer<typeof GameDetailSchema>;
