<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import { flip } from 'svelte/animate';
	import { boardStore, computePosition } from '$lib/board.svelte';
	import type { Column, Label, Task } from '$lib/api/types';
	import TaskCard from './TaskCard.svelte';
	import { Plus } from 'lucide-svelte';

	interface Props {
		column: Column;
		tasks: Task[];
		labelMap: Map<string, Label>;
	}

	const { column, tasks, labelMap }: Props = $props();

	let items = $state<Task[]>([]);

	$effect(() => {
		items = tasks;
	});

	function handleConsider(e: CustomEvent<DndEvent<Task>>) {
		items = e.detail.items;
	}

	async function handleFinalize(e: CustomEvent<DndEvent<Task>>) {
		const next = e.detail.items;
		items = next;
		const droppedId = e.detail.info.id;
		const idx = next.findIndex((t) => t.id === droppedId);
		if (idx === -1) return;
		const before = idx > 0 ? next[idx - 1].position : undefined;
		const after = idx < next.length - 1 ? next[idx + 1].position : undefined;
		const dropped = next[idx];
		// Skip if neither column nor index changed.
		if (dropped.column_id === column.id && dropped.position === before /* sentinel */) {
			// no-op
		}
		const newPosition = computePosition(before, after);
		await boardStore.moveTask(droppedId, column.id, newPosition);
	}

	const limitClass = $derived(
		column.wip_limit !== null && tasks.length > column.wip_limit
			? 'text-red-600 dark:text-red-400'
			: 'text-[var(--muted-foreground)]'
	);

	// Inline title entry at top of column
	let adding = $state(false);
	let newTitle = $state('');
	let creating = $state(false);
	let titleInput: HTMLInputElement | null = $state(null);

	$effect(() => {
		if (adding) titleInput?.focus();
	});

	function openAdd() {
		newTitle = '';
		adding = true;
	}

	function cancelAdd() {
		adding = false;
		newTitle = '';
	}

	async function submitAdd(e: SubmitEvent) {
		e.preventDefault();
		const trimmed = newTitle.trim();
		if (!trimmed || creating) return;
		creating = true;
		try {
			const created = await boardStore.createTask(column.id, trimmed);
			adding = false;
			newTitle = '';
			if (created) {
				const url = new URL(page.url);
				url.searchParams.set('task', created.id);
				// eslint-disable-next-line svelte/no-navigation-without-resolve
				void goto(resolve('/board') + url.search, { replaceState: false, noScroll: true });
			}
		} catch {
			// createTask already sets boardStore.error
		} finally {
			creating = false;
		}
	}
</script>

<section
	class="flex h-full min-w-0 flex-1 basis-0 flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--muted)]/30 p-3"
>
	<header class="flex items-center justify-between">
		<h2 class="text-sm font-semibold tracking-tight">{column.name}</h2>
		<div class="flex items-center gap-1.5">
			<span class="text-xs {limitClass}">
				{tasks.length}{column.wip_limit !== null ? `/${column.wip_limit}` : ''}
			</span>
			<button
				type="button"
				onclick={openAdd}
				title="New task"
				aria-label="New task"
				class="rounded p-0.5 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
			>
				<Plus class="h-4 w-4" />
			</button>
		</div>
	</header>

	{#if adding}
		<form onsubmit={submitAdd} class="flex flex-col gap-1.5">
			<input
				bind:this={titleInput}
				bind:value={newTitle}
				type="text"
				placeholder="Task title…"
				class="rounded border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			/>
			<div class="flex justify-end gap-2 text-xs">
				<button
					type="button"
					onclick={cancelAdd}
					class="text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
				>
					Cancel
				</button>
				<button
					type="submit"
					disabled={creating || !newTitle.trim()}
					class="rounded bg-[var(--primary)] px-2 py-1 font-medium text-[var(--primary-foreground)] disabled:opacity-50"
				>
					{creating ? 'Creating…' : 'Create'}
				</button>
			</div>
		</form>
	{/if}

	<div
		class="flex min-h-12 flex-1 flex-col gap-2 overflow-y-auto"
		use:dndzone={{
			items,
			type: 'task',
			flipDurationMs: 150,
			dropTargetStyle: {}
		}}
		onconsider={handleConsider}
		onfinalize={handleFinalize}
	>
		{#each items as task (task.id)}
			<div animate:flip={{ duration: 150 }}>
				<TaskCard {task} {labelMap} />
			</div>
		{/each}
	</div>
</section>
