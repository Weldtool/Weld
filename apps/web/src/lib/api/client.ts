/**
 * Typed fetch wrapper for the Weld backend.
 *
 * In dev, requests to `/api/*` are proxied to the Rust server (see vite.config.ts).
 * In production builds, set PUBLIC_API_BASE_URL to the deployed backend origin.
 */
import { env } from '$env/dynamic/public';

const API_BASE = (env.PUBLIC_API_BASE_URL ?? '').replace(/\/$/, '');

export class ApiError extends Error {
	constructor(
		public status: number,
		public body: unknown,
		message?: string
	) {
		super(message ?? `Request failed with status ${status}`);
		this.name = 'ApiError';
	}
}

/** Shown when the request never got an HTTP response at all. */
export const NETWORK_ERROR_MESSAGE =
	"Can't reach Weld right now. Check your connection and try again.";

/**
 * A message a user can act on for `status`. Used when the response carries no
 * message of its own — a proxy's HTML error page, an empty body — so the user
 * never sees raw markup, JSON or a bare status text.
 */
export function statusMessage(status: number): string {
	switch (status) {
		case 400:
		case 415:
		case 422:
			return 'Some of the information sent was missing or invalid. Please check it and try again.';
		case 401:
			return 'Your session has expired. Please sign in again.';
		case 403:
			return "You don't have permission to do that.";
		case 404:
			return "That couldn't be found. It may have been moved or deleted.";
		case 408:
			return 'The request took too long. Please try again.';
		case 409:
			return 'That conflicts with a change someone else just made. Please refresh and try again.';
		case 413:
			return 'That is too large to send.';
		case 429:
			return 'Too many attempts. Please wait a moment and try again.';
		case 502:
		case 503:
		case 504:
			return 'The service is temporarily unavailable. Please try again shortly.';
		default:
			return 'Something went wrong. Please try again.';
	}
}

/** Builds the `ApiError` for a failed response from its status and body text. */
export function errorFromResponse(status: number, text: string): ApiError {
	const parsed = text.length > 0 ? safeJsonParse(text) : undefined;
	const serverMessage =
		parsed && typeof parsed === 'object' && 'error' in parsed
			? (parsed as { error: unknown }).error
			: undefined;
	// The server writes `error` for users and never puts internal detail in it.
	// Anything else (HTML, plain text, an empty body) is replaced.
	const message =
		typeof serverMessage === 'string' && serverMessage.trim().length > 0
			? serverMessage
			: statusMessage(status);
	return new ApiError(status, parsed, message);
}

/**
 * The text to show a user for a caught error. API errors carry a message
 * written for them; anything else (a bug, a browser exception) has text meant
 * for developers, so the caller's `fallback` is shown instead.
 */
export function userMessage(err: unknown, fallback: string): string {
	return err instanceof ApiError ? err.message : fallback;
}

export interface RequestOptions {
	method?: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE';
	body?: unknown;
	token?: string;
	signal?: AbortSignal;
	fetch?: typeof fetch;
}

export async function apiRequest<T>(path: string, opts: RequestOptions = {}): Promise<T> {
	const fetchImpl = opts.fetch ?? globalThis.fetch;
	const headers: Record<string, string> = {};
	let body: BodyInit | undefined;

	if (opts.body !== undefined) {
		headers['Content-Type'] = 'application/json';
		body = JSON.stringify(opts.body);
	}
	if (opts.token) {
		headers['Authorization'] = `Bearer ${opts.token}`;
	}

	const url = `${API_BASE}${path.startsWith('/') ? path : `/${path}`}`;

	let response: Response;
	try {
		response = await fetchImpl(url, {
			method: opts.method ?? 'GET',
			credentials: 'include',
			headers,
			body,
			signal: opts.signal
		});
	} catch (err) {
		// A deliberate cancel is not a failure to report.
		if (err instanceof DOMException && err.name === 'AbortError') throw err;
		// Offline, DNS, CORS, server down: the browser's own text ("Failed to
		// fetch", "NetworkError when attempting to fetch resource") means
		// nothing to a user. Status 0 marks it as retryable.
		throw new ApiError(0, undefined, NETWORK_ERROR_MESSAGE);
	}

	const text = await response.text();

	if (!response.ok) throw errorFromResponse(response.status, text);

	return (text.length > 0 ? safeJsonParse(text) : undefined) as T;
}

function safeJsonParse(text: string): unknown {
	try {
		return JSON.parse(text);
	} catch {
		return text;
	}
}
