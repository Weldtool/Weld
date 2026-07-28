import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: false }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

import type { Task } from '$lib/api/types';
import { boardStore } from './board.svelte';
import { canEditTask, TaskSaver } from './task-detail.svelte';

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

const taskFixture = (over: Partial<Task> = {}): Task => ({
	id: 't1',
	project_id: 'p-1',
	column_id: 'col-1',
	short_id: 1,
	title: 'Original',
	description: null,
	assignee_id: null,
	reporter_id: 'u-1',
	priority: 'none',
	position: 1,
	due_date: null,
	estimate_hours: null,
	github_issue_id: null,
	github_pr_id: null,
	github_check_status: null,
	github_repo: null,
	labels: [],
	created_at: '',
	updated_at: '',
	...over
});

beforeEach(() => {
	boardStore.reset();
	boardStore.tasks = [taskFixture()];
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('canEditTask', () => {
	it('lets owners/admins/members edit any task', () => {
		const t = taskFixture({ reporter_id: 'other', assignee_id: null });
		expect(canEditTask(t, 'owner', 'me')).toBe(true);
		expect(canEditTask(t, 'admin', 'me')).toBe(true);
		expect(canEditTask(t, 'member', 'me')).toBe(true);
	});

	it('lets viewers edit only their own tasks', () => {
		expect(canEditTask(taskFixture({ reporter_id: 'me' }), 'viewer', 'me')).toBe(true);
		expect(canEditTask(taskFixture({ assignee_id: 'me' }), 'viewer', 'me')).toBe(true);
		expect(canEditTask(taskFixture({ reporter_id: 'other' }), 'viewer', 'me')).toBe(false);
	});
});

describe('TaskSaver debounce', () => {
	it('coalesces rapid edits into a single PATCH after the debounce window', async () => {
		const captured: RequestInit[] = [];
		const fetchMock = vi.fn(async (_url: unknown, init?: RequestInit) => {
			if (init) captured.push(init);
			return jsonResponse({ status: 200, body: taskFixture({ title: 'Final' }) });
		});
		vi.stubGlobal('fetch', fetchMock);

		const saver = new TaskSaver(() => 't1', 200);
		saver.queue({ title: 'A' });
		saver.queue({ title: 'AB' });
		saver.queue({ title: 'Final' });

		expect(fetchMock).not.toHaveBeenCalled();
		expect(saver.saving).toBe(true);

		await vi.advanceTimersByTimeAsync(200);
		await Promise.resolve();

		expect(fetchMock).toHaveBeenCalledTimes(1);
		const body = JSON.parse(captured[0].body as string);
		expect(body).toEqual({ title: 'Final' });
		expect(boardStore.tasks[0].title).toBe('Final');
	});

	it('re-flushes when more edits arrive during an in-flight request', async () => {
		let resolveFirst: (r: Response) => void = () => {};
		const calls: string[] = [];
		const fetchMock = vi.fn((_url: unknown, init?: RequestInit) => {
			if (!init) throw new Error('expected init');
			const body = JSON.parse(init.body as string);
			calls.push(body.title);
			if (calls.length === 1) {
				return new Promise<Response>((resolve) => {
					resolveFirst = resolve;
				});
			}
			return Promise.resolve(
				jsonResponse({ status: 200, body: taskFixture({ title: body.title }) })
			);
		});
		vi.stubGlobal('fetch', fetchMock);

		const saver = new TaskSaver(() => 't1', 100);
		saver.queue({ title: 'first' });
		await vi.advanceTimersByTimeAsync(100);
		// First request is in-flight; queue another edit.
		saver.queue({ title: 'second' });
		await vi.advanceTimersByTimeAsync(100);
		// Resolve the first; saver should immediately flush the second.
		resolveFirst(jsonResponse({ status: 200, body: taskFixture({ title: 'first' }) }));
		await vi.advanceTimersByTimeAsync(0);
		await Promise.resolve();
		await Promise.resolve();

		expect(calls).toEqual(['first', 'second']);
	});

	it('surfaces an error message when the PATCH fails', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => jsonResponse({ status: 500, body: { error: 'nope' } }))
		);
		const saver = new TaskSaver(() => 't1', 50);
		saver.queue({ title: 'x' });
		await vi.advanceTimersByTimeAsync(50);
		await Promise.resolve();
		expect(saver.error).toContain('nope');
	});
});

describe('focus-preservation conflict resolution', () => {
	it('applyTaskUpdate replaces the store task so derived UI sees fresh values', () => {
		boardStore.applyTaskUpdate(taskFixture({ title: 'Server-updated' }));
		expect(boardStore.taskById('t1')?.title).toBe('Server-updated');
	});

	it('preserves locally-typed text by skipping store sync while focused', () => {
		// Simulates the $effect logic in TaskDetail.svelte: when a field is
		// focused, we must NOT mirror the store value into local state.
		let titleFocused = true;
		let titleLocal = 'user typing in progress';
		const task = taskFixture({ title: 'incoming from WS' });

		const sync = () => {
			if (!titleFocused) titleLocal = task.title;
		};

		sync();
		expect(titleLocal).toBe('user typing in progress');

		// Once the field blurs, the next sync picks up the latest server value.
		titleFocused = false;
		sync();
		expect(titleLocal).toBe('incoming from WS');
	});
});
