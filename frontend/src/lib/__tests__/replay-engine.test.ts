import { describe, it, expect } from 'vitest';
import {
  slotValue,
  slotToDisplay,
  computeKnownScore,
  computeTrueScore,
  buildAllSteps,
} from '../replay-engine';
import type { GameHistory, RoundHistory, Slot, TurnRecord, ColumnClearEvent } from '../../types';

// --- Helpers ---

/** Build a minimal single-round GameHistory for 2 players.
 *
 * Deck layout (column-major, 3 rows × 4 cols = 12 cards/player):
 *   Player 0 board: [1,2,3, 4,5,6, 7,8,9, 10,11,12]
 *   Player 1 board: [0,0,0, 1,1,1, 2,2,2, 3,3,3]
 *
 * initial_deck_order is constructed so that:
 *   - Remaining deck (top→bottom): 5, -2  (deck array = [-2, 5], pop gives 5 first)
 *   - First discard: 3
 *   - Player 1 hand: dealt_hands[1]
 *   - Player 0 hand: dealt_hands[0]  (last 12 cards popped)
 */
function makeBaseRound(overrides: Partial<RoundHistory> = {}): RoundHistory {
  const p0Hand = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
  const p1Hand = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
  // Deck order: [remainingDeck..., firstDiscard, p1Hand..., p0Hand...]
  const initialDeckOrder = [-2, 5, 3, ...p1Hand, ...p0Hand];

  return {
    round_number: 0,
    initial_deck_order: initialDeckOrder,
    dealt_hands: [p0Hand, p1Hand],
    setup_flips: [[0, 3], [1, 4]], // P0 flips positions 0,3; P1 flips 1,4
    starting_player: 0,
    turns: [],
    going_out_player: null,
    end_of_round_clears: [],
    round_scores: [78, 18],
    raw_round_scores: [78, 18],
    cumulative_scores: [78, 18],
    truncated: false,
    ...overrides,
  };
}

function makeHistory(roundOverrides: Partial<RoundHistory> = {}): GameHistory {
  return {
    seed: 42,
    num_players: 2,
    strategy_names: ['TestA', 'TestB'],
    rules_name: 'Standard',
    rounds: [makeBaseRound(roundOverrides)],
    final_scores: [78, 18],
    winners: [1],
  };
}

// --- Unit tests for slot helpers ---

describe('slotValue', () => {
  it('returns value from Hidden slot', () => {
    expect(slotValue({ Hidden: 7 })).toBe(7);
  });

  it('returns value from Revealed slot', () => {
    expect(slotValue({ Revealed: -2 })).toBe(-2);
  });

  it('returns null for Cleared slot', () => {
    expect(slotValue('Cleared')).toBeNull();
  });
});

describe('slotToDisplay', () => {
  it('returns "?" for Hidden slot', () => {
    expect(slotToDisplay({ Hidden: 5 })).toBe('?');
  });

  it('returns string value for Revealed slot', () => {
    expect(slotToDisplay({ Revealed: -1 })).toBe('-1');
    expect(slotToDisplay({ Revealed: 12 })).toBe('12');
  });

  it('returns empty string for Cleared slot', () => {
    expect(slotToDisplay('Cleared')).toBe('');
  });
});

// --- Score computation ---

describe('computeKnownScore', () => {
  it('sums only Revealed slots, ignoring Hidden and Cleared', () => {
    const board: Slot[] = [
      { Revealed: 5 },
      { Hidden: 10 },
      'Cleared',
      { Revealed: 3 },
    ];
    expect(computeKnownScore(board)).toBe(8);
  });

  it('handles negative Revealed values', () => {
    const board: Slot[] = [
      { Revealed: -2 },
      { Revealed: 5 },
      { Revealed: -1 },
      { Hidden: 12 },
    ];
    expect(computeKnownScore(board)).toBe(2);
  });
});

describe('computeTrueScore', () => {
  it('sums Hidden and Revealed values, ignoring Cleared', () => {
    const board: Slot[] = [
      { Hidden: 4 },
      { Revealed: 6 },
      'Cleared',
      { Hidden: -2 },
    ];
    expect(computeTrueScore(board)).toBe(8);
  });
});

// --- buildAllSteps ---

