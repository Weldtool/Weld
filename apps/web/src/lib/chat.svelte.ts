import { channels as channelsApi, messages as messagesApi } from '$lib/api';
import type { Channel, ChatMessage } from '$lib/api/types';
import { realtime, type RealtimeEnvelope } from '$lib/realtime.svelte';

const TYPING_TTL_MS = 3_000;
const TYPING_EMIT_INTERVAL_MS = 2_000;

type ChannelMessages = {
	messages: ChatMessage[];
	nextBefore: string | null;
	loadedAll: boolean;
};

class ChatStore {
	channels = $state<Channel[]>([]);
	messagesByChannel = $state<Record<string, ChannelMessages>>({});
	/** channelId -> userId -> last seen typing timestamp (ms) */
	typingByChannel = $state<Record<string, Record<string, number>>>({});
	/** channelId -> count of unread messages since the active channel changed */
	unreadByChannel = $state<Record<string, number>>({});
	activeChannelId = $state<string | null>(null);
	error = $state<string | null>(null);

	#unsubs: Array<() => void> = [];
	#joinedRooms = new Set<string>();
	#typingTimer: ReturnType<typeof setInterval> | null = null;
	#lastTypingEmitAt = 0;

	async loadChannelsForTeam(teamId: string): Promise<void> {
		try {
			this.channels = await channelsApi.listForTeam(teamId);
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load channels.';
		}
	}

	async openChannel(channelId: string): Promise<void> {
		this.activeChannelId = channelId;
		this.unreadByChannel = { ...this.unreadByChannel, [channelId]: 0 };
		if (!this.messagesByChannel[channelId]) {
			await this.#fetchPage(channelId, undefined);
		}
		this.#joinChannelRoom(channelId);
		this.#ensureRealtimeWired();
		void channelsApi.markRead(channelId).catch(() => {
			// nice-to-have; don't block UI
		});
	}

	async loadOlder(channelId: string): Promise<void> {
		const cur = this.messagesByChannel[channelId];
		if (!cur || cur.loadedAll || !cur.nextBefore) return;
		await this.#fetchPage(channelId, cur.nextBefore);
	}

	async sendMessage(
		channelId: string,
		content: string,
		threadParentId?: string
	): Promise<ChatMessage | null> {
		try {
			const msg = await messagesApi.post(channelId, {
				content,
				thread_parent_id: threadParentId
			});
			this.#applyMessageUpsert(msg);
			return msg;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to send message.';
			return null;
		}
	}

	async editMessage(messageId: string, content: string): Promise<void> {
		const updated = await messagesApi.edit(messageId, { content });
		this.#applyMessageUpsert(updated);
	}

	async deleteMessage(messageId: string, channelId: string): Promise<void> {
		await messagesApi.delete(messageId);
		this.#applyMessageDelete(channelId, messageId);
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
		this.messagesByChannel = {};
		this.typingByChannel = {};
		this.unreadByChannel = {};
		this.activeChannelId = null;
		this.error = null;
	}

	async #fetchPage(channelId: string, before: string | undefined): Promise<void> {
		try {
			const page = await messagesApi.list(channelId, before ? { before } : undefined);
			const cur = this.messagesByChannel[channelId] ?? {
				messages: [],
				nextBefore: null,
				loadedAll: false
			};
			// Server returns newest first; merge into ascending order.
			const incoming = [...page.messages].reverse();
			const merged = before ? [...incoming, ...cur.messages] : [...cur.messages, ...incoming];
			const dedup = dedupById(merged);
			this.messagesByChannel = {
				...this.messagesByChannel,
				[channelId]: {
					messages: dedup,
					nextBefore: page.next_before,
					loadedAll: page.next_before === null
				}
			};
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load messages.';
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
		// Periodically prune expired typing entries so UI updates without an event.
		this.#typingTimer = setInterval(() => {
			this.#pruneExpiredTyping();
		}, 1_000);
	}

	#onMessageNew(envelope: RealtimeEnvelope): void {
		const msg = envelope.payload as unknown as ChatMessage;
		this.#applyMessageUpsert(msg);
		if (msg.channel_id !== this.activeChannelId) {
			this.unreadByChannel = {
				...this.unreadByChannel,
				[msg.channel_id]: (this.unreadByChannel[msg.channel_id] ?? 0) + 1
			};
		}
	}

	#onMessageEdited(envelope: RealtimeEnvelope): void {
		const msg = envelope.payload as unknown as ChatMessage;
		this.#applyMessageUpsert(msg);
	}

	#onMessageDeleted(envelope: RealtimeEnvelope): void {
		const id = envelope.payload.id as string | undefined;
		const channelId = envelope.payload.channel_id as string | undefined;
		if (!id || !channelId) return;
		this.#applyMessageDelete(channelId, id);
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

	#applyMessageUpsert(msg: ChatMessage): void {
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
			[msg.channel_id]: { ...cur, messages: merged }
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
}

function dedupById(list: ChatMessage[]): ChatMessage[] {
	const seen: Record<string, true> = {};
	const out: ChatMessage[] = [];
	for (const m of list) {
		if (seen[m.id]) continue;
		seen[m.id] = true;
		out.push(m);
	}
	return out.sort((a, b) => a.created_at.localeCompare(b.created_at));
}

export const chat = new ChatStore();
