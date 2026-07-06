import { browser } from '$app/environment';

/**
 * Measures *active* time-on-task for the study, not raw wall-clock.
 *
 * The naive `Date.now() - startedAt` approach counts every second the task is
 * open — including time the participant is away (tab backgrounded, laptop shut,
 * or the tab closed and reopened later). That inflates the primary outcome
 * (task completion time) and breaks across reloads. This clock instead:
 *
 *   - runs only while the tab is visible (pauses on `visibilitychange` → hidden
 *     and on `pagehide`, resumes when visible again), so away-time is excluded;
 *   - persists accumulated active time to localStorage, so closing and
 *     re-entering the study resumes the same task's clock instead of losing it
 *     or restarting from zero;
 *   - counts interruptions (`pause_count`) and exposes `was_interrupted` so a
 *     task spread across a long gap can be excluded or sensitivity-checked in
 *     analysis, even though its measured time already excludes the gap.
 *
 * Only one task is timed at a time. `start()` a task, then `stop()` returns its
 * timing summary. Reads `document.visibilityState`; no-ops outside the browser.
 */

const KEY = 'weld:study:taskclock';
// While visible, re-persist this often so an abrupt crash loses at most this
// much active time (normal close/hide is captured exactly via the pause path).
const PERSIST_INTERVAL_MS = 5000;

interface ClockSnapshot {
	taskId: string;
	/** Active (visible) milliseconds banked so far. */
	activeMs: number;
	/** First-ever start timestamp for this task (epoch ms), for wall/away calc. */
	startedWall: number;
	/** Number of times the task was paused/interrupted (hidden, closed, reloaded). */
	pauseCount: number;
}

export interface TaskTiming {
	/** Primary outcome: engaged time, excluding away periods. */
	active_ms: number;
	/** Raw wall-clock from first start to stop (for transparency/auditing). */
	wall_ms: number;
	/** wall_ms − active_ms: time the participant was away mid-task. */
	away_ms: number;
	/** How many times the task was interrupted (backgrounded/closed/reloaded). */
	pause_count: number;
	/** True if the task was interrupted at all — a flag for excluding it. */
	was_interrupted: boolean;
}

class TaskClock {
	#taskId: string | null = null;
	#activeMs = 0;
	#startedWall = 0;
	#pauseCount = 0;
	/** When the current visible segment began (epoch ms), or null while paused. */
	#resumedAt: number | null = null;
	#timer: ReturnType<typeof setInterval> | null = null;
	#listening = false;

	/**
	 * Begin (or resume) timing `taskId`. If a persisted clock for the *same*
	 * task exists (a reload/re-entry mid-task), its banked active time is
	 * restored and the reload is counted as one interruption; otherwise a fresh
	 * clock starts. A stale clock for a different task is discarded.
	 */
	start(taskId: string, opts: { resumedMidTask?: boolean } = {}): void {
		if (!browser) return;
		const now = Date.now();
		const prior = this.#load();
		if (prior && prior.taskId === taskId) {
			// Reload / re-entry on the same task: keep banked active time, exclude
			// the gap, and record the discontinuity as an interruption.
			this.#activeMs = prior.activeMs;
			this.#startedWall = prior.startedWall;
			this.#pauseCount = prior.pauseCount + 1;
		} else {
			this.#activeMs = 0;
			this.#startedWall = now;
			// A cross-device resume (no local snapshot) still means the task was
			// interrupted; flag it so the truncated time can be excluded.
			this.#pauseCount = opts.resumedMidTask ? 1 : 0;
		}
		this.#taskId = taskId;
		this.#resumedAt = this.#isVisible() ? now : null;
		this.#persist();
		this.#attach();
	}

	/** Stop timing the current task and return its timing summary. */
	stop(): TaskTiming | null {
		if (!browser || this.#taskId === null) return null;
		const now = Date.now();
		this.#fold(now);
		const activeMs = this.#activeMs;
		const wallMs = now - this.#startedWall;
		const pauseCount = this.#pauseCount;
		this.#detach();
		this.#taskId = null;
		this.#resumedAt = null;
		this.#clear();
		return {
			active_ms: activeMs,
			wall_ms: wallMs,
			away_ms: Math.max(0, wallMs - activeMs),
			pause_count: pauseCount,
			was_interrupted: pauseCount > 0
		};
	}

	// --- internals -----------------------------------------------------------

	/** Bank the elapsed visible segment without stopping the clock. */
	#fold(now: number): void {
		if (this.#resumedAt !== null) {
			this.#activeMs += now - this.#resumedAt;
			this.#resumedAt = now;
		}
	}

	#pause = (): void => {
		if (this.#taskId === null) return;
		const wasRunning = this.#resumedAt !== null;
		this.#fold(Date.now());
		this.#resumedAt = null;
		if (wasRunning) this.#pauseCount += 1;
		this.#persist();
	};

	#resume = (): void => {
		if (this.#taskId === null || this.#resumedAt !== null) return;
		this.#resumedAt = Date.now();
		this.#persist();
	};

	#onVisibility = (): void => {
		if (this.#isVisible()) this.#resume();
		else this.#pause();
	};

	#isVisible(): boolean {
		return typeof document === 'undefined' || document.visibilityState !== 'hidden';
	}

	#attach(): void {
		if (this.#listening) return;
		document.addEventListener('visibilitychange', this.#onVisibility);
		window.addEventListener('pagehide', this.#pause);
		this.#timer = setInterval(() => {
			// Periodic fold+persist so a hard crash while visible loses ≤ one interval.
			this.#fold(Date.now());
			this.#persist();
		}, PERSIST_INTERVAL_MS);
		this.#listening = true;
	}

	#detach(): void {
		if (!this.#listening) return;
		document.removeEventListener('visibilitychange', this.#onVisibility);
		window.removeEventListener('pagehide', this.#pause);
		if (this.#timer !== null) clearInterval(this.#timer);
		this.#timer = null;
		this.#listening = false;
	}

	#persist(): void {
		if (this.#taskId === null) return;
		const snap: ClockSnapshot = {
			taskId: this.#taskId,
			activeMs: this.#activeMs,
			startedWall: this.#startedWall,
			pauseCount: this.#pauseCount
		};
		try {
			localStorage.setItem(KEY, JSON.stringify(snap));
		} catch {
			// Storage unavailable — clock still works in-memory for this session.
		}
	}

	#load(): ClockSnapshot | null {
		try {
			const raw = localStorage.getItem(KEY);
			return raw ? (JSON.parse(raw) as ClockSnapshot) : null;
		} catch {
			return null;
		}
	}

	#clear(): void {
		try {
			localStorage.removeItem(KEY);
		} catch {
			// ignore
		}
	}
}

export const taskClock = new TaskClock();
