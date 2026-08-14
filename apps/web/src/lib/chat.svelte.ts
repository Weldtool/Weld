import { channels as channelsApi, messages as messagesApi, teams as teamsApi } from '$lib/api';
import type { Channel, ChatMessage, TaskRef, TeamMember } from '$lib/api/types';
import { auth } from '$lib/auth.svelte';
import { realtime, type RealtimeEnvelope } from '$lib/realtime.svelte';
import { telemetry } from '$lib/telemetry.svelte';
import { untrack } from 'svelte';

const TYPING_TTL_MS = 3_000;
const TYPING_EMIT_INTERVAL_MS = 2_000;

/** A message that has been optimistically appended but not yet ack'd by the
 * server. Reconciled (or rolled back) when the POST resolves. */
export type PendingMessage = ChatMessage & {
	pending: true;
	failed?: boolean;
	clientId: string;
};

type AnyMessage = ChatMessage | PendingMessage;

type ChannelMessages = {
	messages: AnyMessage[];
	nextBefore: string | null;
	loadedAll: boolean;
	loadError: string | null;
};

type ThreadState = {
	parent: ChatMessage | null;
	replies: AnyMessage[];
	loading: boolean;
	error: string | null;
};

class ChatStore {
	channels = $state<Channel[]>([]);
	members = $state<TeamMember[]>([]);
	taskRefs = $state<TaskRef[]>([]);
	messagesByChannel = $state<Record<string, ChannelMessages>>({});
	/** channelId -> userId -> last seen typing timestamp (ms) */
	typingByChannel = $state<Record<string, Record<string, number>>>({});
	/** channelId -> count of unread messages since the active channel changed */
	unreadByChannel = $state<Record<string, number>>({});
	activeChannelId = $state<string | null>(null);
	activeTeamId = $state<string | null>(null);
	threadByParent = $state<Record<string, ThreadState>>({});
	error = $state<string | null>(null);

	#unsubs: Array<() => void> = [];
	#joinedRooms = new Set<string>();
	#typingTimer: ReturnType<typeof setInterval> | null = null;
	#lastTypingEmitAt = 0;
	#mentionListener: ((msg: ChatMessage) => void) | null = null;

	async loadForTeam(teamId: string): Promise<void> {
		if (this.activeTeamId === teamId && this.channels.length > 0) return;
		this.activeTeamId = teamId;
		try {
			const [chans, members, taskRefs] = await Promise.all([
				channelsApi.listForTeam(teamId),
				teamsApi.listMembers(teamId),
				teamsApi.listTaskRefs(teamId).catch(() => [])
			]);
			this.channels = chans;
			this.members = members;
			this.taskRefs = taskRefs;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load chat data.';
		}
	}

	async createChannel(input: {
		name: string;
		topic?: string;
		project_id?: string;
		members?: string[];
	}): Promise<Channel | null> {
		if (!this.activeTeamId) return null;
		try {
			const created = await channelsApi.create(this.activeTeamId, {
				...input,
				is_private: true
			});
			this.channels = [...this.channels, created];
			return created;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to create channel.';
			return null;
		}
	}

	updateChannel(updated: Channel): void {
		this.channels = this.channels.map((c) => (c.id === updated.id ? updated : c));
	}

	async archiveChannel(channelId: string): Promise<boolean> {
		try {
			await channelsApi.archive(channelId);
			this.channels = this.channels.filter((c) => c.id !== channelId);
			if (this.activeChannelId === channelId) this.activeChannelId = null;
			const restMsgs = { ...this.messagesByChannel };
			delete restMsgs[channelId];
			this.messagesByChannel = restMsgs;
			const restUnread = { ...this.unreadByChannel };
			delete restUnread[channelId];
			this.unreadByChannel = restUnread;
			return true;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to archive channel.';
			return false;
		}
	}

	loadingChannel = $state<string | null>(null);

