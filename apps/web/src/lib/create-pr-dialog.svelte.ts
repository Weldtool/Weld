import type { Task } from '$lib/api/types';
import { boardStore } from '$lib/board.svelte';

/**
 * Shared state driving the "Create pull request" dialog. The board's drag
 * handler calls `request(task, originColumnId)` when a card is dropped into the
 * project's Review column; the dialog component (mounted once on the board page)
 * reacts.
 *
 * Review is meant to mean "has an open PR", so dismissing the dialog without
 * creating a PR rolls the card back to the column it came from. The task detail
 * panel opens the same dialog without an origin column, so cancelling there is a
 * no-op move-wise.
 */
class CreatePrDialogState {
	task = $state<Task | null>(null);
	// Column the card occupied before being dropped into Review. Null when the
	// dialog was opened manually (e.g. from the task detail panel).
	private originColumnId: string | null = null;

	request(task: Task, originColumnId?: string): void {
		this.task = task;
		this.originColumnId = originColumnId ?? null;
	}

	/** Call after a PR is successfully created — the move into Review is final. */
	confirm(): void {
		this.task = null;
		this.originColumnId = null;
	}

	/** Dismiss without creating a PR; revert the drop into Review if there was one. */
	close(): void {
		const task = this.task;
		const origin = this.originColumnId;
		this.task = null;
		this.originColumnId = null;
		if (task && origin && origin !== task.column_id) {
			void boardStore.revertMove(task.id, origin);
		}
	}
}

export const createPrDialog = new CreatePrDialogState();
