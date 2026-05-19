export { apiRequest, ApiError, type RequestOptions } from './client';
export * from './types';

import { apiRequest, type RequestOptions } from './client';
import { auth as authStore } from '$lib/auth.svelte';
import type {
	BoardState,
	Channel,
	ChatMessage,
	Column,
	GithubConnection,
	GitHubIssue,
	GitHubIssueDetail,
	GitHubPR,
	Label,
	LoginResponse,
	MessagePage,
	Priority,
	Project,
	Task,
	Team,
	TeamMember,
	ThreadView,
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
		authStore.fetch<Team>('/api/v1/teams', { ...opts, method: 'POST', body }),

	listMembers: (teamId: string, opts?: RequestOptions) =>
		authStore.fetch<TeamMember[]>(`/api/v1/teams/${teamId}/members`, opts),

	addMember: (
		teamId: string,
		body: { email?: string; user_id?: string; role: 'owner' | 'admin' | 'member' | 'viewer' },
		opts?: RequestOptions
	) =>
		authStore.fetch<TeamMember>(`/api/v1/teams/${teamId}/members`, {
			...opts,
			method: 'POST',
			body
		}),

	removeMember: (teamId: string, userId: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/teams/${teamId}/members/${userId}`, {
			...opts,
			method: 'DELETE'
		})
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
		body: {
			name?: string;
			description?: string | null;
			github_repo?: string | null;
			done_column_id?: string | null;
			auto_close_on_issue_closed?: boolean;
		},
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
		}),

	link: (
		id: string,
		body: { repo: string; issue_number?: number | null; pr_number?: number | null },
		opts?: RequestOptions
	) => authStore.fetch<Task>(`/api/v1/tasks/${id}/link`, { ...opts, method: 'POST', body }),

	unlink: (id: string, opts?: RequestOptions) =>
		authStore.fetch<Task>(`/api/v1/tasks/${id}/link`, { ...opts, method: 'DELETE' }),

	pushToGithub: (id: string, opts?: RequestOptions) =>
		authStore.fetch<Task>(`/api/v1/tasks/${id}/push-to-github`, { ...opts, method: 'POST' })
};

export const github = {
	getInstallUrl: (teamId: string, opts?: RequestOptions) =>
		authStore.fetch<{ url: string }>(`/api/v1/teams/${teamId}/github/install-url`, opts),

	getConnection: (teamId: string, opts?: RequestOptions) =>
		authStore.fetch<GithubConnection>(`/api/v1/teams/${teamId}/github`, opts),

	disconnect: (teamId: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/teams/${teamId}/github`, { ...opts, method: 'DELETE' }),

	getOAuthUrl: (opts?: RequestOptions) =>
		authStore.fetch<{ url: string }>('/api/v1/github/oauth/url', opts),

	listIssues: (projectId: string, opts?: RequestOptions) =>
		authStore.fetch<GitHubIssue[]>(`/api/v1/projects/${projectId}/github/issues`, opts),

	getIssue: (projectId: string, number: number, opts?: RequestOptions) =>
		authStore.fetch<GitHubIssueDetail>(
			`/api/v1/projects/${projectId}/github/issues/${number}`,
			opts
		),

	listPRs: (projectId: string, opts?: RequestOptions) =>
		authStore.fetch<GitHubPR[]>(`/api/v1/projects/${projectId}/github/prs`, opts),

	syncLabels: (projectId: string, opts?: RequestOptions) =>
		authStore.fetch<Label[]>(`/api/v1/projects/${projectId}/github/labels/sync`, {
			...opts,
			method: 'POST'
		})
};

export const channels = {
	listForTeam: (teamId: string, opts?: RequestOptions) =>
		authStore.fetch<Channel[]>(`/api/v1/teams/${teamId}/channels`, opts),

	create: (
		teamId: string,
		body: {
			name: string;
			topic?: string;
			project_id?: string;
			is_private?: boolean;
			is_direct?: boolean;
			members?: string[];
		},
		opts?: RequestOptions
	) =>
		authStore.fetch<Channel>(`/api/v1/teams/${teamId}/channels`, {
			...opts,
			method: 'POST',
			body
		}),

	get: (id: string, opts?: RequestOptions) =>
		authStore.fetch<Channel>(`/api/v1/channels/${id}`, opts),

	update: (
		id: string,
		body: {
			name?: string;
			topic?: string | null;
			github_repos?: string[];
			github_event_filter?: { pr?: boolean; issue?: boolean; review?: boolean; push?: boolean };
		},
		opts?: RequestOptions
	) => authStore.fetch<Channel>(`/api/v1/channels/${id}`, { ...opts, method: 'PATCH', body }),

	archive: (id: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/channels/${id}`, { ...opts, method: 'DELETE' }),

	addMember: (id: string, body: { user_id: string }, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/channels/${id}/members`, {
			...opts,
			method: 'POST',
			body
		}),

	removeMember: (channelId: string, userId: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/channels/${channelId}/members/${userId}`, {
			...opts,
			method: 'DELETE'
		}),

	markRead: (id: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/channels/${id}/read`, { ...opts, method: 'POST' })
};

export const messages = {
	list: (channelId: string, query?: { before?: string; limit?: number }, opts?: RequestOptions) => {
		const params = new URLSearchParams();
		if (query?.before) params.set('before', query.before);
		if (query?.limit !== undefined) params.set('limit', String(query.limit));
		const qs = params.toString();
		const path = `/api/v1/channels/${channelId}/messages${qs ? `?${qs}` : ''}`;
		return authStore.fetch<MessagePage>(path, opts);
	},

	post: (
		channelId: string,
		body: { content: string; thread_parent_id?: string },
		opts?: RequestOptions
	) =>
		authStore.fetch<ChatMessage>(`/api/v1/channels/${channelId}/messages`, {
			...opts,
			method: 'POST',
			body
		}),

	thread: (id: string, opts?: RequestOptions) =>
		authStore.fetch<ThreadView>(`/api/v1/messages/${id}/thread`, opts),

	edit: (id: string, body: { content: string }, opts?: RequestOptions) =>
		authStore.fetch<ChatMessage>(`/api/v1/messages/${id}`, { ...opts, method: 'PATCH', body }),

	delete: (id: string, opts?: RequestOptions) =>
		authStore.fetch<void>(`/api/v1/messages/${id}`, { ...opts, method: 'DELETE' })
};
