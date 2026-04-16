import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

// --- Mock auth context ---

const mockAuth = { accessToken: null as string | null };
vi.mock('@/contexts/auth-context', () => ({
  useAuth: () => mockAuth,
}));

// --- Mock WebSocket ---

type WsHandler = {
  onopen: (() => void) | null;
  onclose: ((event: { code: number; reason: string }) => void) | null;
  onerror: (() => void) | null;
  onmessage: ((event: { data: string }) => void) | null;
  close: ReturnType<typeof vi.fn>;
  send: ReturnType<typeof vi.fn>;
  readyState: number;
};

let mockWsInstances: WsHandler[] = [];

class MockWebSocket {
  static OPEN = 1;
  static CLOSED = 3;

  onopen: (() => void) | null = null;
  onclose: ((event: { code: number; reason: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  close = vi.fn();
  send = vi.fn();
  readyState = MockWebSocket.OPEN;

  constructor() {
    mockWsInstances.push(this);
  }
}

vi.stubGlobal('WebSocket', MockWebSocket);

// --- Mock fetch ---

let fetchResponses: Array<{ ok: boolean; status: number }> = [];

const mockFetch = vi.fn(() => {
  const resp = fetchResponses.shift() ?? { ok: true, status: 200 };
  return Promise.resolve({
    ok: resp.ok,
    status: resp.status,
    json: () => Promise.resolve({ valid: true }),
  });
});

vi.stubGlobal('fetch', mockFetch);

import { useOnlineGame } from '../use-online-game';

// --- Setup / Teardown ---

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  mockWsInstances = [];
  fetchResponses = [];
  mockAuth.accessToken = null;
  mockFetch.mockClear();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  // Re-stub for next test (beforeEach runs after afterEach)
  vi.stubGlobal('WebSocket', MockWebSocket);
  vi.stubGlobal('fetch', mockFetch);
});

// --- Tests ---

describe('useOnlineGame', () => {
  describe('session validation before WebSocket connect', () => {
    it('creates WebSocket when session validation succeeds', async () => {
      fetchResponses.push({ ok: true, status: 200 });

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'valid-token', 0),
      );

      expect(result.current.connectionStatus).toBe('connecting');

      // Wait for fetch + WS creation
      await waitFor(() => {
        expect(mockWsInstances.length).toBe(1);
      });
    });

    it('sets sessionExpired when session validation returns 401', async () => {
      fetchResponses.push({ ok: false, status: 401 });

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'stale-token', 0),
      );

      await waitFor(() => {
        expect(result.current.sessionExpired).toBe(true);
      });

      expect(result.current.connectionStatus).toBe('disconnected');
      // No WebSocket should have been created
      expect(mockWsInstances.length).toBe(0);
    });

    it('does not create WebSocket when session validation fails', async () => {
      fetchResponses.push({ ok: false, status: 401 });

      renderHook(() => useOnlineGame('ABCDEF', 'stale-token', 0));

      await waitFor(() => {
        expect(mockFetch).toHaveBeenCalledTimes(1);
      });

      // Should never attempt WS
      expect(mockWsInstances.length).toBe(0);
    });

    it('retries on non-401 validation error (e.g. 500) without setting sessionExpired', async () => {
      fetchResponses.push({ ok: false, status: 500 });
      fetchResponses.push({ ok: true, status: 200 });

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'valid-token', 0),
      );

      // Wait for first fetch to complete
      await waitFor(() => {
        expect(result.current.connectionStatus).toBe('disconnected');
      });

      // Should NOT set sessionExpired for 500
      expect(result.current.sessionExpired).toBe(false);
      expect(mockWsInstances.length).toBe(0);

      // Advance timer to trigger retry
      await act(async () => {
        vi.advanceTimersByTime(1500);
      });

