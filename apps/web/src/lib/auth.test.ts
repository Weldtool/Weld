import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({
	goto: vi.fn().mockResolvedValue(undefined)
}));

import { auth } from './auth.svelte';

interface MockResponse {
	status: number;
	body: unknown;
}

function jsonResponse({ status, body }: MockResponse): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'content-type': 'application/json' }
	});
}

function mockSequence(...responses: MockResponse[]) {
	const queue = [...responses];
	const fn = vi.fn(async () => {
		const next = queue.shift();
		if (!next) throw new Error('Unexpected fetch call — no more mocked responses');
		return jsonResponse(next);
	});
	vi.stubGlobal('fetch', fn);
	return fn;
}

beforeEach(() => {
	auth.user = null;
	auth.accessToken = null;
	auth.ready = false;
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.clearAllMocks();
});

describe('auth store', () => {
	it('login stores token and user, marking session authenticated', async () => {
		const fetchSpy = mockSequence(
			{ status: 200, body: { access_token: 'abc', token_type: 'Bearer' } },
			{
				status: 200,
				body: {
					id: 'u-1',
					email: 'a@b.com',
					display_name: 'A',
					avatar_url: null,
					role: 'member',
					email_verified: false,
					created_at: '',
					updated_at: ''
				}
			}
		);

		await auth.login('a@b.com', 'secure_password_123');

		expect(auth.accessToken).toBe('abc');
		expect(auth.user?.email).toBe('a@b.com');
		expect(auth.isAuthenticated).toBe(true);
		expect(fetchSpy).toHaveBeenCalledTimes(2);
	});

	it('fetch retries once after a 401 by refreshing the access token', async () => {
		auth.accessToken = 'stale';
		auth.user = {
			id: 'u-1',
			email: 'a@b.com',
			display_name: 'A',
			avatar_url: null,
			role: 'member',
			email_verified: false,
			created_at: '',
			updated_at: ''
		};

		const fetchSpy = mockSequence(
			{ status: 401, body: { error: 'expired' } },
			{ status: 200, body: { access_token: 'fresh', token_type: 'Bearer' } },
			{ status: 200, body: { ok: true } }
		);

		const result = await auth.fetch<{ ok: boolean }>('/api/v1/users/me');

		expect(result).toEqual({ ok: true });
		expect(auth.accessToken).toBe('fresh');
		expect(fetchSpy).toHaveBeenCalledTimes(3);

		const calls = fetchSpy.mock.calls as unknown as Array<[string, RequestInit]>;
		expect(calls[1][0]).toContain('/api/v1/auth/refresh');
		expect(calls[2][1].headers).toMatchObject({ Authorization: 'Bearer fresh' });
	});

	it('fetch surfaces 401 and clears state when refresh fails', async () => {
		auth.accessToken = 'stale';
		mockSequence(
			{ status: 401, body: { error: 'expired' } },
			{ status: 401, body: { error: 'invalid refresh' } }
		);

		await expect(auth.fetch('/api/v1/users/me')).rejects.toMatchObject({ status: 401 });
		expect(auth.accessToken).toBeNull();
		expect(auth.user).toBeNull();
	});

	it('logout clears state even if the server call fails', async () => {
		auth.accessToken = 'tok';
		auth.user = {
			id: 'u-1',
			email: 'a@b.com',
			display_name: 'A',
			avatar_url: null,
			role: 'member',
			email_verified: false,
			created_at: '',
			updated_at: ''
		};
		mockSequence({ status: 500, body: { error: 'oops' } });

		await auth.logout();

		expect(auth.accessToken).toBeNull();
		expect(auth.user).toBeNull();
	});
});
