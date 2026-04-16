import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';

// --- Types ---

export type PermissionLevel = 'admin' | 'moderator' | 'user';

export type ConnectivityStatus =
  | 'online'
  | 'client-offline'
  | 'server-unreachable'
  | 'database-degraded';

export interface AuthUser {
  id: string;
  username: string;
  display_name: string;
  permission: PermissionLevel;
}

interface AuthState {
  user: AuthUser | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  needsSetup: boolean;
  registrationEnabled: boolean;
  /** Whether the backend server is reachable and database is healthy. Derived from connectivityStatus. */
  backendAvailable: boolean;
  /** Fine-grained connectivity status distinguishing client-offline, server-unreachable, and database-degraded. */
  connectivityStatus: ConnectivityStatus;
  /** Whether the database layer is healthy (server reachable + DB responding). */
  isDatabaseHealthy: boolean;
  /** Immediately re-check server connectivity. */
  retryConnection: () => void;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refresh: () => Promise<boolean>;
  accessToken: string | null;
}

const AuthContext = createContext<AuthState | null>(null);

const HEALTH_POLL_INTERVAL_MS = 20_000;

// --- Provider ---

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [needsSetup, setNeedsSetup] = useState(false);
  const [registrationEnabled, setRegistrationEnabled] = useState(false);
  const [connectivityStatus, setConnectivityStatus] = useState<ConnectivityStatus>('online');
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const healthPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const backendAvailable = connectivityStatus === 'online';
  const isDatabaseHealthy = connectivityStatus === 'online';

  const scheduleRefresh = useCallback((token: string) => {
    if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
    try {
      // JWT uses Base64URL encoding — convert to standard Base64 before decoding
      let b64 = token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/');
      while (b64.length % 4) b64 += '=';
      const payload = JSON.parse(atob(b64));
      const expiresIn = payload.exp * 1000 - Date.now();
      // Refresh 60 seconds before expiry
      const refreshIn = Math.max(expiresIn - 60_000, 5_000);
      refreshTimerRef.current = setTimeout(() => {
        refresh();
      }, refreshIn);
    } catch {
      // Invalid token format — don't schedule
    }
  }, []);

  const setAuth = useCallback((token: string, authUser: AuthUser) => {
    setAccessToken(token);
    setUser(authUser);
    scheduleRefresh(token);
  }, [scheduleRefresh]);

  const clearAuth = useCallback(() => {
    setAccessToken(null);
    setUser(null);
    if (refreshTimerRef.current) {
      clearTimeout(refreshTimerRef.current);
      refreshTimerRef.current = null;
    }
  }, []);

  const login = useCallback(async (username: string, password: string) => {
    const res = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
      credentials: 'same-origin',
    });

    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body?.error?.message || 'Login failed');
    }

    const data = await res.json();
    setAuth(data.access_token, data.user);
    setNeedsSetup(false);
  }, [setAuth]);

  const logout = useCallback(async () => {
    try {
      await fetch('/api/auth/logout', {
        method: 'POST',
        credentials: 'same-origin',
      });
    } catch {
      // Best-effort
    }
    clearAuth();
  }, [clearAuth]);

  const refresh = useCallback(async (): Promise<boolean> => {
    try {
      const res = await fetch('/api/auth/refresh', {
        method: 'POST',
        credentials: 'same-origin',
      });

      if (!res.ok) {
        clearAuth();
        return false;
      }

      const data = await res.json();
      setAuth(data.access_token, data.user);
      setNeedsSetup(false);
      return true;
    } catch {
      clearAuth();
      return false;
    }
  }, [setAuth, clearAuth]);

  /** Check /api/health to determine connectivity status, then optionally run full init. */
  const checkConnectivity = useCallback(async (runFullInit = false) => {
    // If the browser reports offline, short-circuit
    if (!navigator.onLine) {
      setConnectivityStatus('client-offline');
      return;
    }

    try {
      const healthRes = await fetch('/api/health');
      if (!healthRes.ok) {
        setConnectivityStatus('server-unreachable');
        return;
      }

      const health = await healthRes.json();
      if (health.database !== 'ok') {
        setConnectivityStatus('database-degraded');
        return;
      }

      // Server + DB are healthy
      setConnectivityStatus('online');

      if (runFullInit) {
        // Re-run setup status and session restoration
        try {
          const setupRes = await fetch('/api/auth/setup-status');
          if (setupRes.ok) {
            const { needs_setup, registration_enabled } = await setupRes.json();
            setNeedsSetup(needs_setup);
            setRegistrationEnabled(registration_enabled ?? false);
            if (!needs_setup) {
              await refresh();
            }
          }
        } catch {
          // Setup-status failed despite healthy health — may be a transient issue
        }
      }
    } catch (err) {
      // Network error — server unreachable or client offline
      if (err instanceof TypeError) {
        setConnectivityStatus(navigator.onLine ? 'server-unreachable' : 'client-offline');
      }
    }
  }, [refresh]);

  const retryConnection = useCallback(() => {
    checkConnectivity(true);
  }, [checkConnectivity]);

  // On mount: check setup status, then try to restore session
  useEffect(() => {
    async function init() {
      // Check connectivity first via /api/health
      if (!navigator.onLine) {
        setConnectivityStatus('client-offline');
        setIsLoading(false);
        return;
      }

      try {
        const healthRes = await fetch('/api/health');
        if (!healthRes.ok) {
          setConnectivityStatus('server-unreachable');
          setIsLoading(false);
          return;
        }

        const health = await healthRes.json();
        if (health.database !== 'ok') {
          setConnectivityStatus('database-degraded');
          setIsLoading(false);
          return;
        }

        setConnectivityStatus('online');
      } catch (err) {
        if (err instanceof TypeError) {
          setConnectivityStatus(navigator.onLine ? 'server-unreachable' : 'client-offline');
          setIsLoading(false);
          return;
        }
      }

      // Server + DB healthy — proceed with setup-status and session restore
      try {
        const setupRes = await fetch('/api/auth/setup-status');
        if (setupRes.ok) {
          const { needs_setup, registration_enabled } = await setupRes.json();
          setNeedsSetup(needs_setup);
          setRegistrationEnabled(registration_enabled ?? false);
          if (needs_setup) {
            setIsLoading(false);
            return;
          }
        }
      } catch {
        // Setup-status failed — database may have gone down between health and this call
        setConnectivityStatus('database-degraded');
        setIsLoading(false);
        return;
      }

      await refresh();
      setIsLoading(false);
    }
    init();
    return () => {
      if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
    };
  }, [refresh]);

  // Poll /api/health when not fully online
  useEffect(() => {
    if (connectivityStatus === 'online') {
      // Clear any existing poll
      if (healthPollRef.current) {
        clearInterval(healthPollRef.current);
        healthPollRef.current = null;
      }
      return;
    }

    // Start polling
    healthPollRef.current = setInterval(() => {
      checkConnectivity(true);
    }, HEALTH_POLL_INTERVAL_MS);

    return () => {
      if (healthPollRef.current) {
        clearInterval(healthPollRef.current);
        healthPollRef.current = null;
      }
    };
  }, [connectivityStatus, checkConnectivity]);

  // Listen for browser online/offline events
  useEffect(() => {
    const handleOnline = () => {
      // Browser came back online — re-check server health
      checkConnectivity(true);
    };
    const handleOffline = () => {
      setConnectivityStatus('client-offline');
    };

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);
    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, [checkConnectivity]);

  // Listen for backendUnreachable events from apiFetch
  useEffect(() => {
    const handleUnreachable = () => {
      setConnectivityStatus(prev =>
        prev === 'online'
          ? (navigator.onLine ? 'server-unreachable' : 'client-offline')
          : prev
      );
    };

    window.addEventListener('backendUnreachable', handleUnreachable);
    return () => {
      window.removeEventListener('backendUnreachable', handleUnreachable);
    };
  }, []);

  return (
    <AuthContext.Provider
      value={{
        user,
        isAuthenticated: user !== null,
        isLoading,
        needsSetup,
        registrationEnabled,
        backendAvailable,
        connectivityStatus,
        isDatabaseHealthy,
        retryConnection,
        login,
        logout,
        refresh,
        accessToken,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
