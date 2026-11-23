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
	is_personal: boolean;
}

export interface TeamMember {
	user_id: string;
	email: string;
	display_name: string;
	avatar_url: string | null;
	role: TeamRole;
	joined_at: string;
}

export interface ProjectMember {
	user_id: string;
	email: string;
	display_name: string;
	avatar_url: string | null;
	team_role: TeamRole;
	added_at: string;
}

export interface Project {
	id: string;
	team_id: string;
	name: string;
	slug: string;
	description: string | null;
	github_repo: string | null;
	done_column_id: string | null;
	in_progress_column_id: string | null;
	review_column_id: string | null;
	auto_close_on_issue_closed: boolean;
	/** Whether Weld comments on a linked GitHub issue when its card is picked up. */
	github_comment_on_move: boolean;
	created_at: string;
	updated_at: string;
}

export interface GithubConnection {
	id: string;
	team_id: string;
	installation_id: number;
	account_login: string;
	created_by: string;
	created_at: string;
	updated_at: string;
}

export interface GitHubIssue {
	number: number;
	title: string;
	html_url: string;
	state: string;
	user: { login: string; avatar_url: string };
	labels: { name: string; color: string }[];
}

export interface GitHubIssueDetail extends GitHubIssue {
	body: string | null;
}

export interface GitHubPRFile {
	filename: string;
	status: string;
	additions: number;
	deletions: number;
	/** Absent for binary files and for diffs GitHub judges too large to inline. */
	patch?: string;
}

export type ReviewVerdict = 'approve' | 'comment' | 'request_changes';

export type MergeMethod = 'merge' | 'squash' | 'rebase';

export interface GitHubPR {
	number: number;
	title: string;
	html_url: string;
	state: string;
	draft: boolean;
	user: { login: string; avatar_url: string };
	// Present in study mode (seeded fixtures); the live GitHub list endpoint omits
	// these heavier fields, so treat them as optional.
	body?: string;
	labels?: { name: string; color: string }[];
	additions?: number;
	deletions?: number;
	changed_files?: number;
	/** Only the single-PR endpoint returns files; the list endpoint omits them. */
	files?: GitHubPRFile[];
	/** GitHub's mergeability check; null means it is still computing. */
	mergeable?: boolean | null;
	merged?: boolean;
}

export interface RepoBranches {
	default_branch: string;
	branches: string[];
}

export interface IssueTemplate {
	name: string;
	about: string | null;
	body: string;
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
	short_id: number;
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
	github_repo: string | null;
	/** Latest CI conclusion for the linked PR, in GitHub's vocabulary
	 *  ('success', 'failure', 'pending', …). Null when no result is known. */
	github_check_status: string | null;
	labels: string[];
	created_at: string;
	updated_at: string;
}

export interface BoardState {
	columns: Column[];
	tasks: Task[];
	labels: Label[];
}

export interface GitHubEventFilter {
	pr: boolean;
	issue: boolean;
	review: boolean;
	push: boolean;
}

export interface Channel {
	id: string;
	team_id: string;
	project_id: string | null;
	name: string;
	topic: string | null;
	is_direct: boolean;
	is_private: boolean;
	created_by: string;
	created_at: string;
	updated_at: string;
	github_repos: string[];
	github_event_filter: GitHubEventFilter;
}

export type GitHubMessageKind =
	| 'pr.opened'
	| 'pr.merged'
	| 'pr.closed'
	| 'pr.reopened'
	| 'issue.opened'
	| 'issue.closed'
	| 'issue.reopened'
	| 'review.submitted';

export interface GitHubMessageMetadata {
	kind: GitHubMessageKind;
	repo: string;
	number: number;
	title?: string;
	url: string;
	author: string;
	author_avatar: string;
	additions?: number;
	deletions?: number;
	labels?: string[];
	verdict?: 'approved' | 'changes_requested' | 'commented';
}

export interface ChatMessage {
	id: string;
	channel_id: string;
	sender_id: string;
	content: string;
	thread_parent_id: string | null;
	mentions: string[];
	task_refs: string[];
	metadata: Record<string, unknown>;
	edited_at: string | null;
	created_at: string;
	/** Thread reply count. Only present on messages from the channel list
	 * endpoint; realtime events and single-message responses omit it. */
	reply_count?: number;
}

export interface TaskRef {
	id: string;
	short_id: number;
	title: string;
	project_id: string;
	/** Number of the GitHub PR linked to this task, if any. Used to resolve
	 * `PR#<number>` chat references to the task they're linked to. */
	github_pr_id: number | null;
}

export interface MessagePage {
	messages: ChatMessage[];
	next_before: string | null;
}

export interface ThreadView {
	parent: ChatMessage;
	replies: ChatMessage[];
}

/** Public, unauthenticated runtime config served from `GET /api/v1/config`. */
export interface PublicConfig {
	study_mode: boolean;
	/** When true, the study intro requires a single-use access code to begin. */
	study_require_access_code: boolean;
}
