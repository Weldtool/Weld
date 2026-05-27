import { browser } from '$app/environment';
import { projects as projectsApi, teams as teamsApi } from '$lib/api';
import type { Project, UserTeam } from '$lib/api/types';

const SELECTED_PROJECT_KEY = 'weld:selected-project';
const SELECTED_TEAM_KEY = 'weld:selected-team';

class ProjectsStore {
	teams = $state<UserTeam[]>([]);
	selectedTeamId = $state<string | null>(null);
	projects = $state<Project[]>([]);
	selectedId = $state<string | null>(null);
	loading = $state(false);
	error = $state<string | null>(null);
	#loaded = false;

	get team(): UserTeam | null {
		if (!this.selectedTeamId) return null;
		return this.teams.find((t) => t.id === this.selectedTeamId) ?? null;
	}

	get current(): Project | null {
		if (!this.selectedId) return null;
		return this.projects.find((p) => p.id === this.selectedId) ?? null;
	}

	get isReady(): boolean {
		return this.#loaded;
	}

	async ensureLoaded(): Promise<void> {
		if (this.#loaded) return;
		await this.refresh();
	}

	async refresh(): Promise<void> {
		this.loading = true;
		this.error = null;
		try {
			this.teams = await teamsApi.list();
			this.#restoreTeamSelection();
			await this.#loadProjects();
			this.#loaded = true;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load projects.';
		} finally {
			this.loading = false;
		}
	}

	/** Switch the active team, reloading its visible projects. */
	async selectTeam(teamId: string): Promise<void> {
		if (teamId === this.selectedTeamId) return;
		this.selectedTeamId = teamId;
		if (browser) localStorage.setItem(SELECTED_TEAM_KEY, teamId);
		this.loading = true;
		this.error = null;
		try {
			await this.#loadProjects();
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load projects.';
		} finally {
			this.loading = false;
		}
	}

	async #loadProjects(): Promise<void> {
		const team = this.team;
		this.projects = team ? await projectsApi.listForTeam(team.id) : [];
		this.#restoreSelection();
	}

	/** Ensure a (selected) team exists, creating a shared team if the user has none. */
	async ensureTeam(name = 'My team'): Promise<UserTeam | null> {
		if (this.team) return this.team;
		await teamsApi.create({ name });
		await this.refresh();
		return this.team;
	}

	/** Create a new shared team and switch to it. */
	async createTeam(name: string): Promise<UserTeam> {
		const created = await teamsApi.create({ name });
		await this.refresh();
		await this.selectTeam(created.id);
		const team = this.teams.find((t) => t.id === created.id);
		if (!team) throw new Error('Team creation succeeded but the team did not load.');
		return team;
	}

	async create(input: {
		name: string;
		description?: string;
		github_repo?: string;
	}): Promise<Project> {
		const team = await this.ensureTeam();
		if (!team) throw new Error('Could not create or load a team for this user.');
		const body: { name: string; description?: string; github_repo?: string } = { name: input.name };
		if (input.description?.trim()) body.description = input.description.trim();
		if (input.github_repo?.trim()) body.github_repo = input.github_repo.trim();
		const created = await projectsApi.create(team.id, body);
		this.projects = [...this.projects, created];
		this.select(created.id);
		return created;
	}

	async update(
		id: string,
		patch: {
			name?: string;
			description?: string | null;
			github_repo?: string | null;
			done_column_id?: string | null;
			in_progress_column_id?: string | null;
			review_column_id?: string | null;
			auto_close_on_issue_closed?: boolean;
		}
	): Promise<Project> {
		const updated = await projectsApi.update(id, patch);
		this.projects = this.projects.map((p) => (p.id === id ? updated : p));
		return updated;
	}

	async remove(id: string): Promise<void> {
		const previous = this.projects;
		this.projects = previous.filter((p) => p.id !== id);
		if (this.selectedId === id) this.select(this.projects[0]?.id ?? null);
		try {
			await projectsApi.delete(id);
		} catch (err) {
			this.projects = previous;
			throw err;
		}
	}

	select(id: string | null): void {
		this.selectedId = id;
		if (!browser) return;
		if (id) localStorage.setItem(SELECTED_PROJECT_KEY, id);
		else localStorage.removeItem(SELECTED_PROJECT_KEY);
	}

	reset(): void {
		this.teams = [];
		this.selectedTeamId = null;
		this.projects = [];
		this.selectedId = null;
		this.error = null;
		this.#loaded = false;
		if (browser) {
			localStorage.removeItem(SELECTED_PROJECT_KEY);
			localStorage.removeItem(SELECTED_TEAM_KEY);
		}
	}

	#restoreTeamSelection(): void {
		const stored = browser ? localStorage.getItem(SELECTED_TEAM_KEY) : null;
		const found = stored && this.teams.some((t) => t.id === stored) ? stored : null;
		// Default to the first team — the API sorts personal teams first.
		this.selectedTeamId = found ?? this.teams[0]?.id ?? null;
		if (browser) {
			if (this.selectedTeamId) localStorage.setItem(SELECTED_TEAM_KEY, this.selectedTeamId);
			else localStorage.removeItem(SELECTED_TEAM_KEY);
		}
	}

	#restoreSelection(): void {
		const stored = browser ? localStorage.getItem(SELECTED_PROJECT_KEY) : null;
		const found = stored && this.projects.some((p) => p.id === stored) ? stored : null;
		this.selectedId = found ?? this.projects[0]?.id ?? null;
		if (browser) {
			if (this.selectedId) localStorage.setItem(SELECTED_PROJECT_KEY, this.selectedId);
			else localStorage.removeItem(SELECTED_PROJECT_KEY);
		}
	}
}

export const projects = new ProjectsStore();
