<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tasks as tasksApi, github as githubApi } from '$lib/api';
	import type { GitHubIssue, Priority } from '$lib/api/types';
	import { auth } from '$lib/auth.svelte';
	import { boardStore } from '$lib/board.svelte';
	import { appConfig } from '$lib/config.svelte';
	import { projects } from '$lib/projects.svelte';
	import { telemetry } from '$lib/telemetry.svelte';
	import { canEditTask, TaskSaver } from '$lib/task-detail.svelte';
	import { createPrDialog } from '$lib/create-pr-dialog.svelte';
	import AssigneePicker from './AssigneePicker.svelte';
	import DescriptionEditor from './DescriptionEditor.svelte';
	import DueDatePicker from './DueDatePicker.svelte';
	import EstimateInput from './EstimateInput.svelte';
	import LabelPicker from './LabelPicker.svelte';
	import PriorityPicker from './PriorityPicker.svelte';
	import CreateIssueDialog from './CreateIssueDialog.svelte';
	import { X } from 'lucide-svelte';

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

	// GitHub link
	let linkRepo = $state('');
	let linkIssue = $state('');
	let linkPR = $state('');
	let linkError = $state('');
	let linkBusy = $state(false);
	let showLinkForm = $state(false);
	let showIssueDialog = $state(false);
	let openIssues = $state<GitHubIssue[]>([]);

	$effect(() => {
		if (!showLinkForm) return;
		const projectId = projects.current?.id;
		const githubRepo = projects.current?.github_repo;
		if (githubRepo) linkRepo = githubRepo;
		if (projectId && githubRepo) {
			githubApi
				.listIssues(projectId)
				.then((issues) => (openIssues = issues))
				.catch(() => {});
		}
	});

	async function submitLink() {
		if (!task) return;
		const issueNum = linkIssue ? parseInt(linkIssue, 10) : null;
		const prNum = linkPR ? parseInt(linkPR, 10) : null;
		if (!linkRepo.trim()) {
			linkError = 'Repository is required (e.g. owner/repo).';
			return;
		}
		linkError = '';
		linkBusy = true;
		try {
			const linked = await tasksApi.link(task.id, {
				repo: linkRepo.trim(),
				issue_number: issueNum,
				pr_number: prNum
			});
			boardStore.applyTaskUpdate(linked);

			// User-performed GitHub link — the key T6 cross-surface action.
			if (prNum !== null) {
				telemetry.log('pr_linked_to_card', {
					task_id: linked.id,
					short_id: linked.short_id,
					repo: linkRepo.trim(),
					pr_number: prNum
				});
			}
			if (issueNum !== null) {
				telemetry.log('issue_linked_to_card', {
					task_id: linked.id,
					short_id: linked.short_id,
					repo: linkRepo.trim(),
					issue_number: issueNum
				});
			}

			// Autofill: when linking to an issue, pull title/description/labels
			// from GitHub for any fields the user hasn't filled in yet.
			const projectId = projects.current?.id;
			if (issueNum !== null && projectId) {
				await autofillFromIssue(linked.id, projectId, issueNum);
			}

			showLinkForm = false;
			linkRepo = '';
			linkIssue = '';
			linkPR = '';
		} catch (err) {
			linkError = err instanceof Error ? err.message : 'Link failed.';
		} finally {
			linkBusy = false;
		}
	}

	async function autofillFromIssue(taskIdToPatch: string, projectId: string, issueNumber: number) {
		let detail;
		try {
			detail = await githubApi.getIssue(projectId, issueNumber);
		} catch {
			// Fetch is best-effort — failure shouldn't break the link.
			return;
		}

		const current = boardStore.taskById(taskIdToPatch);
		if (!current) return;

		const patch: Parameters<typeof tasksApi.update>[1] = {};
		if (!current.title || current.title.trim() === '') {
			patch.title = detail.title;
		}
		if ((!current.description || current.description.trim() === '') && detail.body) {
			patch.description = detail.body;
		}

		// Map GitHub labels (by name, case-insensitive) onto Weld labels,
		// creating any that don't exist yet. Merge into the task's existing
		// labels rather than replacing.
		const ghLabels = detail.labels ?? [];
		if (ghLabels.length > 0) {
			const merged = [...current.labels];
			for (const gh of ghLabels) {
				const existing = boardStore.labels.find(
					(l) => l.name.toLowerCase() === gh.name.toLowerCase()
				);
				const id = existing
					? existing.id
					: ((await boardStore.createLabel(gh.name, `#${gh.color}`))?.id ?? null);
				if (id && !merged.includes(id)) merged.push(id);
			}
			if (merged.length !== current.labels.length) {
				patch.labels = merged;
			}
		}

		if (Object.keys(patch).length === 0) return;
		try {
			const patched = await tasksApi.update(taskIdToPatch, patch);
			boardStore.applyTaskUpdate(patched);
			if (!titleFocused) titleLocal = patched.title;
		} catch {
			// Surface nothing — the link itself succeeded, autofill is just
			// a nice-to-have.
		}
	}

	async function removeLink() {
		if (!task) return;
		linkBusy = true;
		linkError = '';
		try {
			const updated = await tasksApi.unlink(task.id);
			boardStore.applyTaskUpdate(updated);
		} catch (err) {
			linkError = err instanceof Error ? err.message : 'Unlink failed.';
		} finally {
			linkBusy = false;
		}
	}
