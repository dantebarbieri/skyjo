import type { VisibleSlot, Slot, InteractiveGameState } from '@/types';

/** Convert a VisibleSlot to a Slot for SkyjoCard rendering */
export function toSlot(vs: VisibleSlot): Slot {
  if (vs === 'Hidden') return { Hidden: 0 };
  if (vs === 'Cleared') return 'Cleared';
  return { Revealed: vs.Revealed };
}

export function getPlayerName(state: InteractiveGameState, index: number): string {
  return state.player_names[index] || `Player ${index + 1}`;
}

/** Compute the sum of all revealed card values on a board */
export function computeVisibleScore(board: VisibleSlot[]): number {
  let sum = 0;
  for (const slot of board) {
    if (typeof slot === 'object' && slot !== null && 'Revealed' in slot) {
      sum += slot.Revealed;
    }
  }
  return sum;
}
