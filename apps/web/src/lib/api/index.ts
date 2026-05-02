export { apiRequest, ApiError, type RequestOptions } from './client';
export * from './types';

import { apiRequest, type RequestOptions } from './client';
import { auth as authStore } from '$lib/auth.svelte';
import type {
	BoardState,
	Column,
	Label,
	LoginResponse,
	Priority,
	Project,
	Task,
	Team,
	User,
	UserTeam
} from './types';

/**
 * Authenticated namespaces (`users`, `teams`, `projects`) route through
 * `authStore.fetch`, which transparently refreshes-and-retries on 401.
 * The `auth` namespace bypasses that — its endpoints are the refresh
 * mechanism itself, and must not loop through it.
 */

export const auth = {
	register: (
		body: { email: string; password: string; display_name: string },
		opts?: RequestOptions
	) => apiRequest<User>('/api/v1/auth/register', { ...opts, method: 'POST', body }),

	login: (body: { email: string; password: string }, opts?: RequestOptions) =>
		apiRequest<LoginResponse>('/api/v1/auth/login', { ...opts, method: 'POST', body }),

	logout: (opts?: RequestOptions) =>
		apiRequest<void>('/api/v1/auth/logout', { ...opts, method: 'POST' }),

	refresh: (opts?: RequestOptions) =>
		apiRequest<LoginResponse>('/api/v1/auth/refresh', { ...opts, method: 'POST' })
};

export const users = {
	me: (opts?: RequestOptions) => authStore.fetch<User>('/api/v1/users/me', opts)
};

export const teams = {
	list: (opts?: RequestOptions) => authStore.fetch<UserTeam[]>('/api/v1/teams', opts),

	create: (body: { name: string }, opts?: RequestOptions) =>
		authStore.fetch<Team>('/api/v1/teams', { ...opts, method: 'POST', body })
};

export const projects = {
	listForTeam: (teamId: string, opts?: RequestOptions) =>
		authStore.fetch<Project[]>(`/api/v1/teams/${teamId}/projects`, opts),

	create: (teamId: string, body: { name: string; description?: string }, opts?: RequestOptions) =>
		authStore.fetch<Project>(`/api/v1/teams/${teamId}/projects`, {
			...opts,
			method: 'POST',
			body
		}),

	get: (id: string, opts?: RequestOptions) =>
		authStore.fetch<Project>(`/api/v1/projects/${id}`, opts),

	update: (
		id: string,
		body: { name?: string; description?: string | null; github_repo?: string | null },
		opts?: RequestOptions
	) => authStore.fetch<Project>(`/api/v1/projects/${id}`, { ...opts, method: 'PATCH', body }),

	delete: (id: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/projects/${id}`, { ...opts, method: 'DELETE' })
};

export const board = {
	get: (projectId: string, opts?: RequestOptions) =>
		authStore.fetch<BoardState>(`/api/v1/projects/${projectId}/board`, opts)
};

export const columns = {
	create: (
		projectId: string,
		body: { name: string; position?: number; wip_limit?: number | null },
		opts?: RequestOptions
	) =>
		authStore.fetch<Column>(`/api/v1/projects/${projectId}/columns`, {
			...opts,
			method: 'POST',
			body
		}),

	update: (
		id: string,
		body: { name?: string; position?: number; wip_limit?: number | null },
		opts?: RequestOptions
	) => authStore.fetch<Column>(`/api/v1/columns/${id}`, { ...opts, method: 'PATCH', body }),

	delete: (id: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/columns/${id}`, { ...opts, method: 'DELETE' })
};

export const labels = {
	create: (projectId: string, body: { name: string; color: string }, opts?: RequestOptions) =>
		authStore.fetch<Label>(`/api/v1/projects/${projectId}/labels`, {
			...opts,
			method: 'POST',
			body
		}),

	update: (id: string, body: { name?: string; color?: string }, opts?: RequestOptions) =>
		authStore.fetch<Label>(`/api/v1/labels/${id}`, { ...opts, method: 'PATCH', body }),

	delete: (id: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/labels/${id}`, { ...opts, method: 'DELETE' })
};

export const tasks = {
	create: (
		projectId: string,
		body: {
			title: string;
			column_id: string;
			description?: string;
			assignee_id?: string;
			priority?: Priority;
			labels?: string[];
			due_date?: string;
			estimate_hours?: number;
		},
		opts?: RequestOptions
	) =>
		authStore.fetch<Task>(`/api/v1/projects/${projectId}/tasks`, {
			...opts,
			method: 'POST',
			body
		}),

	get: (id: string, opts?: RequestOptions) => authStore.fetch<Task>(`/api/v1/tasks/${id}`, opts),

	update: (
		id: string,
		body: {
			title?: string;
			description?: string | null;
			assignee_id?: string | null;
			priority?: Priority;
			due_date?: string | null;
			estimate_hours?: number | null;
			labels?: string[];
		},
		opts?: RequestOptions
	) => authStore.fetch<Task>(`/api/v1/tasks/${id}`, { ...opts, method: 'PATCH', body }),

	delete: (id: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/tasks/${id}`, { ...opts, method: 'DELETE' }),

	move: (id: string, body: { column_id: string; position: number }, opts?: RequestOptions) =>
		authStore.fetch<Task>(`/api/v1/tasks/${id}/move`, {
			...opts,
			method: 'POST',
			body
		})
};
