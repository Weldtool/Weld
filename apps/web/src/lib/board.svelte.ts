import {
	board as boardApi,
	columns as columnsApi,
	labels as labelsApi,
	tasks as tasksApi,
	teams as teamsApi
} from '$lib/api';
import type { Column, Label, Priority, Task, TeamMember } from '$lib/api/types';
import { realtime, type RealtimeEnvelope } from '$lib/realtime.svelte';
import { projects } from '$lib/projects.svelte';

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
	members = $state<TeamMember[]>([]);
	filters = $state<BoardFilters>({ ...EMPTY_FILTERS });
	loading = $state(false);
	error = $state<string | null>(null);

	taskById(id: string): Task | undefined {
		return this.tasks.find((t) => t.id === id);
	}

	applyTaskUpdate(task: Task): void {
		this.tasks = this.tasks.some((t) => t.id === task.id)
			? this.tasks.map((t) => (t.id === task.id ? task : t))
			: [...this.tasks, task];
	}

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

	#unsubs: Array<() => void> = [];
	#joinedRoom: string | null = null;

	async loadBoard(projectId: string): Promise<void> {
		this.loading = true;
		this.error = null;
		try {
			const data = await boardApi.get(projectId);
			this.projectId = projectId;
			this.columns = [...data.columns].sort((a, b) => a.position - b.position);
			this.tasks = data.tasks;
			this.labels = data.labels;
			this.#subscribeRealtime(projectId);
			void this.#loadMembers();
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
		this.#teardownRealtime();
		this.projectId = null;
		this.columns = [];
		this.tasks = [];
		this.labels = [];
		this.members = [];
		this.filters = { ...EMPTY_FILTERS };
		this.error = null;
	}

	async #loadMembers(): Promise<void> {
		const teamId = projects.team?.id;
		if (!teamId) return;
		try {
			this.members = await teamsApi.listMembers(teamId);
		} catch {
			// Members are nice-to-have for the assignee picker; don't block
			// the board if the request fails.
		}
	}

	#subscribeRealtime(projectId: string): void {
		const room = `project:${projectId}`;
		if (this.#joinedRoom === room) return;
		this.#teardownRealtime();
		realtime.join(room);
		this.#joinedRoom = room;
		this.#unsubs = [
			realtime.on('task.created', (e) => this.#applyTaskUpsert(e)),
			realtime.on('task.updated', (e) => this.#applyTaskUpsert(e)),
			realtime.on('task.moved', (e) => this.#applyTaskUpsert(e)),
			realtime.on('task.deleted', (e) => this.#applyTaskDelete(e)),
			realtime.on('column.created', (e) => this.#applyColumnUpsert(e)),
			realtime.on('column.updated', (e) => this.#applyColumnUpsert(e)),
			realtime.on('column.deleted', (e) => this.#applyColumnDelete(e)),
			realtime.on('label.created', (e) => this.#applyLabelUpsert(e)),
			realtime.on('label.updated', (e) => this.#applyLabelUpsert(e)),
			realtime.on('label.deleted', (e) => this.#applyLabelDelete(e))
		];
	}

	#teardownRealtime(): void {
		for (const unsub of this.#unsubs) unsub();
		this.#unsubs = [];
		if (this.#joinedRoom) {
			realtime.leave(this.#joinedRoom);
			this.#joinedRoom = null;
		}
	}

	#expectedRoom(): string | null {
		return this.projectId ? `project:${this.projectId}` : null;
	}

	#applyTaskUpsert(envelope: RealtimeEnvelope): void {
		if (envelope.room !== this.#expectedRoom()) return;
		const incoming = envelope.payload as unknown as Task;
		const existing = this.tasks.find((t) => t.id === incoming.id);
		// Server is source of truth: take its position/column even if our
		// optimistic update placed the task elsewhere.
		if (existing) {
			this.tasks = this.tasks.map((t) => (t.id === incoming.id ? incoming : t));
		} else {
			this.tasks = [...this.tasks, incoming];
		}
	}

	#applyTaskDelete(envelope: RealtimeEnvelope): void {
		if (envelope.room !== this.#expectedRoom()) return;
		const id = envelope.payload.task_id as string | undefined;
		if (!id) return;
		this.tasks = this.tasks.filter((t) => t.id !== id);
	}

	#applyColumnUpsert(envelope: RealtimeEnvelope): void {
		if (envelope.room !== this.#expectedRoom()) return;
		const incoming = envelope.payload as unknown as Column;
		const next = this.columns.some((c) => c.id === incoming.id)
			? this.columns.map((c) => (c.id === incoming.id ? incoming : c))
			: [...this.columns, incoming];
		this.columns = next.sort((a, b) => a.position - b.position);
	}

	#applyColumnDelete(envelope: RealtimeEnvelope): void {
		if (envelope.room !== this.#expectedRoom()) return;
		const id = envelope.payload.column_id as string | undefined;
		if (!id) return;
		this.columns = this.columns.filter((c) => c.id !== id);
	}

	#applyLabelUpsert(envelope: RealtimeEnvelope): void {
		if (envelope.room !== this.#expectedRoom()) return;
		const incoming = envelope.payload as unknown as Label;
		this.labels = this.labels.some((l) => l.id === incoming.id)
			? this.labels.map((l) => (l.id === incoming.id ? incoming : l))
			: [...this.labels, incoming];
	}

	#applyLabelDelete(envelope: RealtimeEnvelope): void {
		if (envelope.room !== this.#expectedRoom()) return;
		const id = envelope.payload.label_id as string | undefined;
		if (!id) return;
		this.labels = this.labels.filter((l) => l.id !== id);
		this.tasks = this.tasks.map((t) =>
			t.labels.includes(id) ? { ...t, labels: t.labels.filter((l) => l !== id) } : t
		);
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

	async createTask(input: Parameters<typeof tasksApi.create>[1]): Promise<Task | null> {
		if (!this.projectId) return null;
		const created = await tasksApi.create(this.projectId, input);
		// Guard against the WS task.created event arriving first and already inserting.
		if (!this.tasks.some((t) => t.id === created.id)) {
			this.tasks = [...this.tasks, { ...created, labels: created.labels ?? [] }];
		}
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
