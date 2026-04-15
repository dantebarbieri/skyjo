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
