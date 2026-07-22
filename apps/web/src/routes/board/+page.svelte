<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { GitPullRequest } from '@lucide/svelte';
	import Board from '$lib/components/board/Board.svelte';
	import TaskDraft from '$lib/components/board/TaskDraft.svelte';
	import CreatePrDialog from '$lib/components/board/CreatePrDialog.svelte';
	import GitHubPanel from '$lib/components/board/GitHubPanel.svelte';
	import { boardStore } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';

	// A `?project=` param (e.g. from a chat task link) selects that project so
	// the linked task's board loads even if another project was active.
	$effect(() => {
		const wanted = page.url.searchParams.get('project');
		if (wanted && wanted !== projects.selectedId) {
			projects.select(wanted);
		}
	});

	$effect(() => {
		const projectId = projects.selectedId;
		if (projectId && boardStore.projectId !== projectId) {
			boardStore.loadBoard(projectId);
		} else if (!projectId) {
			boardStore.reset();
		}
	});

	const taskId = $derived(page.url.searchParams.get('task'));
	const draftColumnId = $derived(page.url.searchParams.get('column'));

	let showGitHub = $state(false);
</script>

<header class="mb-6 flex items-end justify-between gap-3">
	<div>
		<h1 class="text-2xl font-semibold tracking-tight">Board</h1>
		<p class="text-sm text-[var(--muted-foreground)]">
			{#if projects.current}
				{projects.current.name}
			{:else}
				Select a project to view its board.
			{/if}
		</p>
	</div>
	{#if projects.current?.github_repo}
		<button
			type="button"
			onclick={() => (showGitHub = !showGitHub)}
			class="flex items-center gap-1.5 rounded-md border border-[var(--border)] bg-[var(--card)] px-3 py-1.5 text-xs text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)] {showGitHub
				? 'bg-[var(--accent)] text-[var(--foreground)]'
				: ''}"
		>
			<GitPullRequest class="h-3.5 w-3.5" />
			GitHub
		</button>
	{/if}
</header>

{#if !projects.current}
	<div
		class="rounded-lg border border-dashed border-[var(--border)] bg-[var(--card)] p-10 text-center text-sm text-[var(--muted-foreground)]"
	>
		No active project.
		<a class="ml-1 underline" href={resolve('/projects')}>Pick one</a> to load its board.
	</div>
{:else}
	<div class="flex min-h-0 flex-1 gap-4 overflow-hidden">
		<div class="flex min-w-0 flex-1 flex-col overflow-hidden">
			<Board />
		</div>
		{#if showGitHub && projects.current}
			<aside
				class="w-72 shrink-0 overflow-y-auto rounded-lg border border-[var(--border)] bg-[var(--card)] p-4"
			>
				<GitHubPanel project={projects.current} />
			</aside>
		{/if}
	</div>
{/if}

{#if taskId === 'new' && draftColumnId}
	{#key draftColumnId}
		<TaskDraft columnId={draftColumnId} />
	{/key}
{/if}
<!-- Existing-task modal is rendered globally by TaskDetailOverlay in the root
     layout, so a `?task=` deep link opens in place on any route. -->

<CreatePrDialog />
