import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import { AuthProvider, useAuth } from '@/contexts/auth-context';

// Helper component that displays auth state for testing
function AuthStateDisplay() {
  const auth = useAuth();
  return (
    <div>
      <span data-testid="loading">{String(auth.isLoading)}</span>
      <span data-testid="backend">{String(auth.backendAvailable)}</span>
      <span data-testid="setup">{String(auth.needsSetup)}</span>
      <span data-testid="authenticated">{String(auth.isAuthenticated)}</span>
    </div>
  );
}

function renderWithAuth(initialPath = '/') {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <AuthProvider>
        <Routes>
          <Route path="*" element={<AuthStateDisplay />} />
        </Routes>
      </AuthProvider>
    </MemoryRouter>,
  );
}

describe('AuthProvider — backend availability', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('sets backendAvailable=true when setup-status fetch succeeds', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        if (url.includes('setup-status')) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve({ needs_setup: false }),
          });
        }
        // refresh endpoint
        return Promise.resolve({ ok: false });
      }),
    );

    renderWithAuth();

    await waitFor(() => {
      expect(screen.getByTestId('loading')).toHaveTextContent('false');
    });

    expect(screen.getByTestId('backend')).toHaveTextContent('true');
    expect(screen.getByTestId('setup')).toHaveTextContent('false');
  });

  it('sets backendAvailable=false when setup-status fetch throws (network error)', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.reject(new TypeError('Failed to fetch'))),
    );

    renderWithAuth();

    await waitFor(() => {
      expect(screen.getByTestId('loading')).toHaveTextContent('false');
    });

    expect(screen.getByTestId('backend')).toHaveTextContent('false');
    expect(screen.getByTestId('setup')).toHaveTextContent('false');
  });

  it('sets needsSetup=true only when backend is reachable and returns needs_setup', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        if (url.includes('setup-status')) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve({ needs_setup: true }),
          });
        }
        return Promise.resolve({ ok: false });
      }),
    );

    renderWithAuth();

    await waitFor(() => {
      expect(screen.getByTestId('loading')).toHaveTextContent('false');
    });

    expect(screen.getByTestId('backend')).toHaveTextContent('true');
    expect(screen.getByTestId('setup')).toHaveTextContent('true');
  });

  it('sets backendAvailable=true even when setup-status returns non-ok (server reachable but error)', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        if (url.includes('setup-status')) {
          return Promise.resolve({ ok: false, status: 500 });
        }
        return Promise.resolve({ ok: false });
      }),
    );

    renderWithAuth();

    await waitFor(() => {
      expect(screen.getByTestId('loading')).toHaveTextContent('false');
    });

    // Server responded (even with error) — it's reachable
    expect(screen.getByTestId('backend')).toHaveTextContent('true');
    expect(screen.getByTestId('setup')).toHaveTextContent('false');
  });
});
