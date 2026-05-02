<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import Board from '$lib/components/board/Board.svelte';
	import TaskDetail from '$lib/components/board/TaskDetail.svelte';
	import { boardStore } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';

	$effect(() => {
		const projectId = projects.selectedId;
		if (projectId && boardStore.projectId !== projectId) {
			boardStore.loadBoard(projectId);
		} else if (!projectId) {
			boardStore.reset();
		}
	});

	const taskId = $derived(page.url.searchParams.get('task'));
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
</header>

{#if !projects.current}
	<div
		class="rounded-lg border border-dashed border-[var(--border)] bg-[var(--card)] p-10 text-center text-sm text-[var(--muted-foreground)]"
	>
		No active project.
		<a class="ml-1 underline" href={resolve('/projects')}>Pick one</a> to load its board.
	</div>
{:else}
	<div class="flex flex-1 flex-col">
		<Board />
	</div>
{/if}

{#if taskId}
	{#key taskId}
		<TaskDetail {taskId} />
	{/key}
{/if}
