import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { browser } from '$app/environment';
import { ApiError, apiRequest, type RequestOptions } from '$lib/api/client';
import type { LoginResponse, User } from '$lib/api/types';

const REFRESH_PATH = '/api/v1/auth/refresh';
const SESSION_KEY = 'weld_access_token';

class AuthStore {
	user = $state<User | null>(null);
	accessToken = $state<string | null>(null);
	/** True once the initial refresh attempt has completed (success or fail). */
	ready = $state(false);

	#refreshing: Promise<boolean> | null = null;
	#refreshCooldownUntil = 0;

	get isAuthenticated(): boolean {
		return this.user !== null && this.accessToken !== null;
	}

	/**
	 * Try to recover a session from the refresh-token cookie. Safe to call multiple
	 * times — concurrent calls share the same in-flight request.
	 */
	async bootstrap(): Promise<void> {
		// Fast path: try the token cached in sessionStorage from this tab.
		if (browser) {
			const stored = sessionStorage.getItem(SESSION_KEY);
			if (stored) {
				this.accessToken = stored;
				await this.#loadUser();
				if (this.user) {
					this.ready = true;
					return;
				}
				// Stored token was invalid/expired — clear it and fall through to cookie refresh.
				sessionStorage.removeItem(SESSION_KEY);
				this.accessToken = null;
			}
		}
		await this.#refresh();
		if (this.accessToken && !this.user) {
			await this.#loadUser();
		}
		this.ready = true;
	}

	async login(email: string, password: string): Promise<void> {
		const result = await apiRequest<LoginResponse>('/api/v1/auth/login', {
			method: 'POST',
			body: { email, password }
		});
		this.accessToken = result.access_token;
		this.#refreshCooldownUntil = 0;
		if (browser) sessionStorage.setItem(SESSION_KEY, result.access_token);
		await this.#loadUser();
	}

	/**
	 * Adopt a session minted elsewhere (e.g. study participant provisioning,
	 * which returns an access token and sets the refresh cookie). Mirrors the
	 * post-login state so the rest of the app treats the participant as logged in.
	 */
	async adoptSession(accessToken: string): Promise<void> {
		this.accessToken = accessToken;
		this.#refreshCooldownUntil = 0;
		if (browser) sessionStorage.setItem(SESSION_KEY, accessToken);
		await this.#loadUser();
		this.ready = true;
	}

	async register(email: string, password: string, displayName: string): Promise<void> {
		await apiRequest<User>('/api/v1/auth/register', {
			method: 'POST',
			body: { email, password, display_name: displayName }
		});
		await this.login(email, password);
	}

	async logout(): Promise<void> {
		try {
			await apiRequest<void>('/api/v1/auth/logout', { method: 'POST' });
		} catch {
			// Even if the server call fails, clear local state.
		}
		this.user = null;
		this.accessToken = null;
		if (browser) sessionStorage.removeItem(SESSION_KEY);
		await goto(resolve('/login'));
	}

	/**
	 * Authenticated request with one automatic refresh-and-retry on 401.
	 * Use this for any endpoint that requires a bearer token.
	 */
	async fetch<T>(path: string, opts: RequestOptions = {}): Promise<T> {
		const attempt = (token: string | null) =>
			apiRequest<T>(path, { ...opts, token: token ?? undefined });

		try {
			return await attempt(this.accessToken);
		} catch (err) {
			if (!(err instanceof ApiError) || err.status !== 401 || path === REFRESH_PATH) {
				throw err;
			}
			const refreshed = await this.#refresh();
			if (!refreshed) {
				this.user = null;
				this.accessToken = null;
				throw err;
			}
			return attempt(this.accessToken);
		}
	}

	/**
	 * Refresh the access token now if it expires within `minValidMs`. Normal
	 * requests don't need this — `fetch` refreshes on a 401 — but code that gets
	 * exactly one shot with no chance to retry (a keepalive request fired while
	 * the page unloads) needs a token that is already good.
	 */
	async ensureFresh(minValidMs: number): Promise<void> {
		const expiresAt = tokenExpiry(this.accessToken);
		if (expiresAt === null || expiresAt - Date.now() > minValidMs) return;
		await this.#refresh();
	}

	async #refresh(): Promise<boolean> {
		// If refresh failed recently, fast-fail so a 401 cascade can't generate
		// a storm of refresh attempts (the cookie itself might be unreachable
		// cross-site, or the token is gone server-side — retrying won't help).
		if (Date.now() < this.#refreshCooldownUntil) return false;
		this.#refreshing ??= (async () => {
			try {
				const result = await apiRequest<LoginResponse>(REFRESH_PATH, { method: 'POST' });
				this.accessToken = result.access_token;
				if (browser) sessionStorage.setItem(SESSION_KEY, result.access_token);
				this.#refreshCooldownUntil = 0;
				return true;
			} catch {
				this.accessToken = null;
				if (browser) sessionStorage.removeItem(SESSION_KEY);
				this.#refreshCooldownUntil = Date.now() + 5000;
				return false;
			} finally {
				this.#refreshing = null;
			}
		})();
		return this.#refreshing;
	}

	async #loadUser(): Promise<void> {
		try {
			this.user = await apiRequest<User>('/api/v1/users/me', {
				token: this.accessToken ?? undefined
			});
		} catch {
			this.user = null;
			this.accessToken = null;
		}
	}
}

/** Expiry of a JWT access token in epoch ms, or null if it can't be read. */
export function tokenExpiry(token: string | null): number | null {
	const payload = token?.split('.')[1];
	if (!payload) return null;
	try {
		const json = atob(payload.replace(/-/g, '+').replace(/_/g, '/'));
		const exp = (JSON.parse(json) as { exp?: unknown }).exp;
		return typeof exp === 'number' ? exp * 1000 : null;
	} catch {
		return null;
	}
}

export const auth = new AuthStore();
