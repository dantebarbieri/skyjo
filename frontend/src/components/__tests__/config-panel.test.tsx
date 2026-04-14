import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import type { SimStatus } from '@/hooks/use-simulation';

// Mock rules-info (it calls into WASM)
vi.mock('../rules-info', () => ({
  default: ({ rulesName }: { rulesName: string }) => (
    <div data-testid="rules-info">{rulesName}</div>
  ),
}));

// Mock use-mouse-position (canvas/RAF dependency)
vi.mock('@/hooks/use-mouse-position', () => ({
  useMouseSubscription: vi.fn(),
}));

import ConfigPanel from '../config-panel';

const defaultStrategies = ['Random', 'Greedy', 'Conservative'];
const defaultRules = ['Standard', 'AuntJanet'];

function renderPanel(overrides: Partial<Parameters<typeof ConfigPanel>[0]> = {}) {
  const props = {
    strategies: defaultStrategies,
    rules: defaultRules,
    onStart: vi.fn(),
    simRunning: false,
    onPause: vi.fn(),
    onResume: vi.fn(),
    onStop: vi.fn(),
    simStatus: 'idle' as SimStatus,
    ...overrides,
  };
  const result = render(
    <MemoryRouter>
      <ConfigPanel {...props} />
    </MemoryRouter>,
  );
  return { ...result, props };
}

describe('ConfigPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Mock fetch for genetic saved generations
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve({ ok: false, json: () => Promise.resolve([]) })));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders without crashing', () => {
    renderPanel();
    expect(screen.getByText('Configuration')).toBeInTheDocument();
  });

  it('displays strategy selectors for each player', () => {
    renderPanel();
    // Default player count is 4
    expect(screen.getByText('Player 1:')).toBeInTheDocument();
    expect(screen.getByText('Player 2:')).toBeInTheDocument();
    expect(screen.getByText('Player 3:')).toBeInTheDocument();
    expect(screen.getByText('Player 4:')).toBeInTheDocument();
  });

  it('displays number of games input with default value', () => {
    renderPanel();
    const label = screen.getByText('Number of games');
    expect(label).toBeInTheDocument();
    const input = label.closest('.space-y-1\\.5')?.querySelector('input');
    expect(input).toHaveValue(100);
  });

  it('number of games input accepts changes', () => {
    renderPanel();
    const label = screen.getByText('Number of games');
    const input = label.closest('.space-y-1\\.5')!.querySelector('input')!;
    fireEvent.change(input, { target: { value: '500' } });
    expect(input).toHaveValue(500);
  });

  it('displays seed input with default value', () => {
    renderPanel();
    const label = screen.getByText('Seed');
    expect(label).toBeInTheDocument();
    const input = label.closest('.space-y-1\\.5')?.querySelector('input');
    expect(input).toHaveValue(42);
  });

  it('seed input accepts changes', () => {
    renderPanel();
    const label = screen.getByText('Seed');
    const input = label.closest('.space-y-1\\.5')!.querySelector('input')!;
    fireEvent.change(input, { target: { value: '123' } });
    expect(input).toHaveValue(123);
  });

  it('displays Run Simulation button', () => {
    renderPanel();
    expect(screen.getByText('Run Simulation')).toBeInTheDocument();
  });

  it('displays Run & Save Histories button', () => {
    renderPanel();
    expect(screen.getByText('Run & Save Histories')).toBeInTheDocument();
  });

  it('calls onStart when Run Simulation is clicked', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByText('Run Simulation'));
    expect(props.onStart).toHaveBeenCalledTimes(1);
    const config = vi.mocked(props.onStart).mock.calls[0][0];
    expect(config.num_games).toBe(100);
    expect(config.seed).toBe(42);
    expect(config.strategies).toHaveLength(4);
    expect(config.withHistories).toBe(false);
  });

  it('calls onStart with withHistories=true when Run & Save Histories is clicked', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByText('Run & Save Histories'));
    expect(props.onStart).toHaveBeenCalledTimes(1);
    expect(vi.mocked(props.onStart).mock.calls[0][0].withHistories).toBe(true);
  });

  it('disables start buttons when simulation is running', () => {
    renderPanel({ simRunning: true, simStatus: 'running' });
    expect(screen.getByText('Run Simulation')).toBeDisabled();
    expect(screen.getByText('Run & Save Histories')).toBeDisabled();
  });

  it('shows Pause and Stop buttons when simulation is running', () => {
    renderPanel({ simRunning: true, simStatus: 'running' });
    expect(screen.getByText('Pause')).toBeInTheDocument();
    expect(screen.getByText('Stop')).toBeInTheDocument();
  });

  it('shows Resume button when simulation is paused', () => {
    renderPanel({ simRunning: true, simStatus: 'paused' });
    expect(screen.getByText('Resume')).toBeInTheDocument();
  });

  it('calls onPause when Pause is clicked', () => {
    const { props } = renderPanel({ simRunning: true, simStatus: 'running' });
    fireEvent.click(screen.getByText('Pause'));
    expect(props.onPause).toHaveBeenCalledTimes(1);
  });

  it('calls onResume when Resume is clicked', () => {
    const { props } = renderPanel({ simRunning: true, simStatus: 'paused' });
    fireEvent.click(screen.getByText('Resume'));
    expect(props.onResume).toHaveBeenCalledTimes(1);
  });

  it('calls onStop when Stop is clicked', () => {
    const { props } = renderPanel({ simRunning: true, simStatus: 'running' });
    fireEvent.click(screen.getByText('Stop'));
    expect(props.onStop).toHaveBeenCalledTimes(1);
  });

  it('displays number of players label', () => {
    renderPanel();
    expect(screen.getByText('Number of players')).toBeInTheDocument();
  });

  it('displays max turns/round input', () => {
    renderPanel();
    const label = screen.getByText('Max turns/round');
    expect(label).toBeInTheDocument();
    const input = label.closest('.space-y-1\\.5')?.querySelector('input');
    expect(input).toHaveValue(10000);
  });

  it('displays Reset button', () => {
    renderPanel();
    expect(screen.getByText('Reset')).toBeInTheDocument();
  });

  it('displays Player Strategies fieldset', () => {
    renderPanel();
    expect(screen.getByText('Player Strategies')).toBeInTheDocument();
  });

  it('displays Apply to All button', () => {
    renderPanel();
    expect(screen.getByText('Apply to All')).toBeInTheDocument();
  });

  it('does not show Pause/Stop when simulation is idle', () => {
    renderPanel({ simRunning: false, simStatus: 'idle' });
    expect(screen.queryByText('Pause')).not.toBeInTheDocument();
    expect(screen.queryByText('Stop')).not.toBeInTheDocument();
  });
});
