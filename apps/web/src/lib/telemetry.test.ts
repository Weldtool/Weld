import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: true }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$lib/config.svelte', () => ({ appConfig: { studyMode: true } }));

import { auth, tokenExpiry } from './auth.svelte';
import { telemetry } from './telemetry.svelte';

/** Minimal `window` so the unload listeners have somewhere to register. */
function stubWindow() {
	const listeners = new Map<string, Set<() => void>>();
	vi.stubGlobal('window', {
		addEventListener: (type: string, fn: () => void) => {
			if (!listeners.has(type)) listeners.set(type, new Set());
			listeners.get(type)?.add(fn);
		},
		removeEventListener: (type: string, fn: () => void) => listeners.get(type)?.delete(fn),
		screen: { width: 1, height: 1 },
		innerWidth: 1,
		innerHeight: 1
	});
	vi.stubGlobal('navigator', { userAgent: 'test', platform: 'test', language: 'en' });
	return (type: string) => listeners.get(type)?.forEach((fn) => fn());
}

function eventsIn(call: unknown[]): { event_type: string }[] {
	const init = call[1] as RequestInit;
	return (JSON.parse(String(init.body)) as { events: { event_type: string }[] }).events;
}

function respond(status: number): Response {
	return new Response(JSON.stringify(status === 200 ? { stored: 1 } : { error: 'x' }), {
		status,
		headers: { 'content-type': 'application/json' }
	});
}

beforeEach(() => {
	vi.useFakeTimers();
	auth.accessToken = 'token';
});

afterEach(async () => {
	vi.stubGlobal(
		'fetch',
		vi.fn(async () => respond(200))
	);
	await telemetry.end();
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('unload flush', () => {
	it('records session_abandoned once when both unload events fire', () => {
		const dispatch = stubWindow();
		const fetchMock = vi.fn(async () => respond(200));
		vi.stubGlobal('fetch', fetchMock);

		telemetry.start();
		telemetry.markInProgress(true);
		dispatch('beforeunload');
		dispatch('pagehide');

		expect(fetchMock).toHaveBeenCalledTimes(1);
		const types = eventsIn(fetchMock.mock.calls[0]).map((e) => e.event_type);
		expect(types.filter((t) => t === 'session_abandoned')).toHaveLength(1);
		telemetry.markInProgress(false);
	});

	it('re-arms after the page is restored from the back/forward cache', () => {
		const dispatch = stubWindow();
		const fetchMock = vi.fn(async () => respond(200));
		vi.stubGlobal('fetch', fetchMock);

		telemetry.start();
		telemetry.markInProgress(true);
		dispatch('pagehide');
		dispatch('pageshow');
		dispatch('pagehide');

		expect(fetchMock).toHaveBeenCalledTimes(2);
		telemetry.markInProgress(false);
	});
});

describe('flush', () => {
	it('keeps a batch the server could not take yet, and drops one it refused', async () => {
		stubWindow();
		telemetry.start();

		// 503: a server fault. The events must survive for the next attempt.
		const failing = vi.fn(async () => respond(503));
		vi.stubGlobal('fetch', failing);
		await telemetry.flush();
		expect(failing).toHaveBeenCalledTimes(1);

		const succeeding = vi.fn(async () => respond(200));
		vi.stubGlobal('fetch', succeeding);
		await telemetry.flush();
		expect(eventsIn(succeeding.mock.calls[0]).map((e) => e.event_type)).toContain('session_start');

		// 400: the content itself was refused. Retrying it would never succeed.
		telemetry.log('view_changed', {});
		const rejecting = vi.fn(async () => respond(400));
		vi.stubGlobal('fetch', rejecting);
		await telemetry.flush();
		const after = vi.fn(async () => respond(200));
		vi.stubGlobal('fetch', after);
		await telemetry.flush();
		expect(after).not.toHaveBeenCalled();
	});
});

describe('tokenExpiry', () => {
	it('reads exp from a JWT and tolerates anything else', () => {
		const payload = btoa(JSON.stringify({ sub: 'u', exp: 1_700_000_000 }));
		expect(tokenExpiry(`h.${payload}.s`)).toBe(1_700_000_000_000);
		expect(tokenExpiry(null)).toBeNull();
		expect(tokenExpiry('not-a-jwt')).toBeNull();
		expect(tokenExpiry('a.!!!.c')).toBeNull();
	});
});