	async openChannel(channelId: string): Promise<void> {
		const previous = this.activeChannelId;
		if (previous !== channelId) {
			telemetry.log('channel_switched', { from: previous, to: channelId });
		}
		this.activeChannelId = channelId;
		// `untrack` so callers inside an $effect don't register a dep on the spread
		// read of `unreadByChannel` (which we also write to, causing an infinite loop).
		untrack(() => {
			this.unreadByChannel = { ...this.unreadByChannel, [channelId]: 0 };
		});
		const existing = untrack(() => this.messagesByChannel[channelId]);
		if (!existing || existing.loadError) {
			this.loadingChannel = channelId;
			try {
				await this.#fetchPage(channelId, undefined);
			} finally {
				this.loadingChannel = null;
			}
		}
		this.#joinChannelRoom(channelId);
		this.#ensureRealtimeWired();
		void channelsApi.markRead(channelId).catch(() => {});
	}

	async loadOlder(channelId: string): Promise<void> {
		const cur = this.messagesByChannel[channelId];
		if (!cur || cur.loadedAll || !cur.nextBefore) return;
		await this.#fetchPage(channelId, cur.nextBefore);
	}

	/**
	 * Optimistically append a message, POST it, and reconcile (or roll back).
	 * Returns the server-confirmed message, or null on failure (the pending
	 * entry is left in place with `failed: true` so the UI can offer Retry).
	 */
	async sendMessage(
		channelId: string,
		content: string,
		threadParentId?: string
	): Promise<ChatMessage | null> {
		const trimmed = content.trim();
		if (!trimmed) return null;

		const clientId = `pending-${crypto.randomUUID()}`;
		const pending: PendingMessage = {
			id: clientId,
			clientId,
			channel_id: channelId,
			sender_id: this.#selfSenderId(),
			content: trimmed,
			thread_parent_id: threadParentId ?? null,
			mentions: [],
			task_refs: [],
			metadata: {},
			edited_at: null,
			created_at: new Date().toISOString(),
			pending: true
		};
		this.#applyMessageUpsert(pending);
		if (threadParentId) this.#applyThreadUpsert(threadParentId, pending);

		try {
			const confirmed = await messagesApi.post(channelId, {
				content: trimmed,
				thread_parent_id: threadParentId
			});
			this.#replacePending(channelId, clientId, confirmed);
			if (threadParentId) this.#replaceThreadPending(threadParentId, clientId, confirmed);
			// User-sent message (the agent's chat_post path is server-side) — capture
			// for B2/T3/T6 and who-did-it. Content length only, never the text (§5).
			telemetry.log('message_sent', {
				channel_id: channelId,
				message_id: confirmed.id,
				length: trimmed.length,
				in_thread: Boolean(threadParentId),
				task_refs: confirmed.task_refs ?? [],
				mention_count: confirmed.mentions?.length ?? 0
			});
			return confirmed;
		} catch (err) {
			this.#markPendingFailed(channelId, clientId);
			if (threadParentId) this.#markThreadPendingFailed(threadParentId, clientId);
			this.error = err instanceof Error ? err.message : 'Failed to send message.';
			return null;
		}
	}

	async retryPending(channelId: string, clientId: string): Promise<void> {
		const cur = this.messagesByChannel[channelId];
		const pending = cur?.messages.find(
			(m): m is PendingMessage => isPending(m) && m.clientId === clientId
		);
		if (!pending) return;
		// Drop the failed marker so UI shows "sending" again.
		this.#updatePending(channelId, clientId, (p) => ({ ...p, failed: false }));
		try {
			const confirmed = await messagesApi.post(channelId, {
				content: pending.content,
				thread_parent_id: pending.thread_parent_id ?? undefined
			});
			this.#replacePending(channelId, clientId, confirmed);
			if (pending.thread_parent_id)
				this.#replaceThreadPending(pending.thread_parent_id, clientId, confirmed);
		} catch (err) {
			this.#markPendingFailed(channelId, clientId);
			this.error = err instanceof Error ? err.message : 'Failed to send message.';
		}
	}

	discardPending(channelId: string, clientId: string): void {
		const cur = this.messagesByChannel[channelId];
		if (!cur) return;
		this.messagesByChannel = {
			...this.messagesByChannel,
			[channelId]: {
				...cur,
				messages: cur.messages.filter((m) => !(isPending(m) && m.clientId === clientId))
			}
		};
	}

	async editMessage(messageId: string, content: string): Promise<void> {
		const updated = await messagesApi.edit(messageId, { content });
		this.#applyMessageUpsert(updated);
		// Mirror the realtime edited-handler so an edit made from the thread panel
		// updates the open thread too, not just the channel list.
		if (updated.thread_parent_id) this.#applyThreadUpsert(updated.thread_parent_id, updated);
		telemetry.log('message_edited', { message_id: messageId, length: content.trim().length });
	}

