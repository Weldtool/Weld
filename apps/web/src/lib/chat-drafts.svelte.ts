import { browser } from '$app/environment';

/**
 * Half-written chat messages, kept alive across navigation.
 *
 * The composer lives inside the chat route, so switching to the board (or to
 * another channel, or to a thread) destroys it and used to take the typed text
 * with it. That is ordinary annoyance in the product and a measurement problem
 * in the study: several tasks require reading the board or a PR *while*
 * composing a reply, so losing the draft punishes exactly the cross-surface
 * behaviour the study is trying to observe.
 *
 * Drafts are per composer — a channel and its threads each keep their own — and
 * are mirrored to localStorage so a refresh or a resume on the same device
 * doesn't lose them either. They never leave the browser: no draft is sent to
 * the server and none is recorded as telemetry until the participant actually
 * sends the message.
 */

const PREFIX = 'weld:chat-draft:';

/** Identifies one composer. Threads compose separately from their channel. */
export function draftKey(channelId: string, threadParentId?: string): string {
	return threadParentId ? `${channelId}:${threadParentId}` : channelId;
}

class ChatDraftStore {
	/** In-memory copy; localStorage is the durable mirror, not the fast path. */
	#drafts = $state<Record<string, string>>({});

	/** The draft for a composer, or '' when there isn't one. */
	get(key: string): string {
		const held = this.#drafts[key];
		if (held !== undefined) return held;
		if (!browser) return '';
		const stored = localStorage.getItem(PREFIX + key) ?? '';
		this.#drafts[key] = stored;
		return stored;
	}

	/** Record what's currently typed. Empty text clears the draft outright so we
	 *  don't leave a trail of blank keys behind in storage. */
	set(key: string, value: string): void {
		if (!value) {
			this.clear(key);
			return;
		}
		this.#drafts[key] = value;
		if (browser) {
			try {
				localStorage.setItem(PREFIX + key, value);
			} catch {
				// Storage full or blocked (private mode): the in-memory copy still
				// covers navigation, which is the case that matters.
			}
		}
	}

	/** Drop a draft — the message was sent, or the box was emptied by hand. */
	clear(key: string): void {
		delete this.#drafts[key];
		if (browser) localStorage.removeItem(PREFIX + key);
	}

	/** Drop every draft (e.g. when a session ends on a shared machine). */
	clearAll(): void {
		this.#drafts = {};
		if (!browser) return;
		for (const k of Object.keys(localStorage)) {
			if (k.startsWith(PREFIX)) localStorage.removeItem(k);
		}
	}
}

export const chatDrafts = new ChatDraftStore();
