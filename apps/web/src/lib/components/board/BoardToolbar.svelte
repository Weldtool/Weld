<script lang="ts">
	import { boardStore } from '$lib/board.svelte';
	import type { Priority } from '$lib/api/types';
	import { Plus, X } from 'lucide-svelte';

	let creatingColumn = $state(false);
	let columnName = $state('');
	let creatingError = $state<string | null>(null);
	let columnInputEl: HTMLInputElement | null = $state(null);

	$effect(() => {
		if (creatingColumn) columnInputEl?.focus();
	});

	const priorities: Priority[] = ['critical', 'high', 'medium', 'low', 'none'];

	async function addColumn(event: SubmitEvent) {
		event.preventDefault();
		const name = columnName.trim();
		if (!name) return;
		creatingError = null;
		try {
			await boardStore.createColumn(name);
			columnName = '';
			creatingColumn = false;
		} catch (err) {
			creatingError = err instanceof Error ? err.message : 'Failed to create column.';
		}
	}
</script>

<div class="flex flex-wrap items-center gap-2">
	<input
		type="search"
		placeholder="Search tasks…"
		value={boardStore.filters.search}
		oninput={(e) => boardStore.setFilter('search', e.currentTarget.value)}
		class="w-56 rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
	/>

	<select
		value={boardStore.filters.priority ?? ''}
		onchange={(e) =>
			boardStore.setFilter('priority', (e.currentTarget.value || null) as Priority | null)}
		class="rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm"
	>
		<option value="">All priorities</option>
		{#each priorities as p (p)}
			<option value={p}>{p}</option>
		{/each}
	</select>

	<select
		value={boardStore.filters.labelId ?? ''}
		onchange={(e) => boardStore.setFilter('labelId', e.currentTarget.value || null)}
		class="rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm"
	>
		<option value="">All labels</option>
		{#each boardStore.labels as label (label.id)}
			<option value={label.id}>{label.name}</option>
		{/each}
	</select>

	{#if boardStore.filters.search || boardStore.filters.priority || boardStore.filters.labelId || boardStore.filters.assigneeId}
		<button
			type="button"
			onclick={() => boardStore.clearFilters()}
			class="inline-flex items-center gap-1 text-xs text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
		>
			<X class="h-3.5 w-3.5" />
			Clear
		</button>
	{/if}

	<div class="ml-auto flex items-center gap-2">
		{#if creatingColumn}
			<form onsubmit={addColumn} class="flex items-center gap-2">
				<input
					bind:this={columnInputEl}
					type="text"
					bind:value={columnName}
					placeholder="Column name"
					class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
				<button
					type="submit"
					class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-sm font-medium text-[var(--primary-foreground)]"
				>
					Add
				</button>
				<button
					type="button"
					onclick={() => {
						creatingColumn = false;
						columnName = '';
					}}
					class="text-sm text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
				>
					Cancel
				</button>
			</form>
		{:else}
			<button
				type="button"
				onclick={() => {
					creatingColumn = true;
					creatingError = null;
				}}
				class="inline-flex items-center gap-1 rounded-md border border-[var(--border)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
			>
				<Plus class="h-4 w-4" />
				New column
			</button>
		{/if}
	</div>
</div>

{#if creatingError}
	<p
		role="alert"
		class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
	>
		{creatingError}
	</p>
{/if}
