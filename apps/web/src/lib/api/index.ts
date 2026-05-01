export { apiRequest, ApiError, type RequestOptions } from './client';
export * from './types';

import { apiRequest, type RequestOptions } from './client';
import type { LoginResponse, Project, Team, User } from './types';

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
	me: (opts?: RequestOptions) => apiRequest<User>('/api/v1/users/me', opts)
};

export const teams = {
	create: (body: { name: string }, opts?: RequestOptions) =>
		apiRequest<Team>('/api/v1/teams', { ...opts, method: 'POST', body })
};

export const projects = {
	listForTeam: (teamId: string, opts?: RequestOptions) =>
		apiRequest<Project[]>(`/api/v1/teams/${teamId}/projects`, opts),

	create: (teamId: string, body: { name: string; description?: string }, opts?: RequestOptions) =>
		apiRequest<Project>(`/api/v1/teams/${teamId}/projects`, {
			...opts,
			method: 'POST',
			body
		}),

	get: (id: string, opts?: RequestOptions) => apiRequest<Project>(`/api/v1/projects/${id}`, opts),

	update: (
		id: string,
		body: { name?: string; description?: string | null; github_repo?: string | null },
		opts?: RequestOptions
	) => apiRequest<Project>(`/api/v1/projects/${id}`, { ...opts, method: 'PATCH', body }),

	delete: (id: string, opts?: RequestOptions) =>
		apiRequest<void>(`/api/v1/projects/${id}`, { ...opts, method: 'DELETE' })
};
