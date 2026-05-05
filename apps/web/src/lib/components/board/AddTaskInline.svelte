<script lang="ts">
	import { boardStore } from '$lib/board.svelte';
	import { Plus } from 'lucide-svelte';

	interface Props {
		columnId: string;
	}

	const { columnId }: Props = $props();

	let editing = $state(false);
	let title = $state('');
	let submitting = $state(false);
	let inputEl: HTMLInputElement | null = $state(null);

	$effect(() => {
		if (editing) inputEl?.focus();
	});

	function open() {
		editing = true;
		title = '';
	}

	function cancel() {
		editing = false;
		title = '';
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const trimmed = title.trim();
		if (!trimmed) return;
		submitting = true;
		try {
			await boardStore.createTask(columnId, trimmed);
			title = '';
			editing = false;
		} catch (err) {
			boardStore.error = err instanceof Error ? err.message : 'Failed to create task.';
		} finally {
			submitting = false;
		}
	}
</script>

{#if editing}
	<form onsubmit={submit} class="flex flex-col gap-2 rounded-md border border-[var(--border)] p-2">
		<input
			bind:this={inputEl}
			type="text"
			bind:value={title}
			placeholder="Task title"
			class="rounded border border-[var(--input)] bg-[var(--background)] px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
		/>
		<div class="flex justify-end gap-2 text-xs">
			<button
				type="button"
				onclick={cancel}
				class="text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
			>
				Cancel
			</button>
			<button
				type="submit"
				disabled={submitting || !title.trim()}
				class="rounded bg-[var(--primary)] px-2 py-1 font-medium text-[var(--primary-foreground)] disabled:opacity-50"
			>
				{submitting ? 'Adding…' : 'Add'}
			</button>
		</div>
	</form>
{:else}
	<button
		type="button"
		onclick={open}
		class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
	>
		<Plus class="h-3.5 w-3.5" />
		Add task
	</button>
{/if}
