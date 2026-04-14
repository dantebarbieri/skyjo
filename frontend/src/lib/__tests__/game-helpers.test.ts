import { describe, it, expect } from 'vitest';
import type { VisibleSlot } from '@/types';
import { toSlot, getPlayerName, computeVisibleScore } from '../game-helpers';
import type { InteractiveGameState } from '@/types';

describe('toSlot', () => {
  it('converts "Hidden" to { Hidden: 0 }', () => {
    expect(toSlot('Hidden')).toEqual({ Hidden: 0 });
  });

  it('converts "Cleared" to "Cleared"', () => {
    expect(toSlot('Cleared')).toBe('Cleared');
  });

  it('converts a Revealed slot preserving the value', () => {
    expect(toSlot({ Revealed: 5 })).toEqual({ Revealed: 5 });
  });

  it('handles negative Revealed values', () => {
    expect(toSlot({ Revealed: -2 })).toEqual({ Revealed: -2 });
  });

  it('handles zero Revealed value', () => {
    expect(toSlot({ Revealed: 0 })).toEqual({ Revealed: 0 });
  });
});

describe('getPlayerName', () => {
  const makeState = (names: string[]): InteractiveGameState =>
    ({
      player_names: names,
    }) as unknown as InteractiveGameState;

  it('returns the player name at the given index', () => {
    const state = makeState(['Alice', 'Bob']);
    expect(getPlayerName(state, 0)).toBe('Alice');
    expect(getPlayerName(state, 1)).toBe('Bob');
  });

  it('returns fallback when index is out of bounds', () => {
    const state = makeState(['Alice']);
    expect(getPlayerName(state, 5)).toBe('Player 6');
  });

  it('returns fallback when name is an empty string', () => {
    const state = makeState(['']);
    expect(getPlayerName(state, 0)).toBe('Player 1');
  });
});

describe('computeVisibleScore', () => {
  it('returns 0 for an empty board', () => {
    expect(computeVisibleScore([])).toBe(0);
  });

  it('returns 0 when all slots are hidden', () => {
    const board: VisibleSlot[] = ['Hidden', 'Hidden', 'Hidden'];
    expect(computeVisibleScore(board)).toBe(0);
  });

  it('returns 0 when all slots are cleared', () => {
    const board: VisibleSlot[] = ['Cleared', 'Cleared'];
    expect(computeVisibleScore(board)).toBe(0);
  });

  it('sums only revealed card values', () => {
    const board: VisibleSlot[] = [
      { Revealed: 3 },
      'Hidden',
      { Revealed: 7 },
      'Cleared',
      { Revealed: -2 },
    ];
    expect(computeVisibleScore(board)).toBe(8);
  });

  it('handles all revealed cards', () => {
    const board: VisibleSlot[] = [
      { Revealed: 1 },
      { Revealed: 2 },
      { Revealed: 3 },
    ];
    expect(computeVisibleScore(board)).toBe(6);
  });

  it('handles negative values correctly', () => {
    const board: VisibleSlot[] = [
      { Revealed: -2 },
      { Revealed: -1 },
      { Revealed: 0 },
    ];
    expect(computeVisibleScore(board)).toBe(-3);
  });
});
