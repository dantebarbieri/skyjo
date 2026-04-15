import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import type { InteractiveGameState, PlayerType, BotSpeed } from '@/types';
import type { PlayPhase } from '../use-interactive-game';
import { useBotTurns } from '../use-bot-turns';

function makeGameState(
  actionType: string,
  player: number,
): InteractiveGameState {
  const actionNeeded = (() => {
    switch (actionType) {
      case 'ChooseInitialFlips':
        return { type: 'ChooseInitialFlips' as const, player, count: 2 };
      case 'ChooseDraw':
        return { type: 'ChooseDraw' as const, player, drawable_piles: [0] };
      case 'RoundOver':
        return {
          type: 'RoundOver' as const,
          round_number: 0,
          round_scores: [10, 20],
          raw_round_scores: [10, 20],
          cumulative_scores: [10, 20],
          going_out_player: 0,
          end_of_round_clears: [],
        };
      case 'GameOver':
        return {
          type: 'GameOver' as const,
          final_scores: [10, 20],
          winners: [0],
          round_number: 1,
          round_scores: [10, 20],
          raw_round_scores: [10, 20],
          going_out_player: 0,
          end_of_round_clears: [],
        };
      default:
        return { type: 'ChooseDraw' as const, player, drawable_piles: [0] };
    }
  })();

  return {
    num_players: 2,
    player_names: ['Human', 'Bot'],
    num_rows: 3,
    num_cols: 4,
    round_number: 0,
    current_player: player,
    action_needed: actionNeeded,
    boards: [Array(12).fill('Hidden'), Array(12).fill('Hidden')],
    discard_tops: [5],
    discard_sizes: [1],
    deck_remaining: 100,
    cumulative_scores: [0, 0],
    going_out_player: null,
    is_final_turn: false,
    last_column_clears: [],
  };
}

interface HookOptions {
  gameState?: InteractiveGameState | null;
  phase?: PlayPhase;
  playerTypes?: PlayerType[];
  botSpeed?: BotSpeed;
  applyBotTurn?: ReturnType<typeof vi.fn<(strategyName: string) => void>>;
  continueToNextRound?: ReturnType<typeof vi.fn<() => void>>;
  showStartingPlayer?: boolean;
  pendingColumnClear?: boolean;
}

function makeOptions(overrides: HookOptions = {}) {
  return {
    gameState: overrides.gameState ?? null,
    phase: overrides.phase ?? 'setup' as PlayPhase,
    playerTypes: overrides.playerTypes ?? ['Human', 'Bot:Random'] as PlayerType[],
    botSpeed: overrides.botSpeed ?? 'instant' as BotSpeed,
    applyBotTurn: overrides.applyBotTurn ?? vi.fn<(strategyName: string) => void>(),
    continueToNextRound: overrides.continueToNextRound ?? vi.fn<() => void>(),
    showStartingPlayer: overrides.showStartingPlayer ?? false,
    pendingColumnClear: overrides.pendingColumnClear ?? false,
  };
}

describe('useBotTurns', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('does nothing when gameState is null', () => {
    const opts = makeOptions({ phase: 'playing' });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });

  it('does nothing when there are no bots', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 0),
      phase: 'playing',
      playerTypes: ['Human', 'Human'],
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });

  it('does nothing when showStartingPlayer is true', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 1),
      phase: 'playing',
      showStartingPlayer: true,
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });

  it('does not trigger for human player turns', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 0),
      phase: 'playing',
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });

  it('triggers applyBotTurn when it is a bot player turn', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 1),
      phase: 'playing',
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(0);
    expect(opts.applyBotTurn).toHaveBeenCalledWith('Random');
  });

  it('triggers bot turn during initial_flips phase', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseInitialFlips', 1),
      phase: 'initial_flips',
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(0);
    expect(opts.applyBotTurn).toHaveBeenCalledWith('Random');
  });

  it('does not trigger during setup phase', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 1),
      phase: 'setup',
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });

  it('does not trigger during game_over phase', () => {
    const opts = makeOptions({
      gameState: makeGameState('GameOver', 0),
      phase: 'game_over',
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });

  it('auto-continues round when all players are bots', () => {
    const opts = makeOptions({
      gameState: makeGameState('RoundOver', 0),
      phase: 'round_over',
      playerTypes: ['Bot:Random', 'Bot:Random'],
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(0);
    expect(opts.continueToNextRound).toHaveBeenCalled();
  });

  it('does not auto-continue round when a human is present', () => {
    const opts = makeOptions({
      gameState: makeGameState('RoundOver', 0),
      phase: 'round_over',
      playerTypes: ['Human', 'Bot:Random'],
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.continueToNextRound).not.toHaveBeenCalled();
  });

  it('respects bot speed delay', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 1),
      phase: 'playing',
      botSpeed: 'normal',
    });
    renderHook(() => useBotTurns(opts));

    // Should not fire before the delay (600ms for normal)
    vi.advanceTimersByTime(500);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(100);
    expect(opts.applyBotTurn).toHaveBeenCalledWith('Random');
  });

  it('does not trigger bot turn when pendingColumnClear is true', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 1),
      phase: 'playing',
      pendingColumnClear: true,
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(1000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });

  it('triggers bot turn when pendingColumnClear is false', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 1),
      phase: 'playing',
      pendingColumnClear: false,
    });
    renderHook(() => useBotTurns(opts));
    vi.advanceTimersByTime(0);
    expect(opts.applyBotTurn).toHaveBeenCalledWith('Random');
  });

  it('cleans up timer on unmount', () => {
    const opts = makeOptions({
      gameState: makeGameState('ChooseDraw', 1),
      phase: 'playing',
      botSpeed: 'slow',
    });
    const { unmount } = renderHook(() => useBotTurns(opts));

    // Unmount before timer fires
    unmount();
    vi.advanceTimersByTime(2000);
    expect(opts.applyBotTurn).not.toHaveBeenCalled();
  });
});
