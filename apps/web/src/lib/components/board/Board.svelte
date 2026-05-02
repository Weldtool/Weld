<script lang="ts">
	import { boardStore } from '$lib/board.svelte';
	import Column from './Column.svelte';
	import BoardToolbar from './BoardToolbar.svelte';

	const labelMap = $derived(new Map(boardStore.labels.map((l) => [l.id, l])));
</script>

<div class="flex min-h-0 flex-1 flex-col gap-4">
	<BoardToolbar />

	{#if boardStore.error}
		<p
			role="alert"
			class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
		>
			{boardStore.error}
		</p>
	{/if}

	{#if boardStore.loading && boardStore.columns.length === 0}
		<p class="text-sm text-[var(--muted-foreground)]">Loading board…</p>
	{:else if boardStore.columns.length === 0}
		<div
			class="rounded-lg border border-dashed border-[var(--border)] p-8 text-center text-sm text-[var(--muted-foreground)]"
		>
			No columns yet. Use “New column” above to create your first one.
		</div>
	{:else}
		<div class="flex min-h-0 w-full flex-1 items-stretch gap-3 pb-3">
			{#each boardStore.columns as column (column.id)}
				<Column {column} tasks={boardStore.tasksForColumn(column.id)} {labelMap} />
			{/each}
		</div>
	{/if}
</div>
