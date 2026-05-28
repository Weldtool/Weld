<script lang="ts">
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import { flip } from 'svelte/animate';
	import { boardStore, computePosition } from '$lib/board.svelte';
	import type { Column } from '$lib/api/types';
	import ColumnComponent from './Column.svelte';
	import BoardToolbar from './BoardToolbar.svelte';

	const labelMap = $derived(new Map(boardStore.labels.map((l) => [l.id, l])));

	let editMode = $state(false);

	let columnItems = $state<Column[]>([]);
	$effect(() => {
		columnItems = boardStore.columns;
	});

	function handleColumnConsider(e: CustomEvent<DndEvent<Column>>) {
		columnItems = e.detail.items;
	}

	async function handleColumnFinalize(e: CustomEvent<DndEvent<Column>>) {
		const next = e.detail.items;
		columnItems = next;
		const movedId = e.detail.info.id;
		const idx = next.findIndex((c) => c.id === movedId);
		if (idx === -1) return;
		const before = idx > 0 ? next[idx - 1].position : undefined;
		const after = idx < next.length - 1 ? next[idx + 1].position : undefined;
		const newPosition = computePosition(before, after);
		await boardStore.updateColumn(movedId, { position: newPosition });
	}
</script>

<div class="flex min-h-0 flex-1 flex-col gap-4">
	<BoardToolbar {editMode} ontoggle={() => (editMode = !editMode)} />

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
			No columns yet. Click "Edit board" to add your first column.
		</div>
	{:else if editMode}
		<div
			class="flex min-h-0 w-full flex-1 items-stretch gap-3 pb-3"
			use:dndzone={{ items: columnItems, type: 'column', flipDurationMs: 150, dropTargetStyle: {} }}
			onconsider={handleColumnConsider}
			onfinalize={handleColumnFinalize}
		>
			{#each columnItems as column (column.id)}
				<div animate:flip={{ duration: 150 }} class="flex min-w-0 flex-1 basis-0">
					<ColumnComponent
						{column}
						tasks={boardStore.tasksForColumn(column.id)}
						{labelMap}
						{editMode}
					/>
				</div>
			{/each}
		</div>
	{:else}
		<div class="flex min-h-0 w-full flex-1 items-stretch gap-3 pb-3">
			{#each boardStore.columns as column (column.id)}
				<ColumnComponent
					{column}
					tasks={boardStore.tasksForColumn(column.id)}
					{labelMap}
					{editMode}
				/>
			{/each}
		</div>
	{/if}
</div>
