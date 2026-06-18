<script lang="ts">
	import { resolve } from '$app/paths';
	import { ApiError } from '$lib/api/client';
	import { projects } from '$lib/projects.svelte';
	import { Plus } from '@lucide/svelte';

	let creating = $state(false);
	let submitting = $state(false);
	let formError = $state<string | null>(null);
	let name = $state('');
	let description = $state('');
	let githubRepo = $state('');

	function openCreate() {
		creating = true;
		formError = null;
		name = '';
		description = '';
		githubRepo = '';
	}

	function closeCreate() {
		if (submitting) return;
		creating = false;
	}

	async function onSubmit(event: SubmitEvent) {
		event.preventDefault();
		formError = null;
		if (!name.trim()) {
			formError = 'Project name is required.';
			return;
		}

		submitting = true;
		try {
			await projects.create({
				name: name.trim(),
				description,
				github_repo: githubRepo
			});
			creating = false;
		} catch (err) {
			formError =
				err instanceof ApiError
					? err.status === 409
						? 'A project with that slug already exists.'
						: err.message
					: 'Failed to create project. Please try again.';
		} finally {
			submitting = false;
		}
	}
</script>

<div class="flex flex-col gap-6">
	<header class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">Projects</h1>
			<p class="text-sm text-[var(--muted-foreground)]">
				{#if projects.team}
					Workspace: <span class="font-medium text-[var(--foreground)]">{projects.team.name}</span>
				{:else}
					Create your first project to get started.
				{/if}
			</p>
		</div>
		<button
			type="button"
			onclick={openCreate}
			class="inline-flex items-center gap-2 rounded-md bg-[var(--primary)] px-3 py-2 text-sm font-medium text-[var(--primary-foreground)] transition-opacity hover:opacity-90"
		>
			<Plus class="h-4 w-4" />
			New project
		</button>
	</header>

	{#if projects.error}
		<p
			role="alert"
			class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
		>
			{projects.error}
		</p>
	{/if}

	{#if projects.loading && projects.projects.length === 0}
		<p class="text-sm text-[var(--muted-foreground)]">Loading projects…</p>
	{:else if projects.projects.length === 0}
		<div
			class="rounded-lg border border-dashed border-[var(--border)] p-8 text-center text-sm text-[var(--muted-foreground)]"
		>
			No projects yet. Click <span class="font-medium text-[var(--foreground)]">New project</span> to
			create one.
		</div>
	{:else}
		<ul class="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
			{#each projects.projects as project (project.id)}
				<li
					class="relative flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--card)] p-4 transition-colors focus-within:border-[var(--ring)] hover:border-[var(--ring)]"
				>
					<a
						href={resolve(`/projects/${project.id}/settings`)}
						aria-label={`Open ${project.name} settings`}
						class="absolute inset-0 rounded-lg focus-visible:outline-2 focus-visible:outline-[var(--ring)]"
					></a>
					<div class="flex items-start justify-between gap-2">
						<h2 class="text-base font-semibold tracking-tight">{project.name}</h2>
						{#if projects.selectedId === project.id}
							<span
								class="rounded-full bg-[var(--accent)] px-2 py-0.5 text-xs text-[var(--accent-foreground)]"
							>
								Active
							</span>
						{/if}
					</div>
					<p class="line-clamp-2 text-sm text-[var(--muted-foreground)]">
						{project.description ?? 'No description.'}
					</p>
					<div class="mt-auto flex items-center justify-between text-xs">
						<span class="text-[var(--muted-foreground)]">{project.slug}</span>
						<button
							type="button"
							onclick={(e) => {
								e.stopPropagation();
								projects.select(project.id);
							}}
							disabled={projects.selectedId === project.id}
							class="relative z-10 font-medium text-[var(--foreground)] hover:underline disabled:opacity-50"
						>
							{projects.selectedId === project.id ? 'Selected' : 'Select'}
						</button>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

{#if creating}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
		role="dialog"
		aria-modal="true"
		aria-labelledby="create-project-title"
	>
		<div class="w-full max-w-md rounded-lg border border-[var(--border)] bg-[var(--card)] p-6">
			<h2 id="create-project-title" class="text-lg font-semibold tracking-tight">Create project</h2>
			<form class="mt-4 flex flex-col gap-4" onsubmit={onSubmit} novalidate>
				<label class="flex flex-col gap-1.5 text-sm">
					<span class="font-medium">Name</span>
					<input
						type="text"
						required
						bind:value={name}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					/>
				</label>

				<label class="flex flex-col gap-1.5 text-sm">
					<span class="font-medium">Description (optional)</span>
					<textarea
						rows={3}
						bind:value={description}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					></textarea>
				</label>

				<label class="flex flex-col gap-1.5 text-sm">
					<span class="font-medium">GitHub repo (optional)</span>
					<input
						type="text"
						placeholder="owner/repo"
						bind:value={githubRepo}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					/>
				</label>

				{#if formError}
					<p
						role="alert"
						class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
					>
						{formError}
					</p>
				{/if}

				<div class="mt-2 flex justify-end gap-2">
					<button
						type="button"
						onclick={closeCreate}
						disabled={submitting}
						class="rounded-md px-3 py-2 text-sm font-medium text-[var(--muted-foreground)] hover:text-[var(--foreground)] disabled:opacity-60"
					>
						Cancel
					</button>
					<button
						type="submit"
						disabled={submitting}
						class="rounded-md bg-[var(--primary)] px-3 py-2 text-sm font-medium text-[var(--primary-foreground)] transition-opacity hover:opacity-90 disabled:opacity-60"
					>
						{submitting ? 'Creating…' : 'Create project'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}
