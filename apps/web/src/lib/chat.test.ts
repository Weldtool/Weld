import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: false }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

import type { ChatMessage, TeamMember } from '$lib/api/types';
import { extractMentionTokens, resolveMentionedUserIds } from './chat-helpers';
import { chat, isPending } from './chat.svelte';

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
		if (!next) throw new Error('Unexpected fetch — no more mocked responses');
		return jsonResponse(next);
	});
	vi.stubGlobal('fetch', fn);
	return fn;
}

const messageFixture = (over: Partial<ChatMessage> = {}): ChatMessage => ({
	id: over.id ?? 'm-1',
	channel_id: over.channel_id ?? 'c-1',
	sender_id: over.sender_id ?? 'u-1',
	content: over.content ?? 'hello',
	thread_parent_id: over.thread_parent_id ?? null,
	mentions: over.mentions ?? [],
	edited_at: null,
	created_at: over.created_at ?? new Date().toISOString()
});

const memberFixture = (over: Partial<TeamMember> = {}): TeamMember => ({
	user_id: over.user_id ?? 'u-1',
	email: over.email ?? 'alice@example.com',
	display_name: over.display_name ?? 'Alice',
	avatar_url: null,
	role: 'member',
	joined_at: ''
});

beforeEach(() => {
	chat.reset();
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('extractMentionTokens', () => {
	it('returns each unique token in source order, lowercased', () => {
		expect(extractMentionTokens('hi @Alice and @bob, @ALICE')).toEqual(['alice', 'bob']);
	});

	it('ignores mentions inside inline code', () => {
		expect(extractMentionTokens('`@alice` and @bob')).toEqual(['bob']);
	});

	it('ignores mentions inside fenced code blocks', () => {
		expect(extractMentionTokens('```\n@alice\n```\n@bob')).toEqual(['bob']);
	});

	it('does not match @ embedded in alphanumeric (e.g. emails)', () => {
		expect(extractMentionTokens('contact me at foo@bar.com or @bob')).toEqual(['bob']);
	});

	it('returns [] for content with no mentions', () => {
		expect(extractMentionTokens('no one here')).toEqual([]);
	});
});

describe('resolveMentionedUserIds', () => {
	it('resolves by email local part and display name', () => {
		const members = [
			memberFixture({ user_id: 'u-1', email: 'alice@x.com', display_name: 'Alice Smith' }),
			memberFixture({ user_id: 'u-2', email: 'bob@x.com', display_name: 'Bobby B' })
		];
		expect(resolveMentionedUserIds('@alice and @bobbyb', members).sort()).toEqual(['u-1', 'u-2']);
	});

	it('returns [] when no token resolves', () => {
		const members = [memberFixture()];
		expect(resolveMentionedUserIds('@nobody', members)).toEqual([]);
	});
});

describe('optimistic send', () => {
	it('appends a pending message immediately and replaces it on success', async () => {
		const confirmed = messageFixture({ id: 'srv-1', content: 'hi there' });
		mockSequence({ status: 201, body: confirmed });

		const promise = chat.sendMessage('c-1', 'hi there');
		// Pending entry visible while POST is in flight.
		const beforeAck = chat.messagesByChannel['c-1'].messages;
		expect(beforeAck).toHaveLength(1);
		expect(isPending(beforeAck[0])).toBe(true);

		const result = await promise;
		expect(result?.id).toBe('srv-1');
		const after = chat.messagesByChannel['c-1'].messages;
		expect(after).toHaveLength(1);
		expect(isPending(after[0])).toBe(false);
		expect(after[0].id).toBe('srv-1');
	});

	it('marks pending as failed when the POST rejects, leaves it in place for retry', async () => {
		mockSequence({ status: 500, body: { error: 'boom' } });

		const result = await chat.sendMessage('c-1', 'oops');
		expect(result).toBeNull();
		const list = chat.messagesByChannel['c-1'].messages;
		expect(list).toHaveLength(1);
		expect(isPending(list[0])).toBe(true);
		expect((list[0] as { failed?: boolean }).failed).toBe(true);
	});

	it('discardPending removes the failed entry', async () => {
		mockSequence({ status: 500, body: { error: 'boom' } });
		await chat.sendMessage('c-1', 'oops');
		const pending = chat.messagesByChannel['c-1'].messages[0];
		expect(isPending(pending)).toBe(true);
		chat.discardPending('c-1', (pending as { clientId: string }).clientId);
		expect(chat.messagesByChannel['c-1'].messages).toHaveLength(0);
	});
});

describe('unread count reconciliation', () => {
	it('increments unread for inactive channels, leaves the active one at 0, and resets on openChannel', async () => {
		mockSequence(
			{ status: 200, body: { messages: [], next_before: null } },
			{ status: 204, body: '' }
		);
		await chat.openChannel('c-active');

		chat.handleIncoming(
			messageFixture({ id: 'a', channel_id: 'c-other', created_at: '2024-01-01T00:00:00Z' })
		);
		chat.handleIncoming(
			messageFixture({ id: 'b', channel_id: 'c-other', created_at: '2024-01-01T00:00:01Z' })
		);
		chat.handleIncoming(
			messageFixture({ id: 'c', channel_id: 'c-active', created_at: '2024-01-01T00:00:02Z' })
		);

		expect(chat.unreadByChannel['c-other']).toBe(2);
		expect(chat.unreadByChannel['c-active'] ?? 0).toBe(0);

		mockSequence(
			{ status: 200, body: { messages: [], next_before: null } },
			{ status: 204, body: '' }
		);
		await chat.openChannel('c-other');
		expect(chat.unreadByChannel['c-other']).toBe(0);
	});

	it('fires the mention listener only when selfId is in the mentions array', () => {
		const seen: string[] = [];
		chat.onMention('me', (m) => seen.push(m.id));
		chat.handleIncoming(messageFixture({ id: '1', mentions: ['someone-else'] }));
		chat.handleIncoming(messageFixture({ id: '2', mentions: ['me'] }));
		chat.handleIncoming(messageFixture({ id: '3', mentions: [] }));
		expect(seen).toEqual(['2']);
	});
});
