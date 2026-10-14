import { tasks as tasksApi } from '$lib/api';
import { ApiError } from '$lib/api/client';
import type { Priority, Task, TeamRole } from '$lib/api/types';
import { boardStore } from '$lib/board.svelte';
import { telemetry } from '$lib/telemetry.svelte';

/** Whether the caller may edit a task in this project. Mirrors the server's
 *  rule: Member+ can edit anyone's task; Viewer can only edit a task they
 *  authored or are assigned. */
export function canEditTask(task: Task, role: TeamRole, callerId: string): boolean {
	if (role === 'owner' || role === 'admin' || role === 'member') return true;
	return task.reporter_id === callerId || task.assignee_id === callerId;
}

export interface PatchPayload {
	title?: string;
	description?: string | null;
	assignee_id?: string | null;
	priority?: Priority;
	due_date?: string | null;
	estimate_hours?: number | null;
	labels?: string[];
}

/**
 * Send a debounced PATCH and reconcile the result back through the board
 * store. Pending edits coalesce: while a request is in flight, later changes
 * are merged into the next PATCH. On error, rolls back to the latest
 * server-authoritative task and surfaces the message.
 */
export class TaskSaver {
	#timer: ReturnType<typeof setTimeout> | null = null;
	#pending: PatchPayload = {};
	#inflight = false;
	saving = $state(false);
	error = $state<string | null>(null);

	constructor(
		private getTaskId: () => string,
		private debounceMs = 500
	) {}

	queue(patch: PatchPayload): void {
		this.#pending = { ...this.#pending, ...patch };
		this.saving = true;
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = setTimeout(() => {
			this.#timer = null;
			void this.flush();
		}, this.debounceMs);
	}

	async flush(): Promise<void> {
		if (this.#timer) {
			clearTimeout(this.#timer);
			this.#timer = null;
		}
		if (this.#inflight || Object.keys(this.#pending).length === 0) {
			if (Object.keys(this.#pending).length === 0) this.saving = false;
			return;
		}
		const payload = this.#pending;
		this.#pending = {};
		this.#inflight = true;
		// Snapshot before-values so the telemetry below can report what changed.
		// This is the primary edit path from the task detail panel (priority for
		// T1/T4, assignee, etc.); it bypasses board.updateTask, so it logs its own.
		const before = boardStore.taskById(this.getTaskId());
		try {
			const updated = await tasksApi.update(this.getTaskId(), payload);
			boardStore.applyTaskUpdate(updated);
			this.error = null;
			const base = { task_id: updated.id, short_id: updated.short_id, title: updated.title };
			if ('priority' in payload && payload.priority !== before?.priority) {
				telemetry.log('card_priority_set', {
					...base,
					from: before?.priority ?? null,
					to: updated.priority
				});
			}
			if ('assignee_id' in payload && payload.assignee_id !== before?.assignee_id) {
				telemetry.log('card_assigned', { ...base, to: updated.assignee_id });
			}
			if ('labels' in payload) {
				telemetry.log('card_labeled', { ...base, labels: updated.labels });
			}
			if (
				'title' in payload ||
				'description' in payload ||
				'due_date' in payload ||
				'estimate_hours' in payload
			) {
				telemetry.log('card_edited', { ...base, fields: Object.keys(payload) });
			}
		} catch (err) {
			this.error =
				err instanceof ApiError ? `Save failed: ${err.message}` : 'Save failed. Please try again.';
		} finally {
			this.#inflight = false;
			if (Object.keys(this.#pending).length > 0) {
				// More changes accumulated during the request — flush again.
				void this.flush();
			} else {
				this.saving = false;
			}
		}
	}
}
