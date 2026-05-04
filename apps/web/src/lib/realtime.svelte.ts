import { browser } from '$app/environment';
import { env } from '$env/dynamic/public';

export type RealtimeEnvelope = {
	type: string;
	payload: Record<string, unknown>;
	room: string;
	timestamp: string;
};

type Handler = (envelope: RealtimeEnvelope) => void;

const RECONNECT_INITIAL_MS = 500;
const RECONNECT_MAX_MS = 30_000;

function wsUrl(token: string): string {
	const base = (env.PUBLIC_API_BASE_URL ?? '').replace(/\/$/, '');
	const encoded = encodeURIComponent(token);
	if (base) {
		// Plain string transform — avoids constructing a mutable URL instance.
		const proto = base.startsWith('https://') ? 'wss://' : 'ws://';
		const host = base.replace(/^https?:\/\//, '');
		return `${proto}${host}/api/v1/ws?token=${encoded}`;
	}
	const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
	return `${proto}//${location.host}/api/v1/ws?token=${encoded}`;
}

/**
 * Singleton WebSocket connection to the realtime hub. Subscriptions and
 * handlers survive reconnects: on every successful connect we re-send the
 * accumulated `subscribe` set.
 */
class RealtimeClient {
	#socket: WebSocket | null = null;
	#token: string | null = null;
	#desiredOpen = false;
	#rooms = new Set<string>();
	#handlers = new Map<string, Set<Handler>>();
	/** room → ISO timestamp of last event seen; replayed via `since` on reconnect */
	#cursors = new Map<string, string>();
	#reconnectDelay = RECONNECT_INITIAL_MS;
	#reconnectTimer: ReturnType<typeof setTimeout> | null = null;

	connect(token: string): void {
		if (!browser) return;
		this.#token = token;
		this.#desiredOpen = true;
		this.#open();
	}

	disconnect(): void {
		this.#desiredOpen = false;
		this.#token = null;
		this.#rooms.clear();
		this.#cursors.clear();
		if (this.#reconnectTimer) {
			clearTimeout(this.#reconnectTimer);
			this.#reconnectTimer = null;
		}
		this.#socket?.close();
		this.#socket = null;
	}

	join(room: string): void {
		if (this.#rooms.has(room)) return;
		this.#rooms.add(room);
		this.#sendSubscribe([room]);
	}

	leave(room: string): void {
		if (!this.#rooms.delete(room)) return;
		this.#cursors.delete(room);
		this.#send({ type: 'ws.unsubscribe', rooms: [room] });
	}

	/**
	 * Record the latest event timestamp seen for `room`. On reconnect the
	 * server uses this value as the `since` cursor to replay anything we
	 * missed while disconnected.
	 */
	setCursor(room: string, isoTimestamp: string): void {
		const prev = this.#cursors.get(room);
		if (prev && prev >= isoTimestamp) return;
		this.#cursors.set(room, isoTimestamp);
	}

	on(eventType: string, handler: Handler): () => void {
		let set = this.#handlers.get(eventType);
		if (!set) {
			// eslint-disable-next-line svelte/prefer-svelte-reactivity
			set = new Set();
			this.#handlers.set(eventType, set);
		}
		set.add(handler);
		return () => {
			const s = this.#handlers.get(eventType);
			s?.delete(handler);
			if (s && s.size === 0) this.#handlers.delete(eventType);
		};
	}

	#open(): void {
		if (!this.#token || !this.#desiredOpen) return;
		if (this.#socket && this.#socket.readyState <= WebSocket.OPEN) return;

		const ws = new WebSocket(wsUrl(this.#token));
		this.#socket = ws;
		ws.addEventListener('open', () => {
			this.#reconnectDelay = RECONNECT_INITIAL_MS;
			this.#resubscribeAll();
		});
		ws.addEventListener('message', (ev) => {
			try {
				const parsed = JSON.parse(ev.data) as RealtimeEnvelope;
				const set = this.#handlers.get(parsed.type);
				if (set) for (const h of set) h(parsed);
			} catch {
				// non-JSON or unexpected payload — ignore
			}
		});
		ws.addEventListener('close', () => {
			this.#socket = null;
			if (this.#desiredOpen) this.#scheduleReconnect();
		});
		ws.addEventListener('error', () => {
			ws.close();
		});
	}

	#scheduleReconnect(): void {
		if (this.#reconnectTimer) return;
		const delay = this.#reconnectDelay;
		this.#reconnectDelay = Math.min(delay * 2, RECONNECT_MAX_MS);
		this.#reconnectTimer = setTimeout(() => {
			this.#reconnectTimer = null;
			this.#open();
		}, delay);
	}

	send(message: unknown): void {
		this.#send(message);
	}

	#sendSubscribe(rooms: string[]): void {
		this.#send({ type: 'ws.subscribe', rooms });
	}

	/**
	 * Resubscribe to every desired room after (re)connect. Rooms with a
	 * recorded cursor are sent individually with `since` so the server can
	 * replay missed events to this connection only.
	 */
	#resubscribeAll(): void {
		const fresh: string[] = [];
		for (const room of this.#rooms) {
			const since = this.#cursors.get(room);
			if (since) {
				this.#send({ type: 'ws.subscribe', rooms: [room], since });
			} else {
				fresh.push(room);
			}
		}
		if (fresh.length > 0) this.#sendSubscribe(fresh);
	}

	#send(message: unknown): void {
		const ws = this.#socket;
		if (ws && ws.readyState === WebSocket.OPEN) {
			ws.send(JSON.stringify(message));
		}
		// If not yet open, the open handler will replay subscriptions.
	}
}

export const realtime = new RealtimeClient();
