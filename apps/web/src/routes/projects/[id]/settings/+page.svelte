<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { ApiError } from '$lib/api/client';
	import { projects } from '$lib/projects.svelte';
	import { ArrowLeft } from 'lucide-svelte';

	const projectId = $derived(page.params.id);
	const project = $derived(projects.projects.find((p) => p.id === projectId) ?? null);

	let name = $state('');
	let description = $state('');
	let githubRepo = $state('');
	let saving = $state(false);
	let saveError = $state<string | null>(null);
	let saveSuccess = $state(false);

	let confirming = $state(false);
	let confirmSlug = $state('');
	let deleting = $state(false);
	let deleteError = $state<string | null>(null);

	let lastSyncedId = $state<string | null>(null);
	$effect(() => {
		if (!project) return;
		if (lastSyncedId === project.id) return;
		name = project.name;
		description = project.description ?? '';
		githubRepo = project.github_repo ?? '';
		lastSyncedId = project.id;
		saveSuccess = false;
		saveError = null;
	});

	async function onSave(event: SubmitEvent) {
		event.preventDefault();
		if (!project) return;
		saveError = null;
		saveSuccess = false;
		if (!name.trim()) {
			saveError = 'Project name is required.';
			return;
		}
		saving = true;
		try {
			await projects.update(project.id, {
				name: name.trim(),
				description: description.trim() ? description.trim() : null,
				github_repo: githubRepo.trim() ? githubRepo.trim() : null
			});
			saveSuccess = true;
		} catch (err) {
			saveError =
				err instanceof ApiError ? err.message : 'Failed to save changes. Please try again.';
		} finally {
			saving = false;
		}
	}

	async function onDelete() {
		if (!project) return;
		deleteError = null;
		deleting = true;
		try {
			await projects.remove(project.id);
			await goto(resolve('/projects'));
		} catch (err) {
			deleteError =
				err instanceof ApiError
					? err.status === 403
						? 'Only the team owner can delete this project.'
						: err.message
					: 'Failed to delete the project. Please try again.';
			confirming = false;
		} finally {
			deleting = false;
		}
	}
</script>

<div class="mx-auto flex w-full max-w-2xl flex-col gap-6">
	<a
		href={resolve('/projects')}
		class="inline-flex items-center gap-1 text-sm text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
	>
		<ArrowLeft class="h-4 w-4" />
		Back to projects
	</a>

	{#if !projects.isReady}
		<p class="text-sm text-[var(--muted-foreground)]">Loading project…</p>
	{:else if !project}
		<div
			class="rounded-lg border border-[var(--border)] bg-[var(--card)] p-6 text-sm text-[var(--muted-foreground)]"
		>
			Project not found. It may have been deleted.
		</div>
	{:else}
		<header>
			<h1 class="text-2xl font-semibold tracking-tight">{project.name}</h1>
			<p class="text-sm text-[var(--muted-foreground)]">Project settings · {project.slug}</p>
		</header>

		<form
			class="flex flex-col gap-4 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6"
			onsubmit={onSave}
			novalidate
		>
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
				<span class="font-medium">Description</span>
				<textarea
					rows={4}
					bind:value={description}
					class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				></textarea>
			</label>

			<label class="flex flex-col gap-1.5 text-sm">
				<span class="font-medium">GitHub repo</span>
				<input
					type="text"
					placeholder="owner/repo"
					bind:value={githubRepo}
					class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
			</label>

			{#if saveError}
				<p
					role="alert"
					class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
				>
					{saveError}
				</p>
			{/if}
			{#if saveSuccess}
				<p
					class="rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-600 dark:text-emerald-400"
				>
					Saved.
				</p>
			{/if}

			<div class="flex justify-end">
				<button
					type="submit"
					disabled={saving}
					class="rounded-md bg-[var(--primary)] px-4 py-2 text-sm font-medium text-[var(--primary-foreground)] transition-opacity hover:opacity-90 disabled:opacity-60"
				>
					{saving ? 'Saving…' : 'Save changes'}
				</button>
			</div>
		</form>

		<div class="flex flex-col gap-3 rounded-lg border border-red-500/30 bg-red-500/5 p-6">
			<div>
				<h2 class="text-base font-semibold text-red-600 dark:text-red-400">Danger zone</h2>
				<p class="text-sm text-[var(--muted-foreground)]">
					Deleting a project hides it from the team. This cannot be undone from the UI.
				</p>
			</div>

			{#if deleteError}
				<p
					role="alert"
					class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
				>
					{deleteError}
				</p>
			{/if}

			{#if !confirming}
				<button
					type="button"
					onclick={() => (confirming = true)}
					class="self-start rounded-md border border-red-500/40 px-3 py-2 text-sm font-medium text-red-600 hover:bg-red-500/10 dark:text-red-400"
				>
					Delete project
				</button>
			{:else}
				<div class="flex flex-col gap-2 rounded-md border border-red-500/40 bg-red-500/10 p-3">
					<p class="text-sm text-[var(--foreground)]">
						Type <span class="font-mono font-semibold">{project.slug}</span> to confirm deletion.
					</p>
					<input
						type="text"
						bind:value={confirmSlug}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					/>
					<div class="flex justify-end gap-2">
						<button
							type="button"
							onclick={() => {
								confirming = false;
								confirmSlug = '';
							}}
							disabled={deleting}
							class="rounded-md px-3 py-2 text-sm font-medium text-[var(--muted-foreground)] hover:text-[var(--foreground)] disabled:opacity-60"
						>
							Cancel
						</button>
						<button
							type="button"
							onclick={onDelete}
							disabled={deleting || confirmSlug !== project.slug}
							class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-60"
						>
							{deleting ? 'Deleting…' : 'Delete project'}
						</button>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