	async deleteMessage(messageId: string, channelId: string): Promise<void> {
		await messagesApi.delete(messageId);
		this.#applyMessageDelete(channelId, messageId);
		telemetry.log('message_deleted', { message_id: messageId, channel_id: channelId });
	}

	async openThread(parentId: string): Promise<void> {
		telemetry.log('thread_opened', { parent_id: parentId });
		// `untrack` so callers inside an $effect (e.g. ThreadPanel) don't register
		// a dep on threadByParent — we read and then write it, which would loop.
		const existing = untrack(() => this.threadByParent[parentId]);
		if (existing && existing.parent && !existing.loading) return;
		untrack(() => {
			this.threadByParent = {
				...this.threadByParent,
				[parentId]: {
					parent: existing?.parent ?? null,
					replies: existing?.replies ?? [],
					loading: true,
					error: null
				}
			};
		});
		try {
			const view = await messagesApi.thread(parentId);
			this.threadByParent = {
				...this.threadByParent,
				[parentId]: { parent: view.parent, replies: view.replies, loading: false, error: null }
			};
		} catch (err) {
			this.threadByParent = {
				...this.threadByParent,
				[parentId]: {
					parent: existing?.parent ?? null,
					replies: existing?.replies ?? [],
					loading: false,
					error: err instanceof Error ? err.message : 'Failed to load thread.'
				}
			};
		}
	}

	closeThread(parentId: string): void {
		const next = { ...this.threadByParent };
		delete next[parentId];
		this.threadByParent = next;
	}

	/** Number of replies a parent message has, considering both loaded thread
	 * state and any thread replies present in the channel cache. */
	threadReplyCount(parentId: string): number {
		const thread = this.threadByParent[parentId];
		if (thread?.replies) return thread.replies.length;
		// Fallback: count any cached thread children we've seen via realtime.
		let count = 0;
		for (const ch of Object.values(this.messagesByChannel)) {
			for (const m of ch.messages) if (m.thread_parent_id === parentId) count++;
		}
		return count;
	}

	emitTyping(channelId: string): void {
		const now = Date.now();
		if (now - this.#lastTypingEmitAt < TYPING_EMIT_INTERVAL_MS) return;
		this.#lastTypingEmitAt = now;
		realtime.send({ type: 'chat.typing', channel_id: channelId });
	}

	typingUsersFor(channelId: string): string[] {
		const map = this.typingByChannel[channelId];
		if (!map) return [];
		const now = Date.now();
		return Object.entries(map)
			.filter(([, ts]) => now - ts < TYPING_TTL_MS)
			.map(([uid]) => uid);
	}

	memberById(id: string | null | undefined): TeamMember | null {
		if (!id) return null;
		return this.members.find((m) => m.user_id === id) ?? null;
	}

	taskByShortId(shortId: number): TaskRef | null {
		return this.taskRefs.find((t) => t.short_id === shortId) ?? null;
	}

	/** Subscribe to mention notifications. Fires for every incoming message
	 * whose `mentions` array includes `selfId`. The caller passes `selfId`. */
	onMention(selfId: string, handler: (msg: ChatMessage) => void): () => void {
		this.#mentionListener = (msg: ChatMessage) => {
			if (msg.mentions.includes(selfId)) handler(msg);
		};
		return () => {
			this.#mentionListener = null;
		};
	}

