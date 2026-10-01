import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: false }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

import type { ChatMessage, TaskRef, TeamMember } from '$lib/api/types';
import {
	extractMentionTokens,
	renderMessageMarkdown,
	resolveMentionedUserIds
} from './chat-helpers';
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
	task_refs: over.task_refs ?? [],
	metadata: over.metadata ?? {},
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

describe('renderMessageMarkdown PR references', () => {
	const taskRef = (over: Partial<TaskRef> = {}): TaskRef => ({
		id: over.id ?? 't-uuid',
		short_id: over.short_id ?? 7,
		title: over.title ?? 'Add retry logic',
		project_id: over.project_id ?? 'p-uuid',
		github_pr_id: 'github_pr_id' in over ? (over.github_pr_id ?? null) : 42
	});

	it('renders PR#<n> as a link to the linked task on the board', () => {
		const html = renderMessageMarkdown('Fixed in PR#42', { taskRefs: [taskRef()] });
		expect(html).toContain('class="chat-prref"');
		expect(html).toContain('data-pr-number="42"');
		expect(html).toContain('href="/board?task=t-uuid&project=p-uuid"');
		expect(html).toContain('PR #42');
		expect(html).toContain('Add retry logic');
	});

	it('is case-insensitive on the pr prefix', () => {
		const html = renderMessageMarkdown('see pr#42 now', { taskRefs: [taskRef()] });
		expect(html).toContain('class="chat-prref"');
		expect(html).toContain('data-pr-number="42"');
	});

	it('renders an inert badge (no link) when no task links that PR number', () => {
		const html = renderMessageMarkdown('PR#999 is unknown', { taskRefs: [taskRef()] });
		expect(html).toContain('chat-prref-bare');
		expect(html).not.toContain('href');
		expect(html).toContain('PR #999');
	});

	it('does not let the #<short_id> task pass re-match the PR number', () => {
		// short_id 42 also exists as a task; PR#42 must stay a PR ref, not a task ref.
		const refs = [
			taskRef({ id: 't-pr', github_pr_id: 42, short_id: 7 }),
			taskRef({ id: 't-42', short_id: 42, github_pr_id: null })
		];
		const html = renderMessageMarkdown('PR#42', { taskRefs: refs });
		expect(html).toContain('class="chat-prref"');
		expect(html).toContain('href="/board?task=t-pr&project=p-uuid"');
		expect(html).not.toContain('chat-taskref');
	});

	it('still renders a bare #<short_id> task reference normally', () => {
		const html = renderMessageMarkdown('see #7', { taskRefs: [taskRef()] });
		expect(html).toContain('class="chat-taskref"');
		expect(html).toContain('href="/board?task=t-uuid&project=p-uuid"');
	});

	it('ignores PR refs inside inline code', () => {
		const html = renderMessageMarkdown('`PR#42`', { taskRefs: [taskRef()] });
		expect(html).not.toContain('chat-prref');
		expect(html).toContain('<code>PR#42</code>');
	});
});

describe('renderMessageMarkdown links', () => {
	it('keeps query-string ampersands intact instead of double-escaping them', () => {
		const html = renderMessageMarkdown('see [docs](https://example.com/search?a=1&b=2)');
		expect(html).toContain('href="https://example.com/search?a=1&amp;b=2"');
		expect(html).not.toContain('&amp;amp;');
	});

	it('still cannot break out of the href attribute', () => {
		const html = renderMessageMarkdown('[x](https://a.com/"onmouseover="alert(1))');
		expect(html).not.toContain('"onmouseover="');
		expect(html).toContain('&quot;onmouseover=&quot;');
	});
});

describe('thread reply counts', () => {
	it('shows the server-reported count for a thread that was never opened', async () => {
		const parent = { ...messageFixture({ id: 'p-1' }), reply_count: 3 };
		const lonely = { ...messageFixture({ id: 'p-2' }), reply_count: 0 };
		mockSequence(
			{ status: 200, body: { messages: [parent, lonely], next_before: null } },
			{ status: 204, body: '' }
		);
		await chat.openChannel('c-1');

		expect(chat.threadReplyCount('p-1')).toBe(3);
		expect(chat.threadReplyCount('p-2')).toBe(0);
	});

	it('moves with realtime replies and deletions, counting each reply once', async () => {
		const parent = { ...messageFixture({ id: 'p-1' }), reply_count: 1 };
		mockSequence(
			{ status: 200, body: { messages: [parent], next_before: null } },
			{ status: 204, body: '' }
		);
		await chat.openChannel('c-1');

		const reply = messageFixture({ id: 'r-new', thread_parent_id: 'p-1' });
		chat.handleIncoming(reply);
		chat.handleIncoming(reply); // replayed on reconnect
		expect(chat.threadReplyCount('p-1')).toBe(2);

		mockSequence({ status: 200, body: {} });
		await chat.deleteMessage('r-new', 'c-1');
		expect(chat.threadReplyCount('p-1')).toBe(1);
	});

	it('does not double count our own reply when its realtime echo arrives', async () => {
		const confirmed = messageFixture({ id: 'r-own', thread_parent_id: 'p-1' });
		mockSequence({ status: 201, body: confirmed });
		await chat.sendMessage('c-1', 'hello', 'p-1');
		chat.handleIncoming(confirmed);

		expect(chat.threadReplyCount('p-1')).toBe(1);
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

describe('editMessage', () => {
	it('PATCHes the message and applies the updated content to the channel list', async () => {
		const confirmed = messageFixture({ id: 'srv-1', content: 'original' });
		mockSequence({ status: 201, body: confirmed });
		await chat.sendMessage('c-1', 'original');

		const edited = { ...confirmed, content: 'edited', edited_at: '2024-01-01T00:00:00Z' };
		const fetchMock = mockSequence({ status: 200, body: edited });
		await chat.editMessage('srv-1', 'edited');

		const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		expect(String(url)).toContain('/messages/srv-1');
		expect(init.method).toBe('PATCH');
		expect(JSON.parse(String(init.body))).toEqual({ content: 'edited' });

		const list = chat.messagesByChannel['c-1'].messages;
		expect(list).toHaveLength(1);
		expect(list[0].content).toBe('edited');
		expect((list[0] as ChatMessage).edited_at).toBe('2024-01-01T00:00:00Z');
	});

	it('also updates the open thread when the edited message is a reply', async () => {
		const parent = messageFixture({ id: 'p-1', content: 'parent' });
		const reply = messageFixture({ id: 'r-1', thread_parent_id: 'p-1', content: 'original' });
		mockSequence({ status: 200, body: { parent, replies: [reply] } });
		await chat.openThread('p-1');

		mockSequence({ status: 200, body: { ...reply, content: 'edited' } });
		await chat.editMessage('r-1', 'edited');

		const replies = chat.threadByParent['p-1'].replies;
		expect(replies).toHaveLength(1);
		expect(replies[0].content).toBe('edited');
	});

	it('propagates the error so the editor can stay open on failure', async () => {
		mockSequence({ status: 500, body: { error: 'boom' } });
		await expect(chat.editMessage('srv-1', 'edited')).rejects.toThrow();
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
