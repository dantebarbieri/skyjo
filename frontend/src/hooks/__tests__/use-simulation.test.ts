import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import type { SimConfig, ProgressStats, CacheEntry } from '@/types';

// --- Mocks ---

class MockWorker {
  onmessage: ((e: MessageEvent) => void) | null = null;
  onerror: ((e: ErrorEvent) => void) | null = null;
  postMessage = vi.fn();
  terminate = vi.fn();
}

let lastWorker: MockWorker;

vi.stubGlobal('Worker', vi.fn(function () {
  lastWorker = new MockWorker();
  return lastWorker;
}));

vi.mock('@/cache', () => ({
  saveCacheEntry: vi.fn(),
  getCacheEntry: vi.fn(),
  getCacheHistories: vi.fn(() => []),
}));

// Mock URL.createObjectURL / revokeObjectURL (used by exportResult)
URL.createObjectURL = vi.fn(() => 'blob:mock');
URL.revokeObjectURL = vi.fn();

import { useSimulation } from '../use-simulation';

const makeConfig = (overrides?: Partial<SimConfig>): SimConfig => ({
  num_games: 100,
  seed: 42,
  strategies: ['Random', 'Greedy'],
  rules: 'Standard',
  withHistories: false,
  realtimeVisualization: false,
  maxTurnsPerRound: 200,
  ...overrides,
});

const makeStats = (): ProgressStats => ({
  num_games: 50,
  num_players: 2,
  wins_per_player: [30, 20],
  win_rate_per_player: [0.6, 0.4],
  avg_score_per_player: [45, 55],
  min_score_per_player: [10, 15],
  max_score_per_player: [80, 90],
  avg_rounds_per_game: 3,
  avg_turns_per_game: 18,
});

function simulateWorkerMessage(msg: unknown) {
  lastWorker.onmessage?.({ data: msg } as MessageEvent);
}

describe('useSimulation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('has correct initial state', () => {
    const { result } = renderHook(() => useSimulation());

    expect(result.current.status).toBe('idle');
    expect(result.current.stats).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.gamesCompleted).toBe(0);
    expect(result.current.histories).toBeNull();
  });

  it('start() sets status to running and creates a Worker', () => {
    const { result } = renderHook(() => useSimulation());
    const cfg = makeConfig();

    act(() => result.current.start(cfg));

    expect(result.current.status).toBe('running');
    expect(Worker).toHaveBeenCalled();
  });

  it('Worker "ready" message triggers postMessage with start config', () => {
    const { result } = renderHook(() => useSimulation());
    const cfg = makeConfig();

    act(() => result.current.start(cfg));
    act(() => simulateWorkerMessage({ type: 'ready' }));

    expect(lastWorker.postMessage).toHaveBeenCalledWith({
      type: 'start',
      config: cfg,
    });
  });

  it('Worker "progress" message updates stats', () => {
    const { result } = renderHook(() => useSimulation());
    const cfg = makeConfig();
    const stats = makeStats();

    act(() => result.current.start(cfg));
    act(() =>
      simulateWorkerMessage({
        type: 'progress',
        stats,
        gamesCompleted: 50,
        totalGames: 100,
        elapsedMs: 1234,
      }),
    );

    expect(result.current.stats).toEqual(stats);
    expect(result.current.gamesCompleted).toBe(50);
    expect(result.current.elapsedMs).toBe(1234);
  });

  it('Worker "complete" message sets status to complete', () => {
    const { result } = renderHook(() => useSimulation());
    const cfg = makeConfig();
    const stats = makeStats();

    act(() => result.current.start(cfg));
    act(() =>
      simulateWorkerMessage({
        type: 'complete',
        stats,
        gamesCompleted: 100,
        totalGames: 100,
        elapsedMs: 5000,
        histories: null,
      }),
    );

    expect(result.current.status).toBe('complete');
    expect(result.current.stats).toEqual(stats);
    expect(result.current.gamesCompleted).toBe(100);
  });

  it('Worker "error" message sets error state and status to idle', () => {
    const { result } = renderHook(() => useSimulation());
    const cfg = makeConfig();

    act(() => result.current.start(cfg));
    act(() =>
      simulateWorkerMessage({ type: 'error', message: 'Something broke' }),
    );

    expect(result.current.status).toBe('idle');
    expect(result.current.error).toBe('Something broke');
  });

  it('pause() and resume() send correct messages to worker', () => {
    const { result } = renderHook(() => useSimulation());
    const cfg = makeConfig();

    act(() => result.current.start(cfg));

    act(() => result.current.pause());
    expect(result.current.status).toBe('paused');
    expect(lastWorker.postMessage).toHaveBeenCalledWith({ type: 'pause' });

    act(() => result.current.resume());
    expect(result.current.status).toBe('running');
    expect(lastWorker.postMessage).toHaveBeenCalledWith({ type: 'resume' });
  });

  it('loadFromCache() sets status to cached and populates stats', () => {
    const { result } = renderHook(() => useSimulation());
    const stats = makeStats();

    const cached: CacheEntry = {
      version: 1,
      key: 'test-key',
      config: makeConfig(),
      stats,
      gamesCompleted: 100,
      totalGames: 100,
      elapsedMs: 3000,
      hasHistories: false,
      savedAt: Date.now(),
    };

    act(() => result.current.loadFromCache(cached));

    expect(result.current.status).toBe('cached');
    expect(result.current.stats).toEqual(stats);
    expect(result.current.gamesCompleted).toBe(100);
    expect(result.current.elapsedMs).toBe(3000);
  });
});
