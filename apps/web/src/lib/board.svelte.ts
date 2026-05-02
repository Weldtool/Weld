import {
	board as boardApi,
	columns as columnsApi,
	labels as labelsApi,
	tasks as tasksApi
} from '$lib/api';
import type { Column, Label, Priority, Task } from '$lib/api/types';

export interface BoardFilters {
	search: string;
	assigneeId: string | null;
	labelId: string | null;
	priority: Priority | null;
}

const EMPTY_FILTERS: BoardFilters = {
	search: '',
	assigneeId: null,
	labelId: null,
	priority: null
};

/**
 * Compute a fractional `position` placing a task between `before` and `after`.
 * Either may be undefined: appending to the end of a column → `before + 1`,
 * inserting at the start → `after / 2`, empty column → `1`.
 */
export function computePosition(before: number | undefined, after: number | undefined): number {
	if (before === undefined && after === undefined) return 1;
	if (before === undefined) return (after as number) / 2;
	if (after === undefined) return before + 1;
	return (before + after) / 2;
}

class BoardStore {
	projectId = $state<string | null>(null);
	columns = $state<Column[]>([]);
	tasks = $state<Task[]>([]);
	labels = $state<Label[]>([]);
	filters = $state<BoardFilters>({ ...EMPTY_FILTERS });
	loading = $state(false);
	error = $state<string | null>(null);

	get filteredTasks(): Task[] {
		const f = this.filters;
		const search = f.search.trim().toLowerCase();
		return this.tasks.filter((t) => {
			if (f.assigneeId && t.assignee_id !== f.assigneeId) return false;
			if (f.priority && t.priority !== f.priority) return false;
			if (f.labelId && !t.labels.includes(f.labelId)) return false;
			if (search) {
				const haystack = `${t.title} ${t.description ?? ''}`.toLowerCase();
				if (!haystack.includes(search)) return false;
			}
			return true;
		});
	}

	tasksForColumn(columnId: string): Task[] {
		return this.filteredTasks
			.filter((t) => t.column_id === columnId)
			.sort((a, b) => a.position - b.position);
	}

	async loadBoard(projectId: string): Promise<void> {
		this.loading = true;
		this.error = null;
		try {
			const data = await boardApi.get(projectId);
			this.projectId = projectId;
			this.columns = [...data.columns].sort((a, b) => a.position - b.position);
			this.tasks = data.tasks;
			this.labels = data.labels;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load board.';
		} finally {
			this.loading = false;
		}
	}

	async refresh(): Promise<void> {
		if (this.projectId) await this.loadBoard(this.projectId);
	}

	reset(): void {
		this.projectId = null;
		this.columns = [];
		this.tasks = [];
		this.labels = [];
		this.filters = { ...EMPTY_FILTERS };
		this.error = null;
	}

	setFilter<K extends keyof BoardFilters>(key: K, value: BoardFilters[K]): void {
		this.filters = { ...this.filters, [key]: value };
	}

	clearFilters(): void {
		this.filters = { ...EMPTY_FILTERS };
	}

	/**
	 * Optimistically move a task: update locally, call the API, roll back on
	 * failure. Returns true on success. On a 409 "rebalance required" we
	 * refetch the board so the client picks up freshly spaced positions.
	 */
	async moveTask(taskId: string, toColumnId: string, position: number): Promise<boolean> {
		const previous = this.tasks;
		const moved = previous.find((t) => t.id === taskId);
		if (!moved) return false;
		this.tasks = previous.map((t) =>
			t.id === taskId ? { ...t, column_id: toColumnId, position } : t
		);
		try {
			const updated = await tasksApi.move(taskId, { column_id: toColumnId, position });
			this.tasks = this.tasks.map((t) => (t.id === taskId ? updated : t));
			return true;
		} catch (err) {
			this.tasks = previous;
			const status = (err as { status?: number } | null)?.status;
			if (status === 409) {
				await this.refresh();
				this.error = 'Board needed a rebalance — reloaded.';
			} else {
				this.error = err instanceof Error ? err.message : 'Failed to move task.';
			}
			return false;
		}
	}

	async createTask(columnId: string, title: string): Promise<Task | null> {
		if (!this.projectId) return null;
		const created = await tasksApi.create(this.projectId, { title, column_id: columnId });
		this.tasks = [...this.tasks, { ...created, labels: created.labels ?? [] }];
		return created;
	}

	async updateTask(id: string, patch: Parameters<typeof tasksApi.update>[1]): Promise<Task> {
		const updated = await tasksApi.update(id, patch);
		this.tasks = this.tasks.map((t) => (t.id === id ? updated : t));
		return updated;
	}

	async deleteTask(id: string): Promise<void> {
		const previous = this.tasks;
		this.tasks = previous.filter((t) => t.id !== id);
		try {
			await tasksApi.delete(id);
		} catch (err) {
			this.tasks = previous;
			throw err;
		}
	}

	async createColumn(name: string): Promise<Column | null> {
		if (!this.projectId) return null;
		const created = await columnsApi.create(this.projectId, { name });
		this.columns = [...this.columns, created].sort((a, b) => a.position - b.position);
		return created;
	}

	async updateColumn(id: string, patch: Parameters<typeof columnsApi.update>[1]): Promise<Column> {
		const updated = await columnsApi.update(id, patch);
		this.columns = this.columns
			.map((c) => (c.id === id ? updated : c))
			.sort((a, b) => a.position - b.position);
		return updated;
	}

	async deleteColumn(id: string): Promise<void> {
		await columnsApi.delete(id);
		this.columns = this.columns.filter((c) => c.id !== id);
	}

	async createLabel(name: string, color: string): Promise<Label | null> {
		if (!this.projectId) return null;
		const created = await labelsApi.create(this.projectId, { name, color });
		this.labels = [...this.labels, created];
		return created;
	}

	async updateLabel(id: string, patch: Parameters<typeof labelsApi.update>[1]): Promise<Label> {
		const updated = await labelsApi.update(id, patch);
		this.labels = this.labels.map((l) => (l.id === id ? updated : l));
		return updated;
	}

	async deleteLabel(id: string): Promise<void> {
		await labelsApi.delete(id);
		this.labels = this.labels.filter((l) => l.id !== id);
		this.tasks = this.tasks.map((t) =>
			t.labels.includes(id) ? { ...t, labels: t.labels.filter((l) => l !== id) } : t
		);
	}
}

export const boardStore = new BoardStore();
