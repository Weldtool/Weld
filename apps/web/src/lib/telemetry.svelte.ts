import { browser } from '$app/environment';
import { env } from '$env/dynamic/public';
import { study as studyApi, type TelemetryEvent } from '$lib/api';
import { ApiError } from '$lib/api/client';
import { auth } from '$lib/auth.svelte';
import { appConfig } from '$lib/config.svelte';

/**
 * Client-side behavioral telemetry for the research study (Category A,
 * paper/instrument/CLAUDE_STUDY.md §5 — `paper/` is tracked on the
 * `paper/tosm-journal-writing` branch, not this one). Buffers events and
 * flushes them in batches to `POST /api/v1/study/events`. Completely inert
 * unless the backend is in study mode (`appConfig.studyMode`), so it has zero
 * effect on the normal product.
 *
 * Privacy: only the discrete events instrumented in the app are recorded — no
 * keystrokes, mouse movement, or screen capture (§5 exclusions). `log()` takes
 * an explicit event type + structured payload; there is no raw input stream.
 */

const FLUSH_INTERVAL_MS = 5000;
const MAX_BUFFER = 25;
const API_BASE = (env.PUBLIC_API_BASE_URL ?? '').replace(/\/$/, '');

class Telemetry {
	#buffer: TelemetryEvent[] = [];
	#currentTaskId: string | null = null;
	#timer: ReturnType<typeof setInterval> | null = null;
	#started = false;
	#inProgress = false;

	get active(): boolean {
		return appConfig.studyMode;
	}

	/** The task panel calls this so subsequent events are tagged with the task. */
	setTask(taskId: string | null): void {
		this.#currentTaskId = taskId;
	}

	/**
	 * Mark whether the participant is mid-study (tasks/survey, not yet done). When
	 * set, an unload (tab close / reload) emits `session_abandoned` on the way out
	 * so dropouts are explicit. Refreshes also emit it; analysis treats a final
	 * `session_abandoned` with no later events / no `survey_completed` as a true
	 * dropout.
	 */
	markInProgress(value: boolean): void {
		this.#inProgress = value;
	}

	/** Record an event. No-op outside study mode. */
	log(eventType: string, payload: Record<string, unknown> = {}): void {
		if (!this.active) return;
		this.#buffer.push({
			event_type: eventType,
			current_task_id: this.#currentTaskId,
			payload,
			client_ts: new Date().toISOString()
		});
		if (this.#buffer.length >= MAX_BUFFER) void this.flush();
	}

	/** Begin a session: emit session_start with environment metadata, arm flushing. */
	start(): void {
		if (!browser || !this.active || this.#started) return;
		this.#started = true;
		this.log('session_start', sessionMetadata());
		this.#timer = setInterval(() => void this.flush(), FLUSH_INTERVAL_MS);
		window.addEventListener('pagehide', this.#flushOnUnload);
		window.addEventListener('beforeunload', this.#flushOnUnload);
	}

	/** End a session cleanly (e.g. survey submitted): emit session_end and flush. */
	async end(): Promise<void> {
		if (!this.#started) return;
		this.log('session_end', {});
		await this.flush();
		this.#teardown();
	}

	/**
	 * Send buffered events. Re-queues on a network error or a server fault so a
	 * flaky connection doesn't lose data — but a 4xx is the server refusing this
	 * batch's *content*, and retrying it forever would poison the buffer and cost
	 * the participant every later event of the session. Drop those and move on.
	 */
	async flush(): Promise<void> {
		if (!this.active || this.#buffer.length === 0) return;
		const batch = this.#buffer;
		this.#buffer = [];
		try {
			await studyApi.events({ events: batch });
		} catch (err) {
			const status = err instanceof ApiError ? err.status : 0;
			if (status >= 400 && status < 500) {
				console.warn(`[telemetry] server rejected a batch of ${batch.length} events`, err);
				return;
			}
			this.#buffer = batch.concat(this.#buffer);
		}
	}

	#teardown(): void {
		if (this.#timer) clearInterval(this.#timer);
		this.#timer = null;
		this.#started = false;
		window.removeEventListener('pagehide', this.#flushOnUnload);
		window.removeEventListener('beforeunload', this.#flushOnUnload);
	}

	// On unload a normal async flush won't complete; use a keepalive fetch so the
	// final batch survives the page going away. sendBeacon can't set the auth
	// header, so we use fetch+keepalive with the current access token.
	#flushOnUnload = (): void => {
		// If the participant is mid-study, record an explicit dropout marker.
		if (this.#inProgress) {
			this.#buffer.push({
				event_type: 'session_abandoned',
				current_task_id: this.#currentTaskId,
				payload: { reason: 'unload' },
				client_ts: new Date().toISOString()
			});
		}
		if (this.#buffer.length === 0) return;
		const batch = this.#buffer;
		this.#buffer = [];
		const token = auth.accessToken;
		if (!token) return;
		void fetch(`${API_BASE}/api/v1/study/events`, {
			method: 'POST',
			keepalive: true,
			credentials: 'include',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${token}`
			},
			body: JSON.stringify({ events: batch })
		});
	};
}

function sessionMetadata(): Record<string, unknown> {
	if (!browser) return {};
	return {
		user_agent: navigator.userAgent,
		platform: navigator.platform,
		language: navigator.language,
		screen_width: window.screen?.width ?? null,
		screen_height: window.screen?.height ?? null,
		viewport_width: window.innerWidth,
		viewport_height: window.innerHeight
	};
}

export const telemetry = new Telemetry();
