import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import type { InteractiveGameState, PlayConfig, ActionNeeded } from '@/types';

// --- WASM mock ---

const mockWasm = {
  default: vi.fn(() => Promise.resolve()),
  create_interactive_game: vi.fn(),
  apply_action: vi.fn(),
  apply_bot_action: vi.fn(),
  destroy_interactive_game: vi.fn(),
};

vi.mock('../../../pkg/skyjo_wasm.js', () => mockWasm);

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => { store[key] = value; }),
    removeItem: vi.fn((key: string) => { delete store[key]; }),
    clear: vi.fn(() => { store = {}; }),
  };
})();
vi.stubGlobal('localStorage', localStorageMock);

import { useInteractiveGame } from '../use-interactive-game';

// --- Helpers ---

const makeConfig = (): PlayConfig => ({
  num_players: 2,
  player_names: ['Alice', 'Bob'],
  player_types: ['Human', 'Human'],
  rules: 'Standard',
  seed: 42,
});

function makeGameState(actionNeeded: ActionNeeded): InteractiveGameState {
  return {
    num_players: 2,
    player_names: ['Alice', 'Bob'],
    num_rows: 3,
    num_cols: 4,
    round_number: 0,
    current_player: 0,
    action_needed: actionNeeded,
    boards: [
      Array(12).fill('Hidden'),
      Array(12).fill('Hidden'),
    ],
    discard_tops: [5],
    discard_sizes: [1],
    deck_remaining: 100,
    cumulative_scores: [0, 0],
    going_out_player: null,
    is_final_turn: false,
    last_column_clears: [],
  };
}

describe('useInteractiveGame', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorageMock.clear();
  });

  it('has correct initial state', () => {
    const { result } = renderHook(() => useInteractiveGame());

    expect(result.current.phase).toBe('setup');
    expect(result.current.gameState).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.roundHistory).toEqual([]);
  });

  it('createGame calls WASM and sets gameState and phase', async () => {
    const initialState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: initialState }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(makeConfig());
    });

    await waitFor(() => {
      expect(result.current.phase).toBe('initial_flips');
    });
    expect(result.current.gameState).toBeDefined();
    expect(result.current.gameState!.action_needed.type).toBe('ChooseInitialFlips');
    expect(mockWasm.create_interactive_game).toHaveBeenCalled();
  });

  it('applyAction calls WASM apply_action and updates gameState', async () => {
    // Setup: create a game first
    const initialState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: initialState }),
    );

    const afterFlipState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 1,
    });
    mockWasm.apply_action.mockReturnValue(
      JSON.stringify({ state: afterFlipState }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(makeConfig());
    });

    act(() => {
      result.current.applyAction({ type: 'InitialFlip', position: 0 });
    });

    expect(mockWasm.apply_action).toHaveBeenCalledWith(1, JSON.stringify({ type: 'InitialFlip', position: 0 }));
    expect(result.current.gameState!.action_needed.type).toBe('ChooseInitialFlips');
  });

  it('applyAction with WASM error sets error state', async () => {
    const initialState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: initialState }),
    );
    mockWasm.apply_action.mockReturnValue(
      JSON.stringify({ error: 'Invalid action' }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(makeConfig());
    });

    act(() => {
      result.current.applyAction({ type: 'InitialFlip', position: 99 });
    });

    expect(result.current.error).toBe('Invalid action');
  });

  it('applyBotTurn calls WASM apply_bot_action and updates gameState', async () => {
    const initialState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: initialState }),
    );

    const afterBotState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 1,
      count: 2,
    });
    mockWasm.apply_bot_action.mockReturnValue(
      JSON.stringify({
        action: { type: 'InitialFlip', position: 3 },
        state: afterBotState,
      }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(makeConfig());
    });

    act(() => {
      result.current.applyBotTurn('Random');
    });

    expect(mockWasm.apply_bot_action).toHaveBeenCalledWith(1, 'Random');
    expect(result.current.gameState!.action_needed.player).toBe(1);
  });

  it('resetGame clears state back to setup', async () => {
    const initialState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: initialState }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(makeConfig());
    });
    expect(result.current.phase).toBe('initial_flips');

    act(() => {
      result.current.resetGame();
    });

    expect(result.current.phase).toBe('setup');
    expect(result.current.gameState).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.roundHistory).toEqual([]);
    expect(mockWasm.destroy_interactive_game).toHaveBeenCalledWith(1);
  });

  it('exportGame returns JSON save data', async () => {
    const config = makeConfig();
    const initialState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: initialState }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(config);
    });

    const json = result.current.exportGame();
    expect(json).not.toBeNull();
    const parsed = JSON.parse(json!);
    expect(parsed.config).toEqual(config);
    expect(parsed.actions).toEqual([]);
  });

  it('importGame parses JSON and restores state via WASM', async () => {
    const config = makeConfig();
    const initialState = makeGameState({
      type: 'ChooseDraw',
      player: 0,
      drawable_piles: [0],
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 2, state: initialState }),
    );
    mockWasm.apply_action.mockReturnValue(
      JSON.stringify({ state: initialState }),
    );

    const saveData = JSON.stringify({
      config,
      actions: [{ type: 'InitialFlip', position: 0 }],
    });

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.importGame(saveData);
    });

    await waitFor(() => {
      expect(result.current.phase).toBe('playing');
    });
    expect(mockWasm.create_interactive_game).toHaveBeenCalled();
  });

  it('phase transitions from initial_flips to playing', async () => {
    const flipsState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: flipsState }),
    );

    const playingState = makeGameState({
      type: 'ChooseDraw',
      player: 0,
      drawable_piles: [0],
    });
    mockWasm.apply_action.mockReturnValue(
      JSON.stringify({ state: playingState }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(makeConfig());
    });
    expect(result.current.phase).toBe('initial_flips');

    act(() => {
      result.current.applyAction({ type: 'InitialFlip', position: 0 });
    });
    expect(result.current.phase).toBe('playing');
  });

  it('captures round history when RoundOver action_needed', async () => {
    const flipsState = makeGameState({
      type: 'ChooseInitialFlips',
      player: 0,
      count: 2,
    });
    mockWasm.create_interactive_game.mockReturnValue(
      JSON.stringify({ game_id: 1, state: flipsState }),
    );

    const roundOverState: InteractiveGameState = {
      ...makeGameState({
        type: 'RoundOver',
        round_number: 0,
        round_scores: [25, 30],
        cumulative_scores: [25, 30],
        going_out_player: 0,
        end_of_round_clears: [],
      }),
    };
    mockWasm.apply_action.mockReturnValue(
      JSON.stringify({ state: roundOverState }),
    );

    const { result } = renderHook(() => useInteractiveGame());

    await act(async () => {
      result.current.createGame(makeConfig());
    });

    act(() => {
      result.current.applyAction({ type: 'DrawFromDeck' });
    });

    expect(result.current.phase).toBe('round_over');
    expect(result.current.roundHistory).toHaveLength(1);
    expect(result.current.roundHistory[0]).toEqual({
      roundNumber: 0,
      roundScores: [25, 30],
      cumulativeScores: [25, 30],
      goingOutPlayer: 0,
    });
  });
});
