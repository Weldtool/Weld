export { apiRequest, ApiError, type RequestOptions } from './client';
export * from './types';

import { apiRequest, type RequestOptions } from './client';
import { auth as authStore } from '$lib/auth.svelte';
import type { LoginResponse, Project, Team, User, UserTeam } from './types';

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
