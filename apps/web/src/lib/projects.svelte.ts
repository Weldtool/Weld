import { browser } from '$app/environment';
import { projects as projectsApi, teams as teamsApi } from '$lib/api';
import type { Project, UserTeam } from '$lib/api/types';

const SELECTED_PROJECT_KEY = 'weld:selected-project';

class ProjectsStore {
	team = $state<UserTeam | null>(null);
	projects = $state<Project[]>([]);
	selectedId = $state<string | null>(null);
	loading = $state(false);
	error = $state<string | null>(null);
	#loaded = false;

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
			const teams = await teamsApi.list();
			this.team = teams[0] ?? null;
			this.projects = this.team ? await projectsApi.listForTeam(this.team.id) : [];
			this.#restoreSelection();
			this.#loaded = true;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load projects.';
		} finally {
			this.loading = false;
		}
	}

	async ensureTeam(name = 'My team'): Promise<UserTeam | null> {
		if (this.team) return this.team;
		await teamsApi.create({ name });
		await this.refresh();
		return this.team;
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
		patch: { name?: string; description?: string | null; github_repo?: string | null }
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
		this.team = null;
		this.projects = [];
		this.selectedId = null;
		this.error = null;
		this.#loaded = false;
		if (browser) localStorage.removeItem(SELECTED_PROJECT_KEY);
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
