<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Priority } from '$lib/api/types';
	import { auth } from '$lib/auth.svelte';
	import { boardStore } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';
	import { canEditTask, TaskSaver } from '$lib/task-detail.svelte';
	import AssigneePicker from './AssigneePicker.svelte';
	import DescriptionEditor from './DescriptionEditor.svelte';
	import DueDatePicker from './DueDatePicker.svelte';
	import EstimateInput from './EstimateInput.svelte';
	import LabelPicker from './LabelPicker.svelte';
	import PriorityPicker from './PriorityPicker.svelte';

	interface Props {
		taskId: string;
	}

	const { taskId }: Props = $props();

	// Source of truth: board store. We mirror it into local state so that
	// in-flight edits aren't clobbered by incoming WS updates while a field
	// is being edited.
	const task = $derived(boardStore.taskById(taskId));

	const role = $derived(projects.team?.role ?? 'viewer');
	const editable = $derived(task ? canEditTask(task, role, auth.user?.id ?? '') : false);

	const saver = new TaskSaver(() => taskId);

	// Title is a plain input → mirror the store value while not focused so
	// incoming WS updates don't clobber typed-but-not-saved text. The Tiptap
	// description editor handles its own focus-aware reconciliation.
	let titleFocused = $state(false);
	let titleLocal = $state('');

	$effect(() => {
		if (!task) return;
		if (!titleFocused) titleLocal = task.title;
	});

	function close() {
		const url = new URL(page.url);
		url.searchParams.delete('task');
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve('/board') is the base; query string is appended dynamically
		goto(resolve('/board') + url.search, { replaceState: false, keepFocus: false });
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}

	function patch<T extends keyof Parameters<TaskSaver['queue']>[0]>(
		field: T,
		value: Parameters<TaskSaver['queue']>[0][T]
	) {
		saver.queue({ [field]: value } as Parameters<TaskSaver['queue']>[0]);
	}

	function onTitleInput(e: Event) {
		const next = (e.currentTarget as HTMLInputElement).value;
		titleLocal = next;
		patch('title', next);
	}

	function onDescriptionInput(next: string) {
		const trimmed = next.trim();
		patch('description', trimmed === '' ? null : next);
	}

	function onAssigneeChange(next: string | null) {
		patch('assignee_id', next);
	}

	function onPriorityChange(next: Priority) {
		patch('priority', next);
	}

	function onDueChange(next: string | null) {
		patch('due_date', next);
	}

	function onEstimateChange(next: number | null) {
		patch('estimate_hours', next);
	}

	function onLabelsChange(next: string[]) {
		patch('labels', next);
	}

	let confirmDelete = $state(false);
	let deleting = $state(false);

	async function handleDelete() {
		if (!task) return;
		deleting = true;
		try {
			await boardStore.deleteTask(task.id);
			close();
		} catch (err) {
			saver.error = err instanceof Error ? err.message : 'Delete failed.';
		} finally {
			deleting = false;
		}
	}

	const reporter = $derived(
		task ? boardStore.members.find((m) => m.user_id === task.reporter_id) : undefined
	);
</script>

<svelte:window onkeydown={onKey} />

<div class="fixed inset-0 z-50 flex">
	<button
		type="button"
		aria-label="Close task detail"
		class="flex-1 bg-black/30 backdrop-blur-sm"
		onclick={close}
	></button>

	<div
		role="dialog"
		aria-modal="true"
		aria-labelledby="task-detail-title"
		class="flex h-full w-full max-w-xl flex-col gap-4 overflow-y-auto border-l border-[var(--border)] bg-[var(--card)] p-6 text-[var(--card-foreground)] shadow-xl"
	>
		<header class="flex items-start justify-between gap-3">
			<input
				id="task-detail-title"
				type="text"
				disabled={!editable}
				value={titleLocal}
				oninput={onTitleInput}
				onfocus={() => (titleFocused = true)}
				onblur={() => {
					titleFocused = false;
					saver.flush();
				}}
				class="flex-1 rounded-md border border-transparent bg-transparent px-2 py-1 text-lg font-semibold focus:border-[var(--border)] focus:outline-none disabled:opacity-80"
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

		{#if !task}
			<p class="text-sm text-[var(--muted-foreground)]">Task not found.</p>
		{:else}
			{#if saver.saving}
				<p class="text-xs text-[var(--muted-foreground)] italic" role="status">Saving…</p>
			{/if}
			{#if saver.error}
				<p
					role="alert"
					class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-600 dark:text-red-400"
				>
					{saver.error}
				</p>
			{/if}

			<DescriptionEditor
				value={task.description ?? ''}
				disabled={!editable}
				saving={saver.saving}
				onInput={onDescriptionInput}
				onBlur={() => saver.flush()}
			/>

			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-1">
					<p class="text-xs text-[var(--muted-foreground)]">Assignee</p>
					<AssigneePicker
						value={task.assignee_id}
						members={boardStore.members}
						disabled={!editable}
						onChange={onAssigneeChange}
					/>
				</div>

				<div class="space-y-1">
					<p class="text-xs text-[var(--muted-foreground)]">Priority</p>
					<PriorityPicker value={task.priority} disabled={!editable} onChange={onPriorityChange} />
				</div>

				<div class="space-y-1">
					<p class="text-xs text-[var(--muted-foreground)]">Due date</p>
					<DueDatePicker value={task.due_date} disabled={!editable} onChange={onDueChange} />
				</div>

				<div class="space-y-1">
					<p class="text-xs text-[var(--muted-foreground)]">Estimate (hours)</p>
					<EstimateInput
						value={task.estimate_hours}
						disabled={!editable}
						onChange={onEstimateChange}
					/>
				</div>
			</div>

			<div class="space-y-1">
				<p class="text-xs text-[var(--muted-foreground)]">Labels</p>
				<LabelPicker value={task.labels} disabled={!editable} onChange={onLabelsChange} />
			</div>

			<dl class="grid grid-cols-2 gap-2 text-xs text-[var(--muted-foreground)]">
				<div>
					<dt>Reporter</dt>
					<dd class="text-[var(--card-foreground)]">{reporter?.display_name ?? '—'}</dd>
				</div>
				<div>
					<dt>Created</dt>
					<dd class="text-[var(--card-foreground)]">
						{new Date(task.created_at).toLocaleString()}
					</dd>
				</div>
			</dl>

			{#if editable}
				<div class="mt-4 border-t border-[var(--border)] pt-4">
					{#if confirmDelete}
						<p class="mb-2 text-sm">Delete this task? This cannot be undone.</p>
						<div class="flex gap-2">
							<button
								type="button"
								class="rounded-md bg-red-600 px-3 py-1.5 text-sm text-white disabled:opacity-60"
								disabled={deleting}
								onclick={handleDelete}
							>
								{deleting ? 'Deleting…' : 'Confirm delete'}
							</button>
							<button
								type="button"
								class="text-sm text-[var(--muted-foreground)]"
								onclick={() => (confirmDelete = false)}
							>
								Cancel
							</button>
						</div>
					{:else}
						<button
							type="button"
							class="text-sm text-red-600 hover:underline dark:text-red-400"
							onclick={() => (confirmDelete = true)}
						>
							Delete task
						</button>
					{/if}
				</div>
			{/if}
		{/if}
	</div>
</div>
