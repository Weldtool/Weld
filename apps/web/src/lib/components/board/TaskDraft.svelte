<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tasks as tasksApi } from '$lib/api';
	import type { Priority } from '$lib/api/types';
	import { boardStore } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';
	import AssigneePicker from './AssigneePicker.svelte';
	import DescriptionEditor from './DescriptionEditor.svelte';
	import DueDatePicker from './DueDatePicker.svelte';
	import EstimateInput from './EstimateInput.svelte';
	import LabelPicker from './LabelPicker.svelte';
	import PriorityPicker from './PriorityPicker.svelte';

	interface Props {
		columnId: string;
	}

	const { columnId }: Props = $props();

	let title = $state('');
	let description = $state('');
	let assigneeId = $state<string | null>(null);
	let priority = $state<Priority>('none');
	let dueDate = $state<string | null>(null);
	let estimate = $state<number | null>(null);
	let labels = $state<string[]>([]);

	let titleInput: HTMLInputElement | null = $state(null);
	let creating = $state(false);
	let error = $state<string | null>(null);

	// "Also create GitHub issue" — only shown when the project has a linked
	// repo, default on so users opt out rather than in (matches the goal of
	// keeping GitHub in sync without extra clicks).
	const hasRepo = $derived(!!projects.current?.github_repo);
	let pushToGithub = $state(true);

	$effect(() => {
		titleInput?.focus();
	});

	function close() {
		const url = new URL(page.url);
		url.searchParams.delete('task');
		url.searchParams.delete('column');
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		goto(resolve('/board') + url.search, { replaceState: false, keepFocus: false });
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}

	async function submit() {
		const trimmed = title.trim();
		if (!trimmed || creating) return;
		creating = true;
		error = null;
		try {
			const created = await boardStore.createTask({
				column_id: columnId,
				title: trimmed,
				description: description.trim() || undefined,
				assignee_id: assigneeId ?? undefined,
				priority,
				due_date: dueDate ?? undefined,
				estimate_hours: estimate ?? undefined,
				labels: labels.length ? labels : undefined
			});
			if (created) {
				if (hasRepo && pushToGithub) {
					try {
						const pushed = await tasksApi.pushToGithub(created.id);
						boardStore.applyTaskUpdate(pushed);
					} catch (err) {
						// Soft-fail: the task exists; the push is the optional bit.
						error = err instanceof Error ? err.message : 'Created locally, GitHub push failed.';
					}
				}
				const url = new URL(page.url);
				url.searchParams.set('task', created.id);
				url.searchParams.delete('column');
				// eslint-disable-next-line svelte/no-navigation-without-resolve
				void goto(resolve('/board') + url.search, { replaceState: true, noScroll: true });
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to create task.';
		} finally {
			creating = false;
		}
	}
</script>

<svelte:window onkeydown={onKey} />

<div class="fixed inset-0 z-50 flex">
	<button
		type="button"
		aria-label="Close new task"
		class="flex-1 bg-black/30 backdrop-blur-sm"
		onclick={close}
	></button>

	<div
		role="dialog"
		aria-modal="true"
		aria-labelledby="task-draft-title"
		class="flex h-full w-full max-w-xl flex-col gap-4 overflow-y-auto border-l border-[var(--border)] bg-[var(--card)] p-6 text-[var(--card-foreground)] shadow-xl"
	>
		<header class="flex items-start justify-between gap-3">
			<input
				id="task-draft-title"
				bind:this={titleInput}
				type="text"
				bind:value={title}
				placeholder="Task title…"
				class="flex-1 rounded-md border border-transparent bg-transparent px-2 py-1 text-lg font-semibold focus:border-[var(--border)] focus:outline-none"
			/>
			<button
				type="button"
				class="text-sm text-[var(--muted-foreground)] hover:underline"
				onclick={close}
				aria-label="Close"
			>
				Close
			</button>
		</header>

		{#if error}
			<p
				role="alert"
				class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-600 dark:text-red-400"
			>
				{error}
			</p>
		{/if}

		<DescriptionEditor value={description} onInput={(next) => (description = next)} />

		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-1">
				<p class="text-xs text-[var(--muted-foreground)]">Assignee</p>
				<AssigneePicker
					value={assigneeId}
					members={boardStore.members}
					onChange={(next) => (assigneeId = next)}
				/>
			</div>

			<div class="space-y-1">
				<p class="text-xs text-[var(--muted-foreground)]">Priority</p>
				<PriorityPicker value={priority} onChange={(next) => (priority = next)} />
			</div>

			<div class="space-y-1">
				<p class="text-xs text-[var(--muted-foreground)]">Due date</p>
				<DueDatePicker value={dueDate} onChange={(next) => (dueDate = next)} />
			</div>

			<div class="space-y-1">
				<p class="text-xs text-[var(--muted-foreground)]">Estimate (hours)</p>
				<EstimateInput value={estimate} onChange={(next) => (estimate = next)} />
			</div>
		</div>

		<div class="space-y-1">
			<p class="text-xs text-[var(--muted-foreground)]">Labels</p>
			<LabelPicker value={labels} onChange={(next) => (labels = next)} />
		</div>

		<div class="mt-auto space-y-3 border-t border-[var(--border)] pt-4">
			{#if hasRepo}
				<label
					class="flex cursor-pointer items-center gap-2 text-xs text-[var(--muted-foreground)]"
				>
					<input type="checkbox" bind:checked={pushToGithub} class="rounded" />
					Also create issue on <code>{projects.current?.github_repo}</code>
				</label>
			{/if}
			<div class="flex items-center justify-end gap-2">
				<button
					type="button"
					onclick={close}
					class="rounded-md px-3 py-1.5 text-sm text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
				>
					Cancel
				</button>
				<button
					type="button"
					onclick={submit}
					disabled={creating || !title.trim()}
					class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-sm font-medium text-[var(--primary-foreground)] disabled:opacity-50"
				>
					{creating ? 'Creating…' : 'Create task'}
				</button>
			</div>
		</div>
	</div>
</div>
