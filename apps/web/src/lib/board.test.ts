import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: false }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

import { ApiError } from '$lib/api/client';
import { boardStore, computePosition } from './board.svelte';
import type { Task } from '$lib/api/types';

interface MockResponse {
	status: number;
	body: unknown;
}

function jsonResponse({ status, body }: MockResponse): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'content-type': 'application/json' }
	});
}

function mockSequence(...responses: MockResponse[]) {
	const queue = [...responses];
	const fn = vi.fn(async () => {
		const next = queue.shift();
		if (!next) throw new Error('Unexpected fetch — no more mocked responses');
		return jsonResponse(next);
	});
	vi.stubGlobal('fetch', fn);
	return fn;
}

const taskFixture = (over: Partial<Task> = {}): Task => ({
	id: over.id ?? 'task-1',
	project_id: 'p-1',
	column_id: over.column_id ?? 'col-1',
	short_id: over.short_id ?? 1,
	title: over.title ?? 'Task',
	description: null,
	assignee_id: null,
	reporter_id: 'u-1',
	priority: over.priority ?? 'none',
	position: over.position ?? 1,
	due_date: null,
	estimate_hours: null,
	github_issue_id: null,
	github_pr_id: null,
	github_repo: null,
	labels: over.labels ?? [],
	created_at: '',
	updated_at: '',
	...over
});

beforeEach(() => {
	boardStore.reset();
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('computePosition', () => {
	it('returns 1 for an empty column', () => {
		expect(computePosition(undefined, undefined)).toBe(1);
	});
	it('appends after the last task', () => {
		expect(computePosition(5, undefined)).toBe(6);
	});
	it('inserts at the start by halving the first', () => {
		expect(computePosition(undefined, 4)).toBe(2);
	});
	it('drops between two neighbours at the midpoint', () => {
		expect(computePosition(2, 3)).toBe(2.5);
	});
});

describe('filteredTasks', () => {
	beforeEach(() => {
		boardStore.tasks = [
			taskFixture({ id: 't1', title: 'Auth bug', priority: 'high', labels: ['L1'] }),
			taskFixture({ id: 't2', title: 'Docs polish', priority: 'low', labels: ['L2'] }),
			taskFixture({ id: 't3', title: 'Auth refactor', priority: 'high', labels: [] })
		];
	});

	it('filters by search across title and description', () => {
		boardStore.setFilter('search', 'auth');
		const ids = boardStore.filteredTasks.map((t) => t.id);
		expect(ids).toEqual(['t1', 't3']);
	});

	it('filters by priority', () => {
		boardStore.setFilter('priority', 'low');
		expect(boardStore.filteredTasks.map((t) => t.id)).toEqual(['t2']);
	});

	it('filters by label membership', () => {
		boardStore.setFilter('labelId', 'L1');
		expect(boardStore.filteredTasks.map((t) => t.id)).toEqual(['t1']);
	});

	it('clearFilters resets the filter state', () => {
		boardStore.setFilter('search', 'auth');
		boardStore.clearFilters();
		expect(boardStore.filteredTasks).toHaveLength(3);
	});
});

describe('moveTask', () => {
	beforeEach(() => {
		boardStore.projectId = 'p-1';
		boardStore.tasks = [
			taskFixture({ id: 't1', column_id: 'col-1', position: 1 }),
			taskFixture({ id: 't2', column_id: 'col-2', position: 1 })
		];
	});

	it('rolls back local state when the server rejects the move', async () => {
		mockSequence({ status: 500, body: { error: 'boom' } });
		const ok = await boardStore.moveTask('t1', 'col-2', 0.5);
		expect(ok).toBe(false);
		const t1 = boardStore.tasks.find((t) => t.id === 't1');
		expect(t1?.column_id).toBe('col-1');
		expect(t1?.position).toBe(1);
		expect(boardStore.error).toContain('boom');
	});

	it('refetches the board on a 409 rebalance response', async () => {
		mockSequence(
			{ status: 409, body: { error: 'rebalance required' } },
			{
				status: 200,
				body: {
					columns: [
						{
							id: 'col-1',
							project_id: 'p-1',
							name: 'Backlog',
							position: 1,
							wip_limit: null,
							created_at: '',
							updated_at: ''
						}
					],
					tasks: [taskFixture({ id: 't1', column_id: 'col-1', position: 1 })],
					labels: []
				}
			}
		);
		const ok = await boardStore.moveTask('t1', 'col-2', 0.5);
		expect(ok).toBe(false);
		expect(boardStore.error).toContain('rebalance');
		expect(boardStore.columns).toHaveLength(1);
	});

	it('keeps the moved task on success', async () => {
		mockSequence({
			status: 200,
			body: { ...taskFixture({ id: 't1', column_id: 'col-2', position: 0.5 }) }
		});
		const ok = await boardStore.moveTask('t1', 'col-2', 0.5);
		expect(ok).toBe(true);
		const moved = boardStore.tasks.find((t) => t.id === 't1');
		expect(moved?.column_id).toBe('col-2');
		expect(moved?.position).toBe(0.5);
	});
});

describe('ApiError surface', () => {
	it('exposes status for 409 detection', () => {
		const err = new ApiError(409, { error: 'rebalance required' });
		expect(err.status).toBe(409);
	});
});
