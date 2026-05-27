import type { Task } from '$lib/api/types';

/**
 * Shared state driving the "Create pull request" dialog. The board's drag
 * handler calls `request(task)` when a card is dropped into the project's
 * Review column; the dialog component (mounted once on the board page) reacts.
 */
class CreatePrDialogState {
	task = $state<Task | null>(null);

	request(task: Task): void {
		this.task = task;
	}

	close(): void {
		this.task = null;
	}
}

export const createPrDialog = new CreatePrDialogState();
