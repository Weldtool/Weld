<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import { flip } from 'svelte/animate';
	import { boardStore, computePosition } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';
	import { createPrDialog } from '$lib/create-pr-dialog.svelte';
	import type { Column, Label, Task } from '$lib/api/types';
	import TaskCard from './TaskCard.svelte';
	import { Plus, MoreVertical, Trash2 } from 'lucide-svelte';

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
		const cameFromAnotherColumn = dropped.column_id !== column.id;
		const newPosition = computePosition(before, after);
		const ok = await boardStore.moveTask(droppedId, column.id, newPosition);

		// Moving a card into the project's Review column prompts a PR. Only when
		// it's a genuine column change, the repo is linked, and there's no PR yet.
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
				createPrDialog.request(moved);
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

	// ── Column delete ────────────────────────────────────────────────────────
	let menuOpen = $state(false);
	let showDeleteConfirm = $state(false);
	let moveTarget = $state('');
	let deleteBusy = $state(false);
	let deleteError = $state<string | null>(null);

	const otherColumns = $derived(boardStore.columns.filter((c) => c.id !== column.id));

	function openDelete() {
		menuOpen = false;
		deleteError = null;
		// Default the move target to a Backlog column, else the first other column.
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
			deleteError = err instanceof Error ? err.message : 'Failed to delete column.';
		} finally {
			deleteBusy = false;
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
			<div class="relative">
				<button
					type="button"
					onclick={() => (menuOpen = !menuOpen)}
					title="Column options"
					aria-label="Column options"
					class="rounded p-0.5 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
				>
					<MoreVertical class="h-4 w-4" />
				</button>
				{#if menuOpen}
					<!-- click-away backdrop -->
					<button
						type="button"
						aria-label="Close menu"
						class="fixed inset-0 z-10 cursor-default"
						onclick={() => (menuOpen = false)}
					></button>
					<div
						class="absolute right-0 z-20 mt-1 w-40 overflow-hidden rounded-md border border-[var(--border)] bg-[var(--card)] py-1 shadow-lg"
					>
						<button
							type="button"
							onclick={openDelete}
							class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-red-600 hover:bg-[var(--accent)] dark:text-red-400"
						>
							<Trash2 class="h-3.5 w-3.5" />
							Delete column
						</button>
					</div>
				{/if}
			</div>
		</div>
	</header>

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

{#if showDeleteConfirm}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
		role="dialog"
		aria-modal="true"
		aria-labelledby="delete-column-title"
	>
		<div
			class="w-full max-w-sm rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-xl"
		>
			<h2 id="delete-column-title" class="text-base font-semibold">Delete “{column.name}”</h2>

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
		</div>
	</div>
{/if}
