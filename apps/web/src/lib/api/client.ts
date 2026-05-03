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

	const response = await fetchImpl(url, {
		method: opts.method ?? 'GET',
		credentials: 'include',
		headers,
		body,
		signal: opts.signal
	});

	const text = await response.text();
	const parsed = text.length > 0 ? safeJsonParse(text) : undefined;

	if (!response.ok) {
		const message =
			(parsed && typeof parsed === 'object' && 'error' in parsed
				? String((parsed as { error: unknown }).error)
				: undefined) ?? response.statusText;
		throw new ApiError(response.status, parsed, message);
	}

	return parsed as T;
}

function safeJsonParse(text: string): unknown {
	try {
		return JSON.parse(text);
	} catch {
		return text;
	}
}
