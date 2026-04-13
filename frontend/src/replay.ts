import type {
  CardValue,
  Slot,
  GameHistory,
  RoundHistory,
  TurnRecord,
  ColumnClearEvent,
} from './types';

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

function slotValue(slot: Slot): CardValue | null {
  if (typeof slot === 'string') return null;
  if ('Hidden' in slot) return slot.Hidden;
  if ('Revealed' in slot) return slot.Revealed;
  return null;
}

function computeKnownScore(board: Slot[]): number {
  let sum = 0;
  for (const slot of board) {
    if (typeof slot !== 'string' && 'Revealed' in slot) {
      sum += slot.Revealed;
    }
  }
  return sum;
}

function computeTrueScore(board: Slot[]): number {
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
        // In standard rules, cleared cards go to discard pile 0
        state.discardPiles[0].push(val);
      }
      state.boards[p][idx] = 'Cleared';
    }
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

  // Step: Deal
  // initial_deck_order is the full shuffled deck. Cards are popped from the end:
  // first dealt_hands (numPlayers * cardsPerPlayer cards), then first discard (1 card).
  // Remaining deck = everything before those pops.
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

  // Step: Setup flips
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

  // Steps: Each turn
  for (let t = 0; t < round.turns.length; t++) {
    const turn = round.turns[t];
    applyTurn(state, turn);
    steps.push({
      state: cloneState(state),
      roundIndex: ri,
      stepLabel: `Turn ${t + 1} — Player ${turn.player_index + 1}`,
    });
  }

  // Step: End of round (reveal all hidden, apply end-of-round clears)
  if (round.end_of_round_clears.length > 0 || round.round_scores) {
    // Reveal all hidden cards
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

function applyTurn(state: ReplayState, turn: TurnRecord): void {
  const p = turn.player_index;
  state.currentPlayer = p;

  if ('DrewFromDeck' in turn.action) {
    const { drawn_card, action, displaced_card } = turn.action.DrewFromDeck;

    // Handle deck reshuffle if deck is empty
    if (state.deckSize === 0) {
      // Reshuffle: all discard piles go into deck (unknown order)
      let reshuffledCount = 0;
      for (const pile of state.discardPiles) {
        reshuffledCount += pile.length;
        pile.length = 0;
      }
      // New discard top + drawn card come from reshuffled deck
      state.deckSize = reshuffledCount - 1 - 1;
      // We don't know the shuffle order, so clear the deck array
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

  // Column clears
  applyColumnClears(state, turn.column_clears);
  if (turn.column_clears.length > 0) {
    const cols = turn.column_clears.map((c) => c.column).join(', ');
    state.description += ` Column(s) ${cols} cleared!`;
  }

  if (turn.went_out) {
    state.description += ` Player ${p + 1} goes out!`;
  }
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

// Render helpers

function slotToDisplay(slot: Slot): string {
  if (typeof slot === 'string') return '';
  if ('Hidden' in slot) return '?';
  if ('Revealed' in slot) return String(slot.Revealed);
  return '';
}

function cardCssClass(v: CardValue): string {
  if (v < 0) return 'card negative';
  if (v === 0) return 'card zero';
  if (v <= 4) return 'card low';
  if (v <= 8) return 'card mid';
  return 'card high';
}

function slotCssClass(slot: Slot): string {
  if (typeof slot === 'string') return 'card cleared';
  if ('Hidden' in slot) return 'card hidden';
  const v = slot.Revealed;
  if (v < 0) return 'card negative';
  if (v === 0) return 'card zero';
  if (v <= 4) return 'card low';
  if (v <= 8) return 'card mid';
  return 'card high';
}

export function renderBoardGrid(
  board: Slot[],
  numRows: number,
  numCols: number
): HTMLElement {
  const grid = document.createElement('div');
  grid.className = 'board-grid';
  grid.style.gridTemplateColumns = `repeat(${numCols}, 1fr)`;

  // Convert column-major to row-major for display
  for (let r = 0; r < numRows; r++) {
    for (let c = 0; c < numCols; c++) {
      const idx = c * numRows + r;
      const slot = board[idx];
      const cell = document.createElement('div');
      cell.className = slotCssClass(slot);
      cell.textContent = slotToDisplay(slot);
      cell.title = `pos ${idx} (row ${r}, col ${c})`;
      grid.appendChild(cell);
    }
  }

  return grid;
}

export function renderReplayStep(
  container: HTMLElement,
  step: ReplayStep,
  strategyNames: string[]
): void {
  container.innerHTML = '';
  const { state } = step;

  const header = document.createElement('h3');
  header.textContent = `Round ${step.roundIndex + 1} — ${step.stepLabel}`;
  container.appendChild(header);

  const desc = document.createElement('p');
  desc.className = 'turn-description';
  desc.textContent = state.description;
  container.appendChild(desc);

  // Deck & discard pile display
  const pilesDiv = document.createElement('div');
  pilesDiv.className = 'piles-container';

  // Deck
  const deckDiv = document.createElement('div');
  deckDiv.className = 'pile-display';
  const deckLabel = document.createElement('div');
  deckLabel.className = 'pile-label';
  deckLabel.textContent = `Deck (${state.deckSize})`;
  deckDiv.appendChild(deckLabel);

  const deckCard = document.createElement('div');
  if (state.deck.length > 0) {
    const topVal = state.deck[state.deck.length - 1];
    deckCard.className = cardCssClass(topVal) + ' pile-card';
    deckCard.textContent = String(topVal);
    const hint = document.createElement('div');
    hint.className = 'hidden-hint';
    hint.textContent = 'hidden from players';
    deckDiv.appendChild(deckCard);
    deckDiv.appendChild(hint);
  } else if (state.deckSize > 0) {
    // After reshuffle — we don't know the exact top card
    deckCard.className = 'card hidden pile-card';
    deckCard.textContent = '?';
    const hint = document.createElement('div');
    hint.className = 'hidden-hint';
    hint.textContent = 'shuffled';
    deckDiv.appendChild(deckCard);
    deckDiv.appendChild(hint);
  } else {
    deckCard.className = 'card cleared pile-card';
    deckCard.textContent = '';
    deckDiv.appendChild(deckCard);
  }
  pilesDiv.appendChild(deckDiv);

  // Discard pile
  const discardDiv = document.createElement('div');
  discardDiv.className = 'pile-display';
  const discardLabel = document.createElement('div');
  discardLabel.className = 'pile-label';
  const discardSize = state.discardPiles[0].length;
  discardLabel.textContent = `Discard (${discardSize})`;
  discardDiv.appendChild(discardLabel);

  const discardCard = document.createElement('div');
  if (discardSize > 0) {
    const topVal = state.discardPiles[0][discardSize - 1];
    discardCard.className = cardCssClass(topVal) + ' pile-card';
    discardCard.textContent = String(topVal);
  } else {
    discardCard.className = 'card cleared pile-card';
    discardCard.textContent = '';
  }
  discardDiv.appendChild(discardCard);
  pilesDiv.appendChild(discardDiv);

  container.appendChild(pilesDiv);

  const boardsDiv = document.createElement('div');
  boardsDiv.className = 'boards-container';

  // Find lowest round score for bold highlighting at round end
  const isRoundEnd = state.roundScores !== null;
  let lowestRoundScore = Infinity;
  if (isRoundEnd) {
    lowestRoundScore = Math.min(...state.roundScores!);
  }

  for (let p = 0; p < state.boards.length; p++) {
    const playerDiv = document.createElement('div');
    playerDiv.className = 'player-board';
    if (state.currentPlayer === p) {
      playerDiv.classList.add('active-player');
    }

    // Going-out player highlighting at round end
    if (isRoundEnd && state.goingOutPlayer === p) {
      const roundScore = state.roundScores![p];
      const isSoloLowest = roundScore === lowestRoundScore
        && state.roundScores!.filter((s) => s === lowestRoundScore).length === 1;
      const wouldBePenalized = roundScore > 0 && !isSoloLowest;

      if (isSoloLowest) {
        playerDiv.classList.add('went-out-good');
      } else if (wouldBePenalized) {
        playerDiv.classList.add('went-out-penalized');
      } else {
        // Not penalized (score <= 0) but not the lowest
        playerDiv.classList.add('went-out-safe');
      }
    }

    const label = document.createElement('h4');
    label.textContent = `Player ${p + 1} (${strategyNames[p]})`;
    playerDiv.appendChild(label);

    const grid = renderBoardGrid(
      state.boards[p],
      state.numRows,
      state.numCols
    );
    playerDiv.appendChild(grid);

    const scoresDiv = document.createElement('div');
    scoresDiv.className = 'board-scores';
    const known = computeKnownScore(state.boards[p]);
    const true_ = computeTrueScore(state.boards[p]);
    if (known === true_) {
      // All cards revealed (or all cleared) — show single score
      let scoreText = `Score: ${known}`;
      if (isRoundEnd && state.goingOutPlayer === p && state.roundScores![p] !== known) {
        scoreText += ` (${state.roundScores![p]})`;
      }
      scoresDiv.textContent = scoreText;
    } else {
      scoresDiv.textContent = `Known: ${known} | True: ${true_}`;
    }
    if (isRoundEnd && state.roundScores![p] === lowestRoundScore) {
      scoresDiv.classList.add('lowest-score');
    }
    playerDiv.appendChild(scoresDiv);

    boardsDiv.appendChild(playerDiv);
  }

  container.appendChild(boardsDiv);
}