</script>

<svelte:window onkeydown={onKey} />

<!-- Backdrop -->
<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
	role="presentation"
	onclick={close}
>
	<!-- Modal -->
	<div
		role="dialog"
		aria-modal="true"
		aria-labelledby="task-detail-title"
		class="flex h-[60vh] w-full max-w-[85vh] flex-col rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-xl"
		onclick={(e) => e.stopPropagation()}
	>
		<!-- Header -->
		<div class="flex shrink-0 items-start gap-3 border-b border-[var(--border)] px-6 py-4">
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
				class="flex-1 rounded-md border border-transparent bg-transparent px-1 py-0.5 text-lg font-semibold focus:border-[var(--border)] focus:outline-none disabled:opacity-80"
			/>
			<div class="flex shrink-0 items-center gap-2">
				{#if saver.saving}
					<span class="text-[10px] text-[var(--muted-foreground)] italic">Saving…</span>
				{/if}
				<button
					type="button"
					class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
					onclick={close}
					aria-label="Close"
				>
					<X class="h-4 w-4" />
				</button>
			</div>
		</div>

		<!-- Body (scrollable fallback for small screens) -->
		<div class="flex-1 space-y-4 overflow-y-auto px-6 py-4">
			{#if !task}
				<p class="text-sm text-[var(--muted-foreground)]">Task not found.</p>
			{:else}
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
						<PriorityPicker
							value={task.priority}
							disabled={!editable}
							onChange={onPriorityChange}
						/>
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

				<!-- GitHub link -->
				<div class="space-y-2">
					<p class="text-xs text-[var(--muted-foreground)]">GitHub</p>
					{#if task.github_repo}
						<div class="flex flex-wrap items-center gap-2 text-sm">
							{#if task.github_issue_id}
								<a
									href="https://github.com/{task.github_repo}/issues/{task.github_issue_id}"
									target="_blank"
									rel="noopener noreferrer"
									class="inline-flex items-center gap-1 rounded border border-[var(--border)] px-2 py-0.5 text-xs hover:underline"
									onclick={() =>
										telemetry.log('issue_referenced_from_card', {
											task_id: task?.id,
											issue_number: task?.github_issue_id
										})}
								>
									<svg
										class="h-3.5 w-3.5 shrink-0"
										viewBox="0 0 16 16"
										fill="currentColor"
										aria-hidden="true"
									>
										<path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" />
										<path
											d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z"
										/>
									</svg>
									Issue #{task.github_issue_id}
								</a>
							{/if}
							{#if task.github_pr_id}
								<a
									href="https://github.com/{task.github_repo}/pull/{task.github_pr_id}"
									target="_blank"
									rel="noopener noreferrer"
									class="inline-flex items-center gap-1 rounded border border-[var(--border)] px-2 py-0.5 text-xs hover:underline"
									onclick={() =>
										telemetry.log('pr_referenced_from_card', {
											task_id: task?.id,
											pr_number: task?.github_pr_id
										})}
								>
									<svg
										class="h-3.5 w-3.5 shrink-0"
										viewBox="0 0 16 16"
										fill="currentColor"
										aria-hidden="true"
									>
										<path
											d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354Z"
										/>
									</svg>
									PR #{task.github_pr_id}
								</a>
							{/if}
							<span class="text-xs text-[var(--muted-foreground)]">{task.github_repo}</span>
							{#if editable}
								<button
									type="button"
									class="text-xs text-red-600 hover:underline disabled:opacity-50 dark:text-red-400"
									disabled={linkBusy}
									onclick={removeLink}
								>
									Unlink
								</button>
							{/if}
						</div>
					{:else if editable}
						{#if showLinkForm}
							<div class="space-y-2 rounded-md border border-[var(--border)] p-3">
								<div class="space-y-1">
									<label class="text-xs text-[var(--muted-foreground)]" for="gh-repo">
										Repository <span class="text-red-500">*</span>
									</label>
									<input
										id="gh-repo"
										type="text"
										placeholder="owner/repo"
										bind:value={linkRepo}
										class="w-full rounded border border-[var(--border)] bg-[var(--background)] px-2 py-1 text-sm focus:ring-1 focus:ring-[var(--ring)] focus:outline-none"
									/>
								</div>
								<div class="grid grid-cols-2 gap-2">
									<div class="space-y-1">
										<label class="text-xs text-[var(--muted-foreground)]" for="gh-issue">
											Issue #
										</label>
										<input
											id="gh-issue"
											type="text"
											inputmode="numeric"
											list="gh-issues-list"
											placeholder="123"
											bind:value={linkIssue}
											class="w-full rounded border border-[var(--border)] bg-[var(--background)] px-2 py-1 text-sm focus:ring-1 focus:ring-[var(--ring)] focus:outline-none"
										/>
										<datalist id="gh-issues-list">
											{#each openIssues as issue (issue.number)}
												<option value={issue.number}>{issue.number} — {issue.title}</option>
											{/each}
										</datalist>
									</div>
									<div class="space-y-1">
										<label class="text-xs text-[var(--muted-foreground)]" for="gh-pr"> PR # </label>
										<input
											id="gh-pr"
											type="number"
											min="1"
											placeholder="456"
											bind:value={linkPR}
											class="w-full rounded border border-[var(--border)] bg-[var(--background)] px-2 py-1 text-sm focus:ring-1 focus:ring-[var(--ring)] focus:outline-none"
										/>
									</div>
								</div>
								{#if linkError}
									<p class="text-xs text-red-600 dark:text-red-400">{linkError}</p>
								{/if}
								<div class="flex gap-2">
									<button
										type="button"
										class="rounded bg-[var(--primary)] px-3 py-1 text-xs text-[var(--primary-foreground)] disabled:opacity-60"
										disabled={linkBusy}
										onclick={submitLink}
									>
										{linkBusy ? 'Linking…' : 'Link'}
									</button>
									<button
										type="button"
										class="text-xs text-[var(--muted-foreground)]"
										onclick={() => {
											showLinkForm = false;
											linkError = '';
										}}
									>
										Cancel
									</button>
								</div>
							</div>
						{:else}
							<div class="flex flex-wrap gap-3">
								<button
									type="button"
									class="text-xs text-[var(--muted-foreground)] hover:underline"
									onclick={() => {
										showLinkForm = true;
										telemetry.log('github_view_opened', {
											task_id: task?.id,
											context: 'link_form'
										});
									}}
								>
									+ Link GitHub issue or PR
								</button>
								{#if projects.current?.github_repo && !appConfig.studyMode}
									<button
										type="button"
										class="text-xs text-[var(--muted-foreground)] hover:underline"
										onclick={() => (showIssueDialog = true)}
									>
										+ Create GitHub issue
									</button>
								{/if}
							</div>
						{/if}
					{:else}
						<p class="text-xs text-[var(--muted-foreground)]">Not linked</p>
					{/if}

					{#if editable && projects.current?.github_repo && !task.github_pr_id && !appConfig.studyMode}
						<button
							type="button"
							class="inline-flex items-center gap-1 text-xs text-[var(--muted-foreground)] hover:underline"
							onclick={() => task && createPrDialog.request(task)}
						>
							<svg
								class="h-3.5 w-3.5 shrink-0"
								viewBox="0 0 16 16"
								fill="currentColor"
								aria-hidden="true"
							>
								<path
									d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354Z"
								/>
							</svg>
							Create pull request
						</button>
					{/if}
					{#if linkError && task.github_repo}
						<p class="text-xs text-red-600 dark:text-red-400">{linkError}</p>
					{/if}
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
			{/if}
		</div>

		<!-- Footer (delete, only for editable tasks) -->
		{#if task && editable}
			<div class="shrink-0 border-t border-[var(--border)] px-6 py-3">
				{#if confirmDelete}
					<div class="flex items-center gap-3">
						<span class="text-sm text-[var(--muted-foreground)]">
							Delete this task? This cannot be undone.
						</span>
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
							class="text-sm text-[var(--muted-foreground)] hover:underline"
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
	</div>
</div>

{#if showIssueDialog && task && projects.current?.github_repo}
	<CreateIssueDialog
		{task}
		projectId={projects.current.id}
		repo={projects.current.github_repo}
		onClose={() => (showIssueDialog = false)}
		onCreated={(updated) => boardStore.applyTaskUpdate(updated)}
	/>
{/if}
