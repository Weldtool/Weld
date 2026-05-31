<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Label, Task, TeamMember } from '$lib/api/types';
	import { telemetry } from '$lib/telemetry.svelte';

	interface Props {
		task: Task;
		labelMap: Map<string, Label>;
		memberMap: Map<string, TeamMember>;
	}

	const { task, labelMap, memberMap }: Props = $props();

	const assignee = $derived(task.assignee_id ? (memberMap.get(task.assignee_id) ?? null) : null);

	function initials(name: string): string {
		const parts = name.trim().split(/\s+/);
		const first = parts[0]?.[0] ?? '';
		const last = parts.length > 1 ? (parts[parts.length - 1][0] ?? '') : '';
		return (first + last).toUpperCase();
	}

	function open() {
		telemetry.log('card_opened', { task_id: task.id, short_id: task.short_id, title: task.title });
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
		<div class="flex items-center gap-1.5">
			{#if task.github_issue_id}
				{#if task.github_repo}
					<a
						href="https://github.com/{task.github_repo}/issues/{task.github_issue_id}"
						target="_blank"
						rel="noopener noreferrer"
						onclick={(e) => e.stopPropagation()}
						title="View GitHub issue #{task.github_issue_id}"
						class="inline-flex items-center gap-0.5 hover:text-[var(--foreground)] hover:underline"
						aria-label="View GitHub issue #{task.github_issue_id}"
					>
						<svg class="h-3 w-3" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
							<path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" />
							<path
								d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z"
							/>
						</svg>
						#{task.github_issue_id}
					</a>
				{:else}
					<span class="inline-flex items-center gap-0.5" aria-label="GitHub issue">
						<svg class="h-3 w-3" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
							<path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" />
							<path
								d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z"
							/>
						</svg>
						#{task.github_issue_id}
					</span>
				{/if}
			{/if}
			{#if task.github_pr_id}
				{#if task.github_repo}
					<a
						href="https://github.com/{task.github_repo}/pull/{task.github_pr_id}"
						target="_blank"
						rel="noopener noreferrer"
						onclick={(e) => e.stopPropagation()}
						title="View GitHub PR #{task.github_pr_id}"
						class="inline-flex items-center gap-0.5 hover:text-[var(--foreground)] hover:underline"
						aria-label="View GitHub pull request #{task.github_pr_id}"
					>
						<svg class="h-3 w-3" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
							<path
								d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354Z"
							/>
						</svg>
						#{task.github_pr_id}
					</a>
				{:else}
					<span class="inline-flex items-center gap-0.5" aria-label="GitHub pull request">
						<svg class="h-3 w-3" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
							<path
								d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354Z"
							/>
						</svg>
						#{task.github_pr_id}
					</span>
				{/if}
			{/if}
			{#if dueLabel}
				<span>Due {dueLabel}</span>
			{/if}
			{#if assignee}
				<span
					title="Assigned to {assignee.display_name}"
					aria-label="Assigned to {assignee.display_name}"
				>
					{#if assignee.avatar_url}
						<img
							src={assignee.avatar_url}
							alt={assignee.display_name}
							class="h-5 w-5 rounded-full object-cover"
						/>
					{:else}
						<span
							class="flex h-5 w-5 items-center justify-center rounded-full bg-[var(--muted)] text-[9px] font-semibold text-[var(--foreground)]"
						>
							{initials(assignee.display_name)}
						</span>
					{/if}
				</span>
			{:else}
				<span
					title="Unassigned"
					aria-label="Unassigned"
					class="flex h-5 w-5 items-center justify-center rounded-full border border-dashed border-[var(--border)] text-[var(--muted-foreground)]"
				>
					<svg
						class="h-3 w-3"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						aria-hidden="true"
					>
						<circle cx="12" cy="8" r="4" />
						<path d="M4 20c0-4 4-6 8-6s8 2 8 6" />
					</svg>
				</span>
			{/if}
		</div>
	</div>
</div>
