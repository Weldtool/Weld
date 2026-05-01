import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: false }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

import { projects } from './projects.svelte';

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

const TEAM = {
	id: 't-1',
	name: 'My team',
	slug: 'my-team',
	owner_id: 'u-1',
	role: 'owner' as const,
	created_at: ''
};

const projectFixture = (over: Partial<{ id: string; name: string }> = {}) => ({
	id: 'p-1',
	team_id: TEAM.id,
	name: 'Alpha',
	slug: 'alpha',
	description: null,
	github_repo: null,
	created_at: '',
	updated_at: '',
	...over
});

beforeEach(() => {
	projects.reset();
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.clearAllMocks();
});

describe('projects store', () => {
	it('refresh loads the user team and its projects, selecting the first', async () => {
		mockSequence(
			{ status: 200, body: [TEAM] },
			{ status: 200, body: [projectFixture({ id: 'p-1', name: 'Alpha' })] }
		);

		await projects.refresh();

		expect(projects.team?.id).toBe(TEAM.id);
		expect(projects.projects).toHaveLength(1);
		expect(projects.selectedId).toBe('p-1');
		expect(projects.current?.name).toBe('Alpha');
	});

	it('create appends the new project and selects it', async () => {
		mockSequence(
			{ status: 200, body: [TEAM] },
			{ status: 200, body: [projectFixture({ id: 'p-1', name: 'Alpha' })] },
			{ status: 201, body: projectFixture({ id: 'p-2', name: 'Beta' }) }
		);

		await projects.refresh();
		await projects.create({ name: 'Beta' });

		expect(projects.projects.map((p) => p.id)).toEqual(['p-1', 'p-2']);
		expect(projects.selectedId).toBe('p-2');
	});

	it('remove rolls back the optimistic update when the server rejects', async () => {
		mockSequence(
			{ status: 200, body: [TEAM] },
			{ status: 200, body: [projectFixture({ id: 'p-1', name: 'Alpha' })] },
			{ status: 403, body: { error: 'forbidden' } }
		);

		await projects.refresh();
		await expect(projects.remove('p-1')).rejects.toMatchObject({ status: 403 });
		expect(projects.projects).toHaveLength(1);
	});

	it('reset clears all state', async () => {
		mockSequence({ status: 200, body: [TEAM] }, { status: 200, body: [projectFixture()] });
		await projects.refresh();
		projects.reset();

		expect(projects.team).toBeNull();
		expect(projects.projects).toEqual([]);
		expect(projects.selectedId).toBeNull();
	});
});
