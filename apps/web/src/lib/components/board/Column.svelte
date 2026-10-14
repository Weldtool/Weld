<script lang="ts">
	import { userMessage } from '$lib/api';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import { flip } from 'svelte/animate';
	import { boardStore, computePosition } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';
	import { createPrDialog } from '$lib/create-pr-dialog.svelte';
	import type { Column, Label, Task, TeamMember } from '$lib/api/types';
	import TaskCard from './TaskCard.svelte';
	import { Plus, Trash2, GripVertical } from '@lucide/svelte';
	import { modal } from '$lib/modal';

	interface Props {
		column: Column;
		tasks: Task[];
		labelMap: Map<string, Label>;
		memberMap: Map<string, TeamMember>;
		editMode: boolean;
	}

	const { column, tasks, labelMap, memberMap, editMode }: Props = $props();

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
		const originColumnId = dropped.column_id;
		const cameFromAnotherColumn = originColumnId !== column.id;
		const newPosition = computePosition(before, after);
		const ok = await boardStore.moveTask(droppedId, column.id, newPosition);

		if (ok && cameFromAnotherColumn) {
			const project = projects.current;
			const moved = boardStore.tasks.find((t) => t.id === droppedId);
			if (
				project &&
				project.review_column_id === column.id &&
				project.github_repo &&
				moved &&
				moved.github_pr_id == null
			) {
				createPrDialog.request(moved, originColumnId);
			}
		}
	}

	const limitClass = $derived(
		column.wip_limit !== null && tasks.length > column.wip_limit
			? 'text-red-600 dark:text-red-400'
			: 'text-[var(--muted-foreground)]'
	);

	function openAdd() {
		const url = new URL(page.url);
		url.searchParams.set('task', 'new');
		url.searchParams.set('column', column.id);
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(resolve('/board') + url.search, { replaceState: false, noScroll: true });
	}

	// ── Column rename ────────────────────────────────────────────────────────
	let renaming = $state(false);
	let renameValue = $state('');
	let renameBusy = $state(false);
	let renameInputEl: HTMLInputElement | null = $state(null);

	$effect(() => {
		if (renaming) renameInputEl?.focus();
	});

	// Reset rename state when leaving edit mode
	$effect(() => {
		if (!editMode) renaming = false;
	});

	function startRename() {
		renameValue = column.name;
		renaming = true;
	}

	async function commitRename() {
		const name = renameValue.trim();
		if (!name || name === column.name) {
			renaming = false;
			return;
		}
		renameBusy = true;
		try {
			await boardStore.updateColumn(column.id, { name });
		} finally {
			renameBusy = false;
			renaming = false;
		}
	}

	function handleRenameKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			void commitRename();
		} else if (e.key === 'Escape') {
			renaming = false;
		}
	}

	// ── Column delete ────────────────────────────────────────────────────────
	let showDeleteConfirm = $state(false);
	let moveTarget = $state('');
	let deleteBusy = $state(false);
	let deleteError = $state<string | null>(null);

	const otherColumns = $derived(boardStore.columns.filter((c) => c.id !== column.id));

	function openDelete() {
		deleteError = null;
		const backlog = otherColumns.find((c) => c.name.toLowerCase() === 'backlog');
		moveTarget = (backlog ?? otherColumns[0])?.id ?? '';
		showDeleteConfirm = true;
	}

	async function confirmDelete() {
		deleteBusy = true;
		deleteError = null;
		try {
			await boardStore.deleteColumn(column.id, tasks.length > 0 ? moveTarget : null);
			showDeleteConfirm = false;
		} catch (err) {
			deleteError = userMessage(err, 'Failed to delete column.');
		} finally {
			deleteBusy = false;
		}
	}
</script>

<section
	class="flex h-full w-full flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--muted)]/30 p-3
		{editMode ? 'border-dashed' : ''}"
