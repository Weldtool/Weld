import { describe, expect, it, vi } from 'vitest';
import { ApiError, apiRequest } from './client';

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
