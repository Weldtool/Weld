<script lang="ts">
	import { page } from '$app/state';
	import { boardStore } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';
	import TaskDetail from './TaskDetail.svelte';

	// Renders the task modal from a `?task=` deep link on ANY route (board, chat,
	// …) so a chat card/task reference pops the modal in place instead of yanking
	// the participant over to the board. `task=new` (the create-a-card draft) is
	// board-only and handled there.
	const taskId = $derived(page.url.searchParams.get('task'));
	const projectId = $derived(page.url.searchParams.get('project'));

	// A card ref carries `&project=`; make sure that project is selected and its
	// board loaded so `boardStore.taskById` resolves off the board route.
	$effect(() => {
		if (!taskId || taskId === 'new') return;
		const wanted = projectId ?? projects.selectedId;
		if (!wanted) return;
		if (projects.selectedId !== wanted) projects.select(wanted);
		if (boardStore.projectId !== wanted) void boardStore.loadBoard(wanted);
	});
</script>

{#if taskId && taskId !== 'new'}
	{#key taskId}
		<TaskDetail {taskId} />
	{/key}
{/if}
