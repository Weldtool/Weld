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
