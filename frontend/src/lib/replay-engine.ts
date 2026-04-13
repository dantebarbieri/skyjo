import type {
  CardValue,
  Slot,
  GameHistory,
  RoundHistory,
  TurnRecord,
  ColumnClearEvent,
} from '../types';

// Standard Skyjo grid dimensions (from StandardRules)
const NUM_ROWS = 3;
const NUM_COLS = 4;

export interface ReplayState {
  boards: Slot[][];
  numRows: number;
  numCols: number;
  deckSize: number;
  /** The deck contents (top = last element). Exact after deal, approximate after reshuffle. */
  deck: CardValue[];
  discardPiles: CardValue[][];
  description: string;
  currentPlayer: number | null;
  roundScores: number[] | null;
  cumulativeScores: number[];
  goingOutPlayer: number | null;
}

export interface ReplayStep {
  state: ReplayState;
  roundIndex: number;
  stepLabel: string;
}

function cloneBoards(boards: Slot[][]): Slot[][] {
  return boards.map((b) => b.map((s) => (typeof s === 'string' ? s : { ...s })));
}

function cloneDiscardPiles(piles: CardValue[][]): CardValue[][] {
  return piles.map((p) => [...p]);
}

function cloneState(state: ReplayState): ReplayState {
  return {
    ...state,
    boards: cloneBoards(state.boards),
    deck: [...state.deck],
    discardPiles: cloneDiscardPiles(state.discardPiles),
    cumulativeScores: [...state.cumulativeScores],
    roundScores: state.roundScores ? [...state.roundScores] : null,
    goingOutPlayer: state.goingOutPlayer,
  };
}

export function slotValue(slot: Slot): CardValue | null {
  if (typeof slot === 'string') return null;
  if ('Hidden' in slot) return slot.Hidden;
  if ('Revealed' in slot) return slot.Revealed;
  return null;
}

export function computeKnownScore(board: Slot[]): number {
  let sum = 0;
  for (const slot of board) {
    if (typeof slot !== 'string' && 'Revealed' in slot) {
      sum += slot.Revealed;
    }
  }
  return sum;
}

export function computeTrueScore(board: Slot[]): number {
  let sum = 0;
  for (const slot of board) {
    const val = slotValue(slot);
    if (val !== null) sum += val;
  }
  return sum;
}

function applyColumnClears(
  state: ReplayState,
  clears: ColumnClearEvent[]
): void {
  for (const clear of clears) {
    const p = clear.player_index;
    for (let r = 0; r < state.numRows; r++) {
      const idx = clear.column * state.numRows + r;
      const slot = state.boards[p][idx];
      const val = slotValue(slot);
      if (val !== null) {
        state.discardPiles[0].push(val);
      }
      state.boards[p][idx] = 'Cleared';
    }
  }
}

function applyTurn(state: ReplayState, turn: TurnRecord): void {
  const p = turn.player_index;
  state.currentPlayer = p;

  if ('DrewFromDeck' in turn.action) {
    const { drawn_card, action, displaced_card } = turn.action.DrewFromDeck;

    if (state.deckSize === 0) {
      let reshuffledCount = 0;
      for (const pile of state.discardPiles) {
        reshuffledCount += pile.length;
        pile.length = 0;
      }
      state.deckSize = reshuffledCount - 1 - 1;
      state.deck = [];
    } else {
      state.deckSize--;
      state.deck.pop();
    }

    if ('Keep' in action) {
      const pos = action.Keep;
      state.boards[p][pos] = { Revealed: drawn_card };
      if (displaced_card !== null) {
        state.discardPiles[0].push(displaced_card);
      }
      state.description = `Player ${p + 1} drew ${drawn_card} from deck, placed at position ${pos}${displaced_card !== null ? `, discarding ${displaced_card}` : ''}.`;
    } else {
      const pos = (action as { DiscardAndFlip: number }).DiscardAndFlip;
      state.discardPiles[0].push(drawn_card);
      const slot = state.boards[p][pos];
      if (typeof slot !== 'string' && 'Hidden' in slot) {
        state.boards[p][pos] = { Revealed: slot.Hidden };
        state.description = `Player ${p + 1} drew ${drawn_card} from deck, discarded it, flipped position ${pos} (${slot.Hidden}).`;
      } else {
        state.description = `Player ${p + 1} drew ${drawn_card} from deck and discarded it.`;
      }
    }
  } else {
    const { pile_index, drawn_card, placement, displaced_card } =
      turn.action.DrewFromDiscard;
    state.discardPiles[pile_index].pop();
    state.boards[p][placement] = { Revealed: drawn_card };
    state.discardPiles[0].push(displaced_card);
    state.description = `Player ${p + 1} took ${drawn_card} from discard, placed at position ${placement}, discarding ${displaced_card}.`;
  }

  applyColumnClears(state, turn.column_clears);
  if (turn.column_clears.length > 0) {
    const cols = turn.column_clears.map((c) => c.column).join(', ');
    state.description += ` Column(s) ${cols} cleared!`;
  }

  if (turn.went_out) {
    state.description += ` Player ${p + 1} goes out!`;
  }
}

