import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import type { ReplayStep } from '@/lib/replay-engine';
import type { GameHistory } from '@/types';

// Build a minimal 2-round mock: round 0 has 4 steps, round 1 has 3 steps (7 total)
const mockSteps: ReplayStep[] = [
  { state: {} as ReplayStep['state'], roundIndex: 0, stepLabel: 'Deal' },
  { state: {} as ReplayStep['state'], roundIndex: 0, stepLabel: 'Flips' },
  { state: {} as ReplayStep['state'], roundIndex: 0, stepLabel: 'Turn 1' },
  { state: {} as ReplayStep['state'], roundIndex: 0, stepLabel: 'Round End' },
  { state: {} as ReplayStep['state'], roundIndex: 1, stepLabel: 'Deal' },
  { state: {} as ReplayStep['state'], roundIndex: 1, stepLabel: 'Flips' },
  { state: {} as ReplayStep['state'], roundIndex: 1, stepLabel: 'Turn 1' },
];

vi.mock('@/lib/replay-engine', () => ({
  buildAllSteps: vi.fn(() => mockSteps),
}));

import { useReplay } from '../use-replay';

const fakeHistory: GameHistory = {
  seed: 1,
  num_players: 2,
  strategy_names: ['Random', 'Random'],
  rules_name: 'Standard',
  rounds: [],
  final_scores: [10, 20],
  winners: [0],
};

describe('useReplay', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('has correct initial state', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    expect(result.current.currentStep).toBe(0);
    expect(result.current.playing).toBe(false);
    expect(result.current.speed).toBe(600);
    expect(result.current.totalSteps).toBe(7);
    expect(result.current.roundStarts).toEqual([0, 4]);
  });

  it('next() increments step', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    act(() => result.current.next());
    expect(result.current.currentStep).toBe(1);

    act(() => result.current.next());
    expect(result.current.currentStep).toBe(2);
  });

  it('prev() decrements step', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    // Move forward first
    act(() => result.current.next());
    act(() => result.current.next());
    expect(result.current.currentStep).toBe(2);

    act(() => result.current.prev());
    expect(result.current.currentStep).toBe(1);
  });

  it('next() at last step stays at last step', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    // Advance to the end
    for (let i = 0; i < 10; i++) {
      act(() => result.current.next());
    }
    expect(result.current.currentStep).toBe(6);

    act(() => result.current.next());
    expect(result.current.currentStep).toBe(6);
  });

  it('prev() at step 0 stays at 0', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    act(() => result.current.prev());
    expect(result.current.currentStep).toBe(0);
  });

  it('jumpToRound(1) jumps to first step of round 1', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    act(() => result.current.jumpToRound(1));
    expect(result.current.currentStep).toBe(4);
    expect(result.current.step.roundIndex).toBe(1);
  });

  it('skipToRoundStart() goes to start of current round', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    // Move into round 0, step 2
    act(() => result.current.next());
    act(() => result.current.next());
    expect(result.current.currentStep).toBe(2);

    act(() => result.current.skipToRoundStart());
    expect(result.current.currentStep).toBe(0);
  });

  it('skipToRoundEnd() goes to last step of current round', () => {
    const { result } = renderHook(() => useReplay(fakeHistory));

    // At step 0 (round 0), skip to end of round 0
    act(() => result.current.skipToRoundEnd());
    // Round 0 ends at step 3 (index before round 1 start at 4)
    expect(result.current.currentStep).toBe(3);
  });
});