describe('buildAllSteps', () => {
  it('deal step has all-hidden boards and correct deck size', () => {
    const steps = buildAllSteps(makeHistory());
    const deal = steps[0];

    expect(deal.stepLabel).toBe('Deal');
    expect(deal.roundIndex).toBe(0);

    // All 12 slots hidden for each player
    for (const board of deal.state.boards) {
      expect(board).toHaveLength(12);
      board.forEach((slot) => {
        expect(slot).toHaveProperty('Hidden');
      });
    }

    // Remaining deck = first 2 elements of initial_deck_order ([-2, 5])
    expect(deal.state.deckSize).toBe(2);
    // Discard has the first discard card
    expect(deal.state.discardPiles[0]).toEqual([3]);
  });

  it('initial flips step reveals the flipped positions', () => {
    const steps = buildAllSteps(makeHistory());
    const flips = steps[1];

    expect(flips.stepLabel).toBe('Initial Flips');

    // Player 0 flipped positions 0 and 3
    const b0 = flips.state.boards[0];
    expect(b0[0]).toEqual({ Revealed: 1 });  // pos 0 was Hidden:1
    expect(b0[3]).toEqual({ Revealed: 4 });  // pos 3 was Hidden:4
    expect(b0[1]).toEqual({ Hidden: 2 });    // not flipped

    // Player 1 flipped positions 1 and 4
    const b1 = flips.state.boards[1];
    expect(b1[1]).toEqual({ Revealed: 0 });  // pos 1 was Hidden:0
    expect(b1[4]).toEqual({ Revealed: 1 });  // pos 4 was Hidden:1
    expect(b1[0]).toEqual({ Hidden: 0 });    // not flipped
  });

  it('deck draw + keep replaces board position and discards displaced card', () => {
    const turn: TurnRecord = {
      player_index: 0,
      action: {
        DrewFromDeck: {
          drawn_card: 5,   // top of deck (last in remaining = 5)
          action: { Keep: 6 }, // place at position 6 (col 2, row 0; was Hidden:7)
          displaced_card: 7,
        },
      },
      column_clears: [],
      went_out: false,
    };
    const steps = buildAllSteps(makeHistory({ turns: [turn] }));
    const turnStep = steps[2]; // Deal, Flips, Turn 1

    expect(turnStep.stepLabel).toBe('Turn 1 — Player 1');

    // Position 6 now has the drawn card revealed
    expect(turnStep.state.boards[0][6]).toEqual({ Revealed: 5 });
    // Displaced card (7) pushed to discard (on top of initial discard 3)
    expect(turnStep.state.discardPiles[0]).toEqual([3, 7]);
    // Deck shrunk by 1 (was 2, now 1)
    expect(turnStep.state.deckSize).toBe(1);
  });

  it('deck draw + discard-and-flip discards drawn card and flips hidden position', () => {
    const turn: TurnRecord = {
      player_index: 0,
      action: {
        DrewFromDeck: {
          drawn_card: 5,
          action: { DiscardAndFlip: 7 }, // flip position 7 (col 2, row 1; was Hidden:8)
          displaced_card: null,
        },
      },
      column_clears: [],
      went_out: false,
    };
    const steps = buildAllSteps(makeHistory({ turns: [turn] }));
    const turnStep = steps[2];

    // Drawn card (5) goes to discard
    expect(turnStep.state.discardPiles[0]).toEqual([3, 5]);
    // Position 7 is now revealed with its original hidden value
    expect(turnStep.state.boards[0][7]).toEqual({ Revealed: 8 });
    expect(turnStep.state.deckSize).toBe(1);
  });

  it('discard draw places card on board and discards displaced card', () => {
    const turn: TurnRecord = {
      player_index: 1,
      action: {
        DrewFromDiscard: {
          pile_index: 0,
          drawn_card: 3,   // takes the 3 from discard pile
          placement: 0,    // place at position 0 (col 0, row 0; was Hidden:0)
          displaced_card: 0,
        },
      },
      column_clears: [],
      went_out: false,
    };
    const steps = buildAllSteps(makeHistory({ turns: [turn] }));
    const turnStep = steps[2];

    expect(turnStep.stepLabel).toBe('Turn 1 — Player 2');

    // Drawn card placed on board
    expect(turnStep.state.boards[1][0]).toEqual({ Revealed: 3 });
    // Discard: initial [3] → pop 3 → push displaced 0 → [0]
    expect(turnStep.state.discardPiles[0]).toEqual([0]);
    // Deck unchanged
    expect(turnStep.state.deckSize).toBe(2);
  });

  it('column clear sets column to Cleared and pushes values to discard', () => {
    // Set up player 1's board so column 0 (positions 0,1,2) are all 0.
    // After initial flips, pos 1 is Revealed:0, rest are Hidden:0.
    // Turn: draw from deck, keep at pos 2 to reveal the 0 there... but that replaces with drawn card.
    // Instead: flip pos 0 via discard-and-flip, then flip pos 2 — but we only get one turn.
    // Easier: use a turn that places a 0 at pos 0, making col 0 = [Revealed:0, Revealed:0, Hidden:0].
    // Column clear happens after the turn action.
    // Let's have player 1 draw from discard (3), place at pos 5 (unrelated),
    // and declare a column clear on column 0.
    const turn: TurnRecord = {
      player_index: 1,
      action: {
        DrewFromDiscard: {
          pile_index: 0,
          drawn_card: 3,
          placement: 5,    // col 1, row 2
          displaced_card: 1,
        },
      },
      column_clears: [
        { player_index: 1, column: 0, card_value: 0, displaced_card: null },
      ],
      went_out: false,
    };
    const steps = buildAllSteps(makeHistory({ turns: [turn] }));
    const turnStep = steps[2];

    // Column 0 (positions 0, 1, 2) should all be Cleared
    const b1 = turnStep.state.boards[1];
    expect(b1[0]).toBe('Cleared');
    expect(b1[1]).toBe('Cleared');
    expect(b1[2]).toBe('Cleared');

    // Discard pile: started [3], pop 3 (discard draw), push displaced 1, then column clear pushes 0,0,0
    // After flips step: pos 1 is Revealed:0, others in col 0 are Hidden:0
    // slotValue of Hidden:0 = 0, Revealed:0 = 0 → all pushed
    expect(turnStep.state.discardPiles[0]).toContain(0);
    // Description mentions column cleared
    expect(turnStep.state.description).toContain('cleared');
  });

  it('round end step reveals all hidden cards and shows scores', () => {
    const steps = buildAllSteps(makeHistory());
    const roundEnd = steps[steps.length - 1];

    expect(roundEnd.stepLabel).toBe('Round End');

    // All slots should be Revealed (no Hidden remaining)
    for (const board of roundEnd.state.boards) {
      board.forEach((slot) => {
        expect(slot).not.toHaveProperty('Hidden');
      });
    }

    expect(roundEnd.state.roundScores).toEqual([78, 18]);
    expect(roundEnd.state.cumulativeScores).toEqual([78, 18]);
    expect(roundEnd.state.goingOutPlayer).toBeNull();
  });
});