	reset(): void {
		for (const unsub of this.#unsubs) unsub();
		this.#unsubs = [];
		for (const room of this.#joinedRooms) realtime.leave(room);
		this.#joinedRooms.clear();
		if (this.#typingTimer) {
			clearInterval(this.#typingTimer);
			this.#typingTimer = null;
		}
		this.channels = [];
		this.members = [];
		this.taskRefs = [];
		this.messagesByChannel = {};
		this.typingByChannel = {};
		this.unreadByChannel = {};
		this.threadByParent = {};
		this.activeChannelId = null;
		this.activeTeamId = null;
		this.error = null;
	}

	#selfSenderId(): string {
		return auth.user?.id ?? '';
	}

	async #fetchPage(channelId: string, before: string | undefined): Promise<void> {
		try {
			const page = await messagesApi.list(channelId, before ? { before } : undefined);
			const cur = this.messagesByChannel[channelId] ?? {
				messages: [],
				nextBefore: null,
				loadedAll: false,
				loadError: null
			};
			const incoming = [...page.messages].reverse(); // newest-first → ascending
			const merged = before ? [...incoming, ...cur.messages] : [...cur.messages, ...incoming];
			const dedup = dedupAndSort(merged);
			this.messagesByChannel = {
				...this.messagesByChannel,
				[channelId]: {
					messages: dedup,
					nextBefore: page.next_before,
					loadedAll: page.next_before === null,
					loadError: null
				}
			};
			const newest = dedup[dedup.length - 1];
			if (newest && !isPending(newest))
				realtime.setCursor(`channel:${channelId}`, newest.created_at);
		} catch (err) {
			const msg = err instanceof Error ? err.message : 'Failed to load messages.';
			this.error = msg;
			const cur = this.messagesByChannel[channelId];
			this.messagesByChannel = {
				...this.messagesByChannel,
				[channelId]: {
					messages: cur?.messages ?? [],
					nextBefore: cur?.nextBefore ?? null,
					loadedAll: cur?.loadedAll ?? false,
					loadError: msg
				}
			};
		}
	}

	#joinChannelRoom(channelId: string): void {
		const room = `channel:${channelId}`;
		if (this.#joinedRooms.has(room)) return;
		realtime.join(room);
		this.#joinedRooms.add(room);
	}

	#ensureRealtimeWired(): void {
		if (this.#unsubs.length > 0) return;
		this.#unsubs = [
			realtime.on('chat.message.new', (e) => this.#onMessageNew(e)),
			realtime.on('chat.message.edited', (e) => this.#onMessageEdited(e)),
			realtime.on('chat.message.deleted', (e) => this.#onMessageDeleted(e)),
			realtime.on('chat.user.typing', (e) => this.#onUserTyping(e))
		];
		this.#typingTimer = setInterval(() => this.#pruneExpiredTyping(), 1_000);
	}

	#onMessageNew(envelope: RealtimeEnvelope): void {
		this.handleIncoming(envelope.payload as unknown as ChatMessage);
	}

	/** Public for tests; also invoked from the realtime subscription. Applies
	 * an inbound message to the channel cache, bumps unread for inactive
	 * channels, advances the realtime cursor, and notifies mention listeners. */
	handleIncoming(msg: ChatMessage): void {
		this.#applyMessageUpsert(msg);
		if (msg.thread_parent_id) this.#applyThreadUpsert(msg.thread_parent_id, msg);
		realtime.setCursor(`channel:${msg.channel_id}`, msg.created_at);
		if (msg.channel_id !== this.activeChannelId) {
			this.unreadByChannel = {
				...this.unreadByChannel,
				[msg.channel_id]: (this.unreadByChannel[msg.channel_id] ?? 0) + 1
			};
		}
		this.#mentionListener?.(msg);
	}

	#onMessageEdited(envelope: RealtimeEnvelope): void {
		const msg = envelope.payload as unknown as ChatMessage;
		this.#applyMessageUpsert(msg);
		if (msg.thread_parent_id) this.#applyThreadUpsert(msg.thread_parent_id, msg);
	}

	#onMessageDeleted(envelope: RealtimeEnvelope): void {
		const id = envelope.payload.id as string | undefined;
		const channelId = envelope.payload.channel_id as string | undefined;
		const parentId = envelope.payload.thread_parent_id as string | undefined;
		if (!id || !channelId) return;
		this.#applyMessageDelete(channelId, id);
		if (parentId) this.#removeFromThread(parentId, id);
	}

	#onUserTyping(envelope: RealtimeEnvelope): void {
		const channelId = envelope.payload.channel_id as string | undefined;
		const userId = envelope.payload.user_id as string | undefined;
		if (!channelId || !userId) return;
		const cur = this.typingByChannel[channelId] ?? {};
		this.typingByChannel = {
			...this.typingByChannel,
			[channelId]: { ...cur, [userId]: Date.now() }
		};
	}

