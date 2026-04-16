/**
 * Centralized API client with automatic JWT token injection and refresh-on-401.
 */

/** Error thrown when a network request fails due to connectivity issues. */
export class NetworkError extends Error {
  constructor(message = 'Network error — server may be unreachable') {
    super(message);
    this.name = 'NetworkError';
  }
}

/** Type guard for NetworkError. */
export function isNetworkError(err: unknown): err is NetworkError {
  return err instanceof NetworkError;
}

let getAccessToken: (() => string | null) | null = null;
let refreshAuth: (() => Promise<boolean>) | null = null;
let onAuthFailure: (() => void) | null = null;

/**
 * Initialize the API client with auth callbacks.
 * Called once from the AuthProvider / App setup.
 */
export function initApiClient(
  tokenGetter: () => string | null,
  refresher: () => Promise<boolean>,
  authFailureHandler: () => void,
) {
  getAccessToken = tokenGetter;
  refreshAuth = refresher;
  onAuthFailure = authFailureHandler;
}

/**
 * Fetch wrapper that:
 * 1. Attaches Authorization: Bearer <token> if available
 * 2. On 401/403: attempts a silent token refresh and retries once
 * 3. On refresh failure: calls onAuthFailure (redirect to login)
 * 4. On network error (TypeError from fetch): throws NetworkError and fires
 *    a 'backendUnreachable' event on window so other parts of the app can react
 */
export async function apiFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  const token = getAccessToken?.();

  const headers = new Headers(init?.headers);
  if (token) {
    headers.set('Authorization', `Bearer ${token}`);
  }

  let response: Response;
  try {
    response = await fetch(input, {
      ...init,
      headers,
      credentials: 'same-origin',
    });
  } catch (err) {
    if (err instanceof TypeError) {
      window.dispatchEvent(new CustomEvent('backendUnreachable'));
      throw new NetworkError();
    }
    throw err;
  }

  // If unauthorized, try refreshing the token once
  if ((response.status === 401 || response.status === 403) && refreshAuth) {
    const refreshed = await refreshAuth();
    if (refreshed) {
      const newToken = getAccessToken?.();
      if (newToken) {
        headers.set('Authorization', `Bearer ${newToken}`);
        try {
          response = await fetch(input, {
            ...init,
            headers,
            credentials: 'same-origin',
          });
        } catch (err) {
          if (err instanceof TypeError) {
            window.dispatchEvent(new CustomEvent('backendUnreachable'));
            throw new NetworkError();
          }
          throw err;
        }
      }
    }

    // If still failing after refresh, trigger auth failure
    if (response.status === 401 || response.status === 403) {
      onAuthFailure?.();
    }
  }

  return response;
}
