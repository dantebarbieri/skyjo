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
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refresh: () => Promise<boolean>;
  accessToken: string | null;
}

const AuthContext = createContext<AuthState | null>(null);

// --- Provider ---

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [needsSetup, setNeedsSetup] = useState(false);
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  // On mount: check setup status, then try to restore session
  useEffect(() => {
    async function init() {
      try {
        const setupRes = await fetch('/api/auth/setup-status');
        if (setupRes.ok) {
          const { needs_setup } = await setupRes.json();
          setNeedsSetup(needs_setup);
          if (needs_setup) {
            setIsLoading(false);
            return;
          }
        }
      } catch {
        // Server may be unreachable — proceed without setup check
      }
      await refresh();
      setIsLoading(false);
    }
    init();
    return () => {
      if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <AuthContext.Provider
      value={{
        user,
        isAuthenticated: user !== null,
        isLoading,
        needsSetup,
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
