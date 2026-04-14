import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { Slot } from '@/types';
import type { ReplayState } from '@/lib/replay-engine';

// Mock use-mouse-position (used by SkyjoCard tilt effect)
vi.mock('@/hooks/use-mouse-position', () => ({
  useMouseSubscription: vi.fn(),
}));

import PlayerBoard from '../player-board';

// Standard 3 rows × 4 cols = 12 slots
const NUM_ROWS = 3;
const NUM_COLS = 4;

function makeBoard(slots: Slot[]): Slot[] {
  return slots;
}

function makeReplayState(overrides: Partial<ReplayState> = {}): ReplayState {
  return {
    boards: [],
    numRows: NUM_ROWS,
    numCols: NUM_COLS,
    deckSize: 100,
    deck: [],
    discardPiles: [[5]],
    description: 'Test state',
    currentPlayer: 0,
    roundScores: null,
    cumulativeScores: [0, 0],
    goingOutPlayer: null,
    ...overrides,
  };
}

function allRevealed(values: number[]): Slot[] {
  return values.map((v) => ({ Revealed: v }));
}

function allHidden(values: number[]): Slot[] {
  return values.map((v) => ({ Hidden: v }));
}

describe('PlayerBoard', () => {
  it('renders correct number of cards for a 3×4 board', () => {
    const board = allRevealed([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    const { container } = render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={0}
        strategyName="Random"
        state={makeReplayState()}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    // Grid should contain 12 card elements
    const grid = container.querySelector('.grid.gap-1');
    expect(grid).toBeTruthy();
    expect(grid!.children).toHaveLength(12);
  });

  it('displays card values for revealed cards', () => {
    const board = allRevealed([5, -2, 0, 12, 5, -2, 0, 12, 5, -2, 0, 12]);
    render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={0}
        strategyName="Greedy"
        state={makeReplayState()}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    // Revealed cards show their value as text content (center + 2 corners = 3 per card)
    // Check that specific values appear
    expect(screen.getAllByText('5').length).toBeGreaterThan(0);
    expect(screen.getAllByText('-2').length).toBeGreaterThan(0);
    expect(screen.getAllByText('12').length).toBeGreaterThan(0);
    expect(screen.getAllByText('0').length).toBeGreaterThan(0);
  });

  it('renders hidden cards with SKYJO text', () => {
    const board = allHidden([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={0}
        strategyName="Random"
        state={makeReplayState()}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    // Hidden cards show "SKYJO" text
    const skyjoTexts = screen.getAllByText('SKYJO');
    expect(skyjoTexts).toHaveLength(12);
  });

  it('renders a mix of hidden, revealed, and cleared slots', () => {
    const board: Slot[] = [
      { Hidden: 1 },
      { Revealed: 5 },
      'Cleared',
      { Revealed: 3 },
      { Hidden: 7 },
      { Revealed: -1 },
      'Cleared',
      { Hidden: 9 },
      { Revealed: 10 },
      { Hidden: 2 },
      { Revealed: 0 },
      { Hidden: 4 },
    ];
    const { container } = render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={0}
        strategyName="Conservative"
        state={makeReplayState()}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    const grid = container.querySelector('.grid.gap-1');
    expect(grid!.children).toHaveLength(12);
    // Revealed cards show values
    expect(screen.getAllByText('5').length).toBeGreaterThan(0);
    expect(screen.getAllByText('10').length).toBeGreaterThan(0);
    // Hidden cards show SKYJO
    expect(screen.getAllByText('SKYJO').length).toBe(5); // 5 hidden cards
  });

  it('displays player name and strategy', () => {
    const board = allHidden([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={2}
        strategyName="Greedy"
        state={makeReplayState({ currentPlayer: 0 })}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    expect(screen.getByText('Player 3')).toBeInTheDocument();
    expect(screen.getByText('(Greedy)')).toBeInTheDocument();
  });

  it('displays score information', () => {
    // All revealed, so known === true
    const board = allRevealed([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={0}
        strategyName="Random"
        state={makeReplayState()}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    // Sum of 1..12 = 78
    expect(screen.getByText('Score: 78')).toBeInTheDocument();
  });

  it('shows known vs true score when some cards are hidden', () => {
    // Mix: revealed sum = 5+3 = 8, true sum includes hidden values too
    const board: Slot[] = [
      { Revealed: 5 },
      { Hidden: 10 },
      { Revealed: 3 },
      { Hidden: 2 },
      { Hidden: 1 },
      { Hidden: 4 },
      { Hidden: 6 },
      { Hidden: 7 },
      { Hidden: 8 },
      { Hidden: 9 },
      { Hidden: 11 },
      { Hidden: 12 },
    ];
    render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={0}
        strategyName="Random"
        state={makeReplayState()}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    // known = 8, true = 5+10+3+2+1+4+6+7+8+9+11+12 = 78
    expect(screen.getByText('Known: 8 | True: 78')).toBeInTheDocument();
  });

  it('highlights active player with blue border', () => {
    const board = allHidden([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    const { container } = render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={0}
        strategyName="Random"
        state={makeReplayState({ currentPlayer: 0 })}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    const wrapper = container.firstElementChild!;
    expect(wrapper.className).toContain('border-blue-500');
  });

  it('does not highlight non-active player', () => {
    const board = allHidden([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    const { container } = render(
      <PlayerBoard
        board={board}
        numRows={NUM_ROWS}
        numCols={NUM_COLS}
        playerIndex={1}
        strategyName="Random"
        state={makeReplayState({ currentPlayer: 0 })}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    const wrapper = container.firstElementChild!;
    expect(wrapper.className).not.toContain('border-blue-500');
  });

  it('renders with different grid dimensions', () => {
    // 2 rows × 3 cols = 6 slots
    const board = allRevealed([1, 2, 3, 4, 5, 6]);
    const { container } = render(
      <PlayerBoard
        board={board}
        numRows={2}
        numCols={3}
        playerIndex={0}
        strategyName="Random"
        state={makeReplayState({ numRows: 2, numCols: 3 })}
        isRoundEnd={false}
        lowestRoundScore={0}
      />,
    );
    const grid = container.querySelector('.grid.gap-1');
    expect(grid!.children).toHaveLength(6);
  });
});
