<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tasks as tasksApi, github as githubApi } from '$lib/api';
	import type { GitHubIssue, Priority } from '$lib/api/types';
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

	// ── Import from GitHub ─────────────────────────────────────────────────
	// When a repo is linked, the user can pull an existing open issue straight
	// into the draft — autofilling title/description/labels — instead of typing
	// a task from scratch. If they do, the created task is *linked* to that
	// issue rather than pushing a brand-new one.
	let ghIssues = $state<GitHubIssue[]>([]);
	let issuesLoading = $state(false);
	let importedIssue = $state<number | null>(null);

	$effect(() => {
		if (!hasRepo) return;
		const projectId = projects.current?.id;
		if (!projectId) return;
		issuesLoading = true;
		githubApi
			.listIssues(projectId)
			.then((issues) => (ghIssues = issues))
			.catch(() => {})
			.finally(() => (issuesLoading = false));
	});

	// Map GitHub label names (case-insensitive) onto Weld labels, creating any
	// that don't exist yet, and return their IDs.
	async function resolveLabels(ghLabels: { name: string; color: string }[]): Promise<string[]> {
		const ids: string[] = [];
		for (const gh of ghLabels) {
			const existing = boardStore.labels.find(
				(l) => l.name.toLowerCase() === gh.name.toLowerCase()
			);
			const id = existing
				? existing.id
				: ((await boardStore.createLabel(gh.name, `#${gh.color}`))?.id ?? null);
			if (id && !ids.includes(id)) ids.push(id);
		}
		return ids;
	}

	async function importIssue(issueNumber: number) {
		const projectId = projects.current?.id;
		if (!projectId) return;
		error = null;
		let detail;
		try {
			detail = await githubApi.getIssue(projectId, issueNumber);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load GitHub issue.';
			return;
		}
		importedIssue = issueNumber;
		title = detail.title;
		if (detail.body) description = detail.body;
		if ((detail.labels ?? []).length > 0) {
			labels = await resolveLabels(detail.labels);
		}
	}

	function clearImport() {
		importedIssue = null;
	}

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
				const repo = projects.current?.github_repo;
				if (importedIssue !== null && repo) {
					// Imported from an existing issue → link rather than create a dupe.
					try {
						const linked = await tasksApi.link(created.id, {
							repo,
							issue_number: importedIssue
						});
						boardStore.applyTaskUpdate(linked);
					} catch (err) {
						error = err instanceof Error ? err.message : 'Created locally, GitHub link failed.';
					}
				} else if (hasRepo && pushToGithub) {
					try {
						const pushed = await tasksApi.pushToGithub(created.id);
						boardStore.applyTaskUpdate(pushed);
					} catch (err) {
						// Soft-fail: the task exists; the push is the optional bit.
						error = err instanceof Error ? err.message : 'Created locally, GitHub push failed.';
					}
				}
				close();
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to create task.';
		} finally {
			creating = false;
		}
	}
</script>

<svelte:window onkeydown={onKey} />

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
	role="presentation"
	onclick={close}
>
	<div
		role="dialog"
		aria-modal="true"
		aria-labelledby="task-draft-title"
		class="flex h-[85vh] w-full max-w-2xl flex-col rounded-lg border border-[var(--border)] bg-[var(--card)] text-[var(--card-foreground)] shadow-xl"
		onclick={(e) => e.stopPropagation()}
	>
		<header class="flex shrink-0 items-start gap-3 border-b border-[var(--border)] px-6 py-4">
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

		<div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 py-4">
			{#if error}
				<p
					role="alert"
					class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-600 dark:text-red-400"
				>
					{error}
				</p>
			{/if}

			{#if hasRepo}
				<div class="space-y-1 rounded-md border border-[var(--border)] bg-[var(--muted)]/30 p-3">
					{#if importedIssue !== null}
						<div class="flex items-center justify-between gap-2 text-sm">
							<span class="text-[var(--muted-foreground)]">
								Imported from <code>{projects.current?.github_repo}</code> issue
								<span class="font-medium text-[var(--foreground)]">#{importedIssue}</span>
							</span>
							<button
								type="button"
								class="text-xs text-[var(--muted-foreground)] hover:underline"
								onclick={clearImport}
							>
								Clear
							</button>
						</div>
					{:else}
						<label class="text-xs text-[var(--muted-foreground)]" for="import-issue">
							Import from GitHub issue
						</label>
						<select
							id="import-issue"
							disabled={issuesLoading || ghIssues.length === 0}
							onchange={(e) => {
								const v = (e.currentTarget as HTMLSelectElement).value;
								if (v) void importIssue(parseInt(v, 10));
								(e.currentTarget as HTMLSelectElement).value = '';
							}}
							class="w-full rounded border border-[var(--border)] bg-[var(--background)] px-2 py-1 text-sm focus:ring-1 focus:ring-[var(--ring)] focus:outline-none disabled:opacity-60"
						>
							<option value="">
								{#if issuesLoading}
									Loading open issues…
								{:else if ghIssues.length === 0}
									No open issues
								{:else}
									Select an issue to autofill…
								{/if}
							</option>
							{#each ghIssues as issue (issue.number)}
								<option value={issue.number}>#{issue.number} — {issue.title}</option>
							{/each}
						</select>
					{/if}
				</div>
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
		</div>

		<div class="shrink-0 space-y-3 border-t border-[var(--border)] px-6 py-4">
			{#if hasRepo && importedIssue === null}
				<label
					class="flex cursor-pointer items-center gap-2 text-xs text-[var(--muted-foreground)]"
				>
					<input type="checkbox" bind:checked={pushToGithub} class="rounded" />
					Also create issue on <code>{projects.current?.github_repo}</code>
				</label>
			{:else if importedIssue !== null}
				<p class="text-xs text-[var(--muted-foreground)]">
					Will link to issue <span class="font-medium">#{importedIssue}</span> on
					<code>{projects.current?.github_repo}</code>.
				</p>
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
