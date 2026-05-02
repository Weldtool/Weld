<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Label, Task } from '$lib/api/types';

	interface Props {
		task: Task;
		labelMap: Map<string, Label>;
	}

	const { task, labelMap }: Props = $props();

	function open() {
		const url = new URL(page.url);
		url.searchParams.set('task', task.id);
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve('/board') is the base; query string is appended dynamically
		goto(resolve('/board') + url.search, {
			replaceState: false,
			keepFocus: true,
			noScroll: true
		});
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			open();
		}
	}

	const priorityClass: Record<Task['priority'], string> = {
		critical: 'bg-red-500/20 text-red-700 dark:text-red-300',
		high: 'bg-orange-500/20 text-orange-700 dark:text-orange-300',
		medium: 'bg-yellow-500/20 text-yellow-700 dark:text-yellow-300',
		low: 'bg-blue-500/20 text-blue-700 dark:text-blue-300',
		none: 'bg-[var(--muted)] text-[var(--muted-foreground)]'
	};

	const dueLabel = $derived(
		task.due_date
			? new Date(task.due_date).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
			: null
	);
</script>

<div
	role="button"
	tabindex="0"
	onclick={open}
	onkeydown={onKey}
	class="cursor-pointer rounded-md border border-[var(--border)] bg-[var(--card)] p-3 text-left shadow-sm transition-colors hover:border-[var(--ring)] focus:border-[var(--ring)] focus:outline-none active:cursor-grabbing"
>
	<h3 class="text-sm leading-snug font-medium">{task.title}</h3>

	{#if task.labels.length > 0}
		<div class="mt-2 flex flex-wrap gap-1">
			{#each task.labels as labelId (labelId)}
				{@const label = labelMap.get(labelId)}
				{#if label}
					<span
						class="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium"
						style="background-color: {label.color}20; color: {label.color};"
					>
						{label.name}
					</span>
				{/if}
			{/each}
		</div>
	{/if}

	<div class="mt-2 flex items-center justify-between text-[11px] text-[var(--muted-foreground)]">
		<span class="rounded px-1.5 py-0.5 {priorityClass[task.priority]}">{task.priority}</span>
		{#if dueLabel}
			<span>Due {dueLabel}</span>
		{/if}
	</div>
</div>
