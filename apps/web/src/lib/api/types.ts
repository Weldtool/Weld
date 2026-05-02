/** Types mirroring weld-server response shapes. Keep in sync with the Rust models. */

export type GlobalRole = 'admin' | 'member';
export type TeamRole = 'owner' | 'admin' | 'member' | 'viewer';

export interface User {
	id: string;
	email: string;
	display_name: string;
	avatar_url: string | null;
	role: GlobalRole;
	email_verified: boolean;
	created_at: string;
	updated_at: string;
}

export interface Team {
	id: string;
	name: string;
	slug: string;
	owner_id: string;
	created_at: string;
}

export interface UserTeam extends Team {
	role: TeamRole;
}

export interface TeamMember {
	user_id: string;
	email: string;
	display_name: string;
	avatar_url: string | null;
	role: TeamRole;
	joined_at: string;
}

export interface Project {
	id: string;
	team_id: string;
	name: string;
	slug: string;
	description: string | null;
	github_repo: string | null;
	created_at: string;
	updated_at: string;
}

export interface LoginResponse {
	access_token: string;
	token_type: 'Bearer';
}

export type Priority = 'critical' | 'high' | 'medium' | 'low' | 'none';

export interface Column {
	id: string;
	project_id: string;
	name: string;
	position: number;
	wip_limit: number | null;
	created_at: string;
	updated_at: string;
}

export interface Label {
	id: string;
	project_id: string;
	name: string;
	color: string;
	created_at: string;
}

export interface Task {
	id: string;
	project_id: string;
	column_id: string;
	title: string;
	description: string | null;
	assignee_id: string | null;
	reporter_id: string;
	priority: Priority;
	position: number;
	due_date: string | null;
	estimate_hours: number | null;
	github_issue_id: number | null;
	github_pr_id: number | null;
	labels: string[];
	created_at: string;
	updated_at: string;
}

export interface BoardState {
	columns: Column[];
	tasks: Task[];
	labels: Label[];
}
