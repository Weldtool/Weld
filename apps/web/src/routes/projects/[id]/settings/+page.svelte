<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { ApiError } from '$lib/api/client';
	import { board as boardApi, projects as projectsApi } from '$lib/api';
	import type { Column, ProjectMember } from '$lib/api/types';
	import { projects } from '$lib/projects.svelte';
	import { ArrowLeft, X } from 'lucide-svelte';
	import { onMount } from 'svelte';

	const projectId = $derived(page.params.id);
	const project = $derived(projects.projects.find((p) => p.id === projectId) ?? null);

	let name = $state('');
	let description = $state('');
	let githubRepo = $state('');
	let doneColumnId = $state<string>('');
	let inProgressColumnId = $state<string>('');
	let reviewColumnId = $state<string>('');
	let autoClose = $state(false);
	let saving = $state(false);
	let saveError = $state<string | null>(null);
	let saveSuccess = $state(false);

	let columns = $state<Column[]>([]);

	let confirming = $state(false);
	let confirmSlug = $state('');
	let deleting = $state(false);
	let deleteError = $state<string | null>(null);

	let members = $state<ProjectMember[]>([]);
	let membersLoading = $state(false);
	let membersError = $state<string | null>(null);
	let inviteEmail = $state('');
	let inviting = $state(false);
	let inviteError = $state<string | null>(null);

	async function loadMembers() {
		if (!projectId) return;
		membersLoading = true;
		membersError = null;
		try {
			members = await projectsApi.listMembers(projectId);
		} catch (err) {
			membersError = err instanceof ApiError ? err.message : 'Failed to load members.';
		} finally {
			membersLoading = false;
		}
	}

	async function onInvite(event: SubmitEvent) {
		event.preventDefault();
		const email = inviteEmail.trim();
		if (!email || !projectId) return;
		inviteError = null;
		inviting = true;
		try {
			const member = await projectsApi.addMember(projectId, { email });
			members = [...members.filter((m) => m.user_id !== member.user_id), member];
			inviteEmail = '';
		} catch (err) {
			inviteError =
				err instanceof ApiError
					? err.status === 404
						? 'No registered user with that email.'
						: err.message
					: 'Failed to add member. Please try again.';
		} finally {
			inviting = false;
		}
	}

	async function onRemoveMember(userId: string) {
		if (!projectId) return;
		const previous = members;
		members = members.filter((m) => m.user_id !== userId);
		try {
			await projectsApi.removeMember(projectId, userId);
		} catch (err) {
			members = previous;
			membersError = err instanceof ApiError ? err.message : 'Failed to remove member.';
		}
	}

	let lastSyncedId = $state<string | null>(null);
	$effect(() => {
		if (!project) return;
		if (lastSyncedId === project.id) return;
		name = project.name;
		description = project.description ?? '';
		githubRepo = project.github_repo ?? '';
		doneColumnId = project.done_column_id ?? '';
		inProgressColumnId = project.in_progress_column_id ?? '';
		reviewColumnId = project.review_column_id ?? '';
		autoClose = project.auto_close_on_issue_closed;
		lastSyncedId = project.id;
		saveSuccess = false;
		saveError = null;
	});

	onMount(async () => {
		if (!projectId) return;
		try {
			const state = await boardApi.get(projectId);
			columns = state.columns.slice().sort((a, b) => a.position - b.position);
		} catch {
			// columns stays empty — dropdown just won't show options
		}
		await loadMembers();
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
				github_repo: githubRepo.trim() ? githubRepo.trim() : null,
				done_column_id: doneColumnId || null,
				in_progress_column_id: inProgressColumnId || null,
				review_column_id: reviewColumnId || null,
				auto_close_on_issue_closed: autoClose
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

			<fieldset class="flex flex-col gap-3 rounded-md border border-[var(--border)] p-4">
				<legend class="px-1 text-sm font-medium">GitHub board automation</legend>

				<label class="flex flex-col gap-1.5 text-sm">
					<span class="font-medium">"In Progress" column</span>
					<select
						bind:value={inProgressColumnId}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					>
						<option value="">None</option>
						{#each columns as col (col.id)}
							<option value={col.id}>{col.name}</option>
						{/each}
					</select>
					<p class="text-[10px] text-[var(--muted-foreground)]">
						A linked PR closed without merging sends the task back here.
					</p>
				</label>

				<label class="flex flex-col gap-1.5 text-sm">
					<span class="font-medium">"Review" column</span>
					<select
						bind:value={reviewColumnId}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					>
						<option value="">None</option>
						{#each columns as col (col.id)}
							<option value={col.id}>{col.name}</option>
						{/each}
					</select>
					<p class="text-[10px] text-[var(--muted-foreground)]">
						Dragging a task into this column prompts you to open a pull request.
					</p>
				</label>

				<label class="flex flex-col gap-1.5 text-sm">
					<span class="font-medium">"Done" column</span>
					<select
						bind:value={doneColumnId}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					>
						<option value="">None</option>
						{#each columns as col (col.id)}
							<option value={col.id}>{col.name}</option>
						{/each}
					</select>
					<p class="text-[10px] text-[var(--muted-foreground)]">
						A merged PR (or closed linked issue) auto-moves the task here.
					</p>
				</label>

				<label class="flex cursor-pointer items-center gap-2 text-sm">
					<input type="checkbox" bind:checked={autoClose} class="rounded" />
					<span>Automatically move tasks based on GitHub activity</span>
				</label>
			</fieldset>

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

		<section
			class="flex flex-col gap-4 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6"
		>
			<div>
				<h2 class="text-base font-semibold">Members</h2>
				<p class="text-sm text-[var(--muted-foreground)]">
					Team owners and admins always have access. Add members and viewers here to give them
					access to this project.
				</p>
			</div>

			<form class="flex items-start gap-2" onsubmit={onInvite} novalidate>
				<div class="flex flex-1 flex-col gap-1">
					<input
						type="email"
						placeholder="teammate@example.com"
						bind:value={inviteEmail}
						class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					/>
					{#if inviteError}
						<p role="alert" class="text-xs text-red-600 dark:text-red-400">{inviteError}</p>
					{/if}
				</div>
				<button
					type="submit"
					disabled={inviting || !inviteEmail.trim()}
					class="rounded-md bg-[var(--primary)] px-4 py-2 text-sm font-medium text-[var(--primary-foreground)] transition-opacity hover:opacity-90 disabled:opacity-60"
				>
					{inviting ? 'Adding…' : 'Add'}
				</button>
			</form>

			{#if membersError}
				<p
					role="alert"
					class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
				>
					{membersError}
				</p>
			{/if}

			{#if membersLoading}
				<p class="text-sm text-[var(--muted-foreground)]">Loading members…</p>
			{:else if members.length === 0}
				<p class="text-sm text-[var(--muted-foreground)]">
					No members added yet. Team admins and owners still have access.
				</p>
			{:else}
				<ul class="flex flex-col divide-y divide-[var(--border)]">
					{#each members as member (member.user_id)}
						<li class="flex items-center justify-between gap-3 py-2">
							<div class="min-w-0">
								<div class="truncate text-sm font-medium">{member.display_name}</div>
								<div class="truncate text-xs text-[var(--muted-foreground)]">
									{member.email} · {member.team_role}
								</div>
							</div>
							<button
								type="button"
								onclick={() => onRemoveMember(member.user_id)}
								class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-[var(--muted-foreground)] hover:bg-red-500/10 hover:text-red-600 dark:hover:text-red-400"
								aria-label="Remove {member.display_name}"
								title="Remove from project"
							>
								<X class="h-4 w-4" />
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</section>

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
