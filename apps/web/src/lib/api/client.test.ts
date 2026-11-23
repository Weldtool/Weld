import { describe, expect, it, vi } from 'vitest';
import {
	ApiError,
	NETWORK_ERROR_MESSAGE,
	apiRequest,
	errorFromResponse,
	statusMessage,
	userMessage
} from './client';

function mockFetch(status: number, body: unknown): typeof fetch {
	const text = typeof body === 'string' ? body : JSON.stringify(body);
	return vi.fn(async () =>
		Promise.resolve(
			new Response(text, {
				status,
				headers: { 'content-type': 'application/json' }
			})
		)
	) as unknown as typeof fetch;
}

describe('apiRequest', () => {
	it('parses JSON success responses', async () => {
		const fetchImpl = mockFetch(200, { hello: 'world' });
		const result = await apiRequest<{ hello: string }>('/api/v1/test', { fetch: fetchImpl });
		expect(result).toEqual({ hello: 'world' });
	});

	it('throws ApiError with the server error message on failure', async () => {
		const fetchImpl = mockFetch(403, { error: 'Insufficient team permissions' });
		await expect(apiRequest('/api/v1/test', { fetch: fetchImpl })).rejects.toMatchObject({
			status: 403,
			message: 'Insufficient team permissions'
		});
	});

	it('attaches the bearer token when provided', async () => {
		const fetchImpl = vi.fn(async (_url: unknown, init: RequestInit) => {
			expect((init.headers as Record<string, string>).Authorization).toBe('Bearer xyz');
			return new Response('{}', { status: 200 });
		});
		await apiRequest('/api/v1/users/me', {
			token: 'xyz',
			fetch: fetchImpl as unknown as typeof fetch
		});
	});

	it('serializes JSON bodies on POST', async () => {
		const fetchImpl = vi.fn(async (_url: unknown, init: RequestInit) => {
			expect(init.method).toBe('POST');
			expect(init.body).toBe(JSON.stringify({ name: 'Acme' }));
			return new Response('{}', { status: 201 });
		});
		await apiRequest('/api/v1/teams', {
			method: 'POST',
			body: { name: 'Acme' },
			fetch: fetchImpl as unknown as typeof fetch
		});
	});

	it('exports an ApiError that preserves status and parsed body', () => {
		const err = new ApiError(404, { error: 'not found' }, 'not found');
		expect(err.status).toBe(404);
		expect(err.body).toEqual({ error: 'not found' });
	});
});

describe('error messages shown to users', () => {
	it('replaces a non-JSON error body with a message for the status', async () => {
		const fetchImpl = vi.fn(async () =>
			Promise.resolve(
				new Response('<html><body>502 Bad Gateway</body></html>', {
					status: 502,
					headers: { 'content-type': 'text/html' }
				})
			)
		) as unknown as typeof fetch;
		const err = await apiRequest('/api/v1/test', { fetch: fetchImpl }).catch((e: unknown) => e);
		expect(err).toBeInstanceOf(ApiError);
		expect((err as ApiError).message).toBe(statusMessage(502));
		expect((err as ApiError).message).not.toContain('<');
	});

	it('uses the status message when the body is empty or has no error text', () => {
		expect(errorFromResponse(404, '').message).toBe(statusMessage(404));
		expect(errorFromResponse(400, '{"error":""}').message).toBe(statusMessage(400));
		expect(errorFromResponse(422, 'Failed to deserialize the JSON body').message).toBe(
			statusMessage(422)
		);
	});

	it('turns a network failure into a retryable, readable error', async () => {
		const fetchImpl = vi.fn(async () => {
			throw new TypeError('Failed to fetch');
		}) as unknown as typeof fetch;
		await expect(apiRequest('/api/v1/test', { fetch: fetchImpl })).rejects.toMatchObject({
			status: 0,
			message: NETWORK_ERROR_MESSAGE
		});
	});

	it('lets a deliberate abort through untouched', async () => {
		const fetchImpl = vi.fn(async () => {
			throw new DOMException('aborted', 'AbortError');
		}) as unknown as typeof fetch;
		await expect(apiRequest('/api/v1/test', { fetch: fetchImpl })).rejects.toMatchObject({
			name: 'AbortError'
		});
	});

	it('shows API messages but never the text of an unexpected exception', () => {
		expect(userMessage(new ApiError(409, undefined, 'Email is already registered'), 'x')).toBe(
			'Email is already registered'
		);
		expect(
			userMessage(new TypeError("Cannot read properties of undefined (reading 'id')"), 'Oops.')
		).toBe('Oops.');
		expect(userMessage('weird', 'Oops.')).toBe('Oops.');
	});
});