      // Should have retried
      await waitFor(() => {
        expect(mockFetch).toHaveBeenCalledTimes(2);
      });
    });
  });

  describe('WS close code handling', () => {
    it('sets sessionExpired on WS close code 4001', async () => {
      fetchResponses.push({ ok: true, status: 200 });

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'valid-token', 0),
      );

      await waitFor(() => {
        expect(mockWsInstances.length).toBe(1);
      });

      // Simulate WS close with session expired code
      act(() => {
        mockWsInstances[0].onclose?.({ code: 4001, reason: 'session_expired' });
      });

      expect(result.current.sessionExpired).toBe(true);
      expect(result.current.connectionStatus).toBe('disconnected');
    });

    it('retries on normal WS close code 1006 (abnormal)', async () => {
      fetchResponses.push({ ok: true, status: 200 });
      // Second validation for reconnect
      fetchResponses.push({ ok: true, status: 200 });

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'valid-token', 0),
      );

      await waitFor(() => {
        expect(mockWsInstances.length).toBe(1);
      });

      // Simulate abnormal close
      act(() => {
        mockWsInstances[0].onclose?.({ code: 1006, reason: '' });
      });

      expect(result.current.sessionExpired).toBe(false);
      expect(result.current.connectionStatus).toBe('disconnected');

      // Advance timer to trigger reconnect
      await act(async () => {
        vi.advanceTimersByTime(1500);
      });

      // Should have attempted a second validation fetch
      await waitFor(() => {
        expect(mockFetch).toHaveBeenCalledTimes(2);
      });
    });

    it('does not set sessionExpired on normal WS close code 1000', async () => {
      fetchResponses.push({ ok: true, status: 200 });

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'valid-token', 0),
      );

      await waitFor(() => {
        expect(mockWsInstances.length).toBe(1);
      });

      act(() => {
        mockWsInstances[0].onopen?.();
      });

      act(() => {
        mockWsInstances[0].onclose?.({ code: 1000, reason: 'normal' });
      });

      // Normal close still retries (existing behavior) but never sets sessionExpired
      expect(result.current.sessionExpired).toBe(false);
      expect(result.current.connectionStatus).toBe('disconnected');
    });
  });

  describe('disconnect clears sessionExpired', () => {
    it('resets sessionExpired on disconnect', async () => {
      fetchResponses.push({ ok: false, status: 401 });

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'stale-token', 0),
      );

      await waitFor(() => {
        expect(result.current.sessionExpired).toBe(true);
      });

      act(() => {
        result.current.disconnect();
      });

      expect(result.current.sessionExpired).toBe(false);
    });
  });

  // --- Helper: connect and open WS ---

  async function connectWs() {
    fetchResponses.push({ ok: true, status: 200 });
    const hook = renderHook(() => useOnlineGame('ABCDEF', 'valid-token', 0));
    await waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];
    act(() => { ws.onopen?.(); });
    return { hook, ws };
  }

  function makeGameState(currentPlayer: number) {
    return {
      num_players: 2,
      player_names: ['Alice', 'Bob'],
      num_rows: 3,
      num_cols: 4,
      round_number: 0,
      current_player: currentPlayer,
      action_needed: { type: 'ChooseDraw', player: currentPlayer, drawable_piles: [0] },
      boards: [[{ Revealed: 5 }], [{ Revealed: 3 }]],
      discard_tops: [3],
      discard_sizes: [1],
      deck_remaining: 100,
      cumulative_scores: [0, 0],
      going_out_player: null,
      is_final_turn: false,
      last_column_clears: [],
    };
  }

  describe('deadlineKey increments on new deadlines (#26)', () => {
    it('deadlineKey starts at 0', async () => {
      const { hook } = await connectWs();
      expect(hook.result.current.deadlineKey).toBe(0);
    });

    it('increments deadlineKey on GameState with deadline', async () => {
      const { hook, ws } = await connectWs();

      act(() => {
        ws.onmessage?.({ data: JSON.stringify({
          type: 'GameState',
          state: makeGameState(0),
          turn_deadline_secs: 30,
        }) });
      });

      expect(hook.result.current.deadlineKey).toBe(1);
      expect(hook.result.current.turnDeadlineSecs).toBe(30);
    });

    it('does not increment deadlineKey when deadline is null', async () => {
      const { hook, ws } = await connectWs();

      act(() => {
        ws.onmessage?.({ data: JSON.stringify({
          type: 'GameState',
          state: makeGameState(0),
          turn_deadline_secs: null,
        }) });
      });

      expect(hook.result.current.deadlineKey).toBe(0);
      expect(hook.result.current.turnDeadlineSecs).toBeNull();
    });

    it('increments deadlineKey on TimeoutAction even with same deadline value', async () => {
      const { hook, ws } = await connectWs();

      // First: GameState sets deadline to 30
      act(() => {
        ws.onmessage?.({ data: JSON.stringify({
          type: 'GameState',
          state: makeGameState(0),
          turn_deadline_secs: 30,
        }) });
      });

      expect(hook.result.current.deadlineKey).toBe(1);

      // Timeout: same deadline value (30) — this is the bug scenario
      act(() => {
        ws.onmessage?.({ data: JSON.stringify({
          type: 'TimeoutAction',
          player: 0,
          action: { type: 'DrawFromDeck' },
          state: makeGameState(1),
          turn_deadline_secs: 30,
        }) });
      });

      // Key must increment even though value is still 30
      expect(hook.result.current.deadlineKey).toBe(2);
      expect(hook.result.current.turnDeadlineSecs).toBe(30);
      expect(hook.result.current.wasTimeout).toBe(true);
    });

    it('increments deadlineKey on ActionApplied', async () => {
      const { hook, ws } = await connectWs();

      act(() => {
        ws.onmessage?.({ data: JSON.stringify({
          type: 'ActionApplied',
          player: 0,
          action: { type: 'DrawFromDeck' },
          state: makeGameState(1),
          turn_deadline_secs: 30,
        }) });
      });

      expect(hook.result.current.deadlineKey).toBe(1);
    });

    it('increments deadlineKey on BotAction', async () => {
      const { hook, ws } = await connectWs();

      act(() => {
        ws.onmessage?.({ data: JSON.stringify({
          type: 'BotAction',
          player: 1,
          action: { type: 'DrawFromDeck' },
          state: makeGameState(0),
          turn_deadline_secs: 30,
        }) });
      });

      expect(hook.result.current.deadlineKey).toBe(1);
    });

    it('increments on each successive message (monotonic)', async () => {
      const { hook, ws } = await connectWs();

      // Simulate 3 turns all with deadline=30
      for (let i = 0; i < 3; i++) {
        act(() => {
          ws.onmessage?.({ data: JSON.stringify({
            type: 'ActionApplied',
            player: i % 2,
            action: { type: 'DrawFromDeck' },
            state: makeGameState((i + 1) % 2),
            turn_deadline_secs: 30,
          }) });
        });
      }

      expect(hook.result.current.deadlineKey).toBe(3);
    });
  });

  describe('network error on validation', () => {
    it('retries when validation fetch throws (network error)', async () => {
      // First call throws (network error), second succeeds
      mockFetch
        .mockRejectedValueOnce(new TypeError('Failed to fetch'))
        .mockResolvedValueOnce({
          ok: true,
          status: 200,
          json: () => Promise.resolve({ valid: true }),
        } as Response);

      const { result } = renderHook(() =>
        useOnlineGame('ABCDEF', 'valid-token', 0),
      );

      // Wait for first fetch to fail
      await waitFor(() => {
        expect(result.current.connectionStatus).toBe('disconnected');
      });

      // Should NOT set sessionExpired on network errors
      expect(result.current.sessionExpired).toBe(false);

      // Advance timer to trigger retry
      await act(async () => {
        vi.advanceTimersByTime(2000);
      });

      // Second fetch should happen
      await waitFor(() => {
        expect(mockFetch).toHaveBeenCalledTimes(2);
      });
    });
  });
});