function buildRoundSteps(
  round: RoundHistory,
  numPlayers: number,
  prevCumulativeScores: number[]
): ReplayStep[] {
  const steps: ReplayStep[] = [];
  const ri = round.round_number;
  const cardsPerPlayer = NUM_ROWS * NUM_COLS;

  const boards: Slot[][] = round.dealt_hands.map((hand) =>
    hand.map((card) => ({ Hidden: card }) as Slot)
  );
  const totalPopped = numPlayers * cardsPerPlayer + 1;
  const remainingDeck = round.initial_deck_order.slice(
    0,
    round.initial_deck_order.length - totalPopped
  );
  const firstDiscard =
    round.initial_deck_order[round.initial_deck_order.length - totalPopped];
  const discardPiles: CardValue[][] = [[firstDiscard]];

  let state: ReplayState = {
    boards: cloneBoards(boards),
    numRows: NUM_ROWS,
    numCols: NUM_COLS,
    deckSize: remainingDeck.length,
    deck: [...remainingDeck],
    discardPiles: cloneDiscardPiles(discardPiles),
    description: 'Cards dealt. Each player has 12 hidden cards.',
    currentPlayer: null,
    roundScores: null,
    cumulativeScores: [...prevCumulativeScores],
    goingOutPlayer: null,
  };
  steps.push({
    state: cloneState(state),
    roundIndex: ri,
    stepLabel: 'Deal',
  });

  for (let p = 0; p < numPlayers; p++) {
    for (const pos of round.setup_flips[p]) {
      const slot = state.boards[p][pos];
      if (typeof slot !== 'string' && 'Hidden' in slot) {
        state.boards[p][pos] = { Revealed: slot.Hidden };
      }
    }
  }
  state.description = 'Each player flips 2 initial cards.';
  state.currentPlayer = null;
  steps.push({
    state: cloneState(state),
    roundIndex: ri,
    stepLabel: 'Initial Flips',
  });

  for (let t = 0; t < round.turns.length; t++) {
    const turn = round.turns[t];
    applyTurn(state, turn);
    steps.push({
      state: cloneState(state),
      roundIndex: ri,
      stepLabel: `Turn ${t + 1} — Player ${turn.player_index + 1}`,
    });
  }

  if (round.end_of_round_clears.length > 0 || round.round_scores) {
    for (let p = 0; p < numPlayers; p++) {
      for (let i = 0; i < state.boards[p].length; i++) {
        const slot = state.boards[p][i];
        if (typeof slot !== 'string' && 'Hidden' in slot) {
          state.boards[p][i] = { Revealed: slot.Hidden };
        }
      }
    }
    applyColumnClears(state, round.end_of_round_clears);
    state.description = 'Round over. All cards revealed.';
    state.currentPlayer = null;
    state.goingOutPlayer = round.going_out_player;
    state.roundScores = [...round.round_scores];
    state.cumulativeScores = [...round.cumulative_scores];
    steps.push({
      state: cloneState(state),
      roundIndex: ri,
      stepLabel: 'Round End',
    });
  }

  return steps;
}

export function buildAllSteps(history: GameHistory): ReplayStep[] {
  const allSteps: ReplayStep[] = [];
  let prevCumulative = new Array(history.num_players).fill(0);

  for (const round of history.rounds) {
    const roundSteps = buildRoundSteps(round, history.num_players, prevCumulative);
    allSteps.push(...roundSteps);
    prevCumulative = [...round.cumulative_scores];
  }

  return allSteps;
}

export function slotToDisplay(slot: Slot): string {
  if (typeof slot === 'string') return '';
  if ('Hidden' in slot) return '?';
  if ('Revealed' in slot) return String(slot.Revealed);
  return '';
}