	#pruneExpiredTyping(): void {
		const now = Date.now();
		let mutated = false;
		const next: Record<string, Record<string, number>> = {};
		for (const [chId, users] of Object.entries(this.typingByChannel)) {
			const fresh: Record<string, number> = {};
			for (const [uid, ts] of Object.entries(users)) {
				if (now - ts < TYPING_TTL_MS) fresh[uid] = ts;
				else mutated = true;
			}
			next[chId] = fresh;
		}
		if (mutated) this.typingByChannel = next;
	}

	#applyMessageUpsert(msg: AnyMessage): void {
		const cur = this.messagesByChannel[msg.channel_id] ?? {
			messages: [],
			nextBefore: null,
			loadedAll: false
		};
		const exists = cur.messages.some((m) => m.id === msg.id);
		const merged = exists
			? cur.messages.map((m) => (m.id === msg.id ? msg : m))
			: [...cur.messages, msg];
		this.messagesByChannel = {
			...this.messagesByChannel,
			[msg.channel_id]: { ...cur, messages: dedupAndSort(merged), loadError: null }
		};
	}

	#applyMessageDelete(channelId: string, messageId: string): void {
		const cur = this.messagesByChannel[channelId];
		if (!cur) return;
		this.messagesByChannel = {
			...this.messagesByChannel,
			[channelId]: { ...cur, messages: cur.messages.filter((m) => m.id !== messageId) }
		};
	}

	#replacePending(channelId: string, clientId: string, confirmed: ChatMessage): void {
		const cur = this.messagesByChannel[channelId];
		if (!cur) return;
		const next = cur.messages.map((m) => (isPending(m) && m.clientId === clientId ? confirmed : m));
		this.messagesByChannel = {
			...this.messagesByChannel,
			[channelId]: { ...cur, messages: dedupAndSort(next) }
		};
	}

	#markPendingFailed(channelId: string, clientId: string): void {
		this.#updatePending(channelId, clientId, (p) => ({ ...p, failed: true }));
	}

	#updatePending(
		channelId: string,
		clientId: string,
		mut: (p: PendingMessage) => PendingMessage
	): void {
		const cur = this.messagesByChannel[channelId];
		if (!cur) return;
		this.messagesByChannel = {
			...this.messagesByChannel,
			[channelId]: {
				...cur,
				messages: cur.messages.map((m) => (isPending(m) && m.clientId === clientId ? mut(m) : m))
			}
		};
	}

	#applyThreadUpsert(parentId: string, msg: AnyMessage): void {
		const cur = this.threadByParent[parentId];
		if (!cur) return;
		const exists = cur.replies.some((m) => m.id === msg.id);
		const replies = exists
			? cur.replies.map((m) => (m.id === msg.id ? msg : m))
			: [...cur.replies, msg];
		this.threadByParent = {
			...this.threadByParent,
			[parentId]: { ...cur, replies: dedupAndSort(replies) }
		};
	}

	#replaceThreadPending(parentId: string, clientId: string, confirmed: ChatMessage): void {
		const cur = this.threadByParent[parentId];
		if (!cur) return;
		this.threadByParent = {
			...this.threadByParent,
			[parentId]: {
				...cur,
				replies: dedupAndSort(
					cur.replies.map((m) => (isPending(m) && m.clientId === clientId ? confirmed : m))
				)
			}
		};
	}

	#markThreadPendingFailed(parentId: string, clientId: string): void {
		const cur = this.threadByParent[parentId];
		if (!cur) return;
		this.threadByParent = {
			...this.threadByParent,
			[parentId]: {
				...cur,
				replies: cur.replies.map((m) =>
					isPending(m) && m.clientId === clientId ? { ...m, failed: true } : m
				)
			}
		};
	}

	#removeFromThread(parentId: string, messageId: string): void {
		const cur = this.threadByParent[parentId];
		if (!cur) return;
		this.threadByParent = {
			...this.threadByParent,
			[parentId]: { ...cur, replies: cur.replies.filter((m) => m.id !== messageId) }
		};
	}
}

export function isPending(m: AnyMessage): m is PendingMessage {
	return (m as PendingMessage).pending === true;
}

function dedupAndSort(list: AnyMessage[]): AnyMessage[] {
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- local, non-reactive
	const seen = new Set<string>();
	const out: AnyMessage[] = [];
	for (const m of list) {
		if (seen.has(m.id)) continue;
		seen.add(m.id);
		out.push(m);
	}
	return out.sort((a, b) => a.created_at.localeCompare(b.created_at));
}

export const chat = new ChatStore();