>
	<header class="flex items-center justify-between gap-1.5">
		{#if editMode}
			<GripVertical class="h-4 w-4 shrink-0 cursor-grab text-[var(--muted-foreground)]" />
		{/if}

		{#if renaming}
			<input
				bind:this={renameInputEl}
				type="text"
				bind:value={renameValue}
				onblur={commitRename}
				onkeydown={handleRenameKeydown}
				disabled={renameBusy}
				class="min-w-0 flex-1 rounded border border-[var(--input)] bg-[var(--background)] px-2 py-0.5 text-sm font-semibold outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			/>
		{:else}
			<button
				type="button"
				onclick={editMode ? startRename : undefined}
				title={editMode ? 'Click to rename' : undefined}
				class="min-w-0 flex-1 truncate text-left text-sm font-semibold tracking-tight
					{editMode ? 'cursor-text hover:text-[var(--primary)]' : 'cursor-default'}"
			>
				{column.name}
			</button>
		{/if}

		<div class="flex shrink-0 items-center gap-1.5">
			<span class="text-xs {limitClass}">
				{tasks.length}{column.wip_limit !== null ? `/${column.wip_limit}` : ''}
			</span>

			{#if editMode}
				<button
					type="button"
					onclick={openDelete}
					title="Delete column"
					aria-label="Delete column"
					class="rounded p-0.5 text-red-500 hover:bg-[var(--accent)] hover:text-red-600 dark:text-red-400"
				>
					<Trash2 class="h-4 w-4" />
				</button>
			{:else}
				<button
					type="button"
					onclick={openAdd}
					title="New task"
					aria-label="New task"
					class="rounded p-0.5 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
				>
					<Plus class="h-4 w-4" />
				</button>
			{/if}
		</div>
	</header>

	{#if editMode}
		<!-- In edit mode show tasks as a static non-interactive list -->
		<div class="flex min-h-12 flex-1 flex-col gap-2 overflow-y-auto opacity-50">
			{#each tasks as task (task.id)}
				<TaskCard {task} {labelMap} {memberMap} />
			{/each}
		</div>
	{:else}
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
					<TaskCard {task} {labelMap} {memberMap} />
				</div>
			{/each}
		</div>
	{/if}
</section>

{#if showDeleteConfirm}
	<dialog
		use:modal={() => (showDeleteConfirm = false)}
		aria-labelledby="delete-column-title"
		class="m-auto w-full max-w-sm rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-xl backdrop:bg-black/40"
	>
		<h2 id="delete-column-title" class="text-base font-semibold">Delete "{column.name}"</h2>

		{#if tasks.length > 0}
			<p class="mt-2 text-sm text-[var(--muted-foreground)]">
				This column has {tasks.length} task{tasks.length === 1 ? '' : 's'}. Move them to:
			</p>
			{#if otherColumns.length === 0}
				<p class="mt-2 text-sm text-red-600 dark:text-red-400">
					No other column exists to move tasks into. Create one first.
				</p>
			{:else}
				<select
					bind:value={moveTarget}
					class="mt-2 w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				>
					{#each otherColumns as col (col.id)}
						<option value={col.id}>{col.name}</option>
					{/each}
				</select>
			{/if}
		{:else}
			<p class="mt-2 text-sm text-[var(--muted-foreground)]">
				This column is empty and will be removed. This cannot be undone.
			</p>
		{/if}

		{#if deleteError}
			<p class="mt-2 text-xs text-red-500">{deleteError}</p>
		{/if}

		<div class="mt-4 flex justify-end gap-2">
			<button
				type="button"
				onclick={() => (showDeleteConfirm = false)}
				class="rounded-md border border-[var(--border)] px-3 py-1.5 text-sm hover:bg-[var(--accent)]"
			>
				Cancel
			</button>
			<button
				type="button"
				onclick={confirmDelete}
				disabled={deleteBusy || (tasks.length > 0 && (otherColumns.length === 0 || !moveTarget))}
				class="rounded-md bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:opacity-90 disabled:opacity-50"
			>
				{deleteBusy ? 'Deleting…' : 'Delete column'}
			</button>
		</div>
	</dialog>
{/if}
