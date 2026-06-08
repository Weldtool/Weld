<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tasks as tasksApi, github as githubApi } from '$lib/api';
	import type { GitHubIssue, GitHubPR, Priority } from '$lib/api/types';
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
	import CreateIssueDialog from './CreateIssueDialog.svelte';
	import { X } from 'lucide-svelte';

	interface Props {
		taskId: string;
	}

	const { taskId }: Props = $props();

	const task = $derived(boardStore.taskById(taskId));
	const role = $derived(projects.team?.role ?? 'viewer');
	const editable = $derived(task ? canEditTask(task, role, auth.user?.id ?? '') : false);

	const saver = new TaskSaver(() => taskId);

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

	// Priority pill config
	type PriorityOption = {
		value: Priority;
		label: string;
		activeClass: string;
		inactiveClass: string;
	};
	const priorityOptions: PriorityOption[] = [
		{
			value: 'none',
			label: 'None',
			activeClass:
				'border-[var(--border)] bg-[var(--accent)] text-[var(--foreground)] font-semibold',
			inactiveClass:
				'border-[var(--border)] bg-transparent text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]'
		},
		{
			value: 'low',
			label: 'Low',
			activeClass: 'border-blue-500/40 bg-blue-500/15 text-blue-600 dark:text-blue-400 font-semibold',
			inactiveClass:
				'border-[var(--border)] bg-transparent text-[var(--muted-foreground)] hover:border-blue-500/30 hover:bg-blue-500/10 hover:text-blue-600 dark:hover:text-blue-400'
		},
		{
			value: 'medium',
			label: 'Medium',
			activeClass:
				'border-amber-500/40 bg-amber-500/15 text-amber-600 dark:text-amber-400 font-semibold',
			inactiveClass:
				'border-[var(--border)] bg-transparent text-[var(--muted-foreground)] hover:border-amber-500/30 hover:bg-amber-500/10 hover:text-amber-600 dark:hover:text-amber-400'
		},
		{
			value: 'high',
			label: 'High',
			activeClass:
				'border-orange-500/40 bg-orange-500/15 text-orange-600 dark:text-orange-400 font-semibold',
			inactiveClass:
				'border-[var(--border)] bg-transparent text-[var(--muted-foreground)] hover:border-orange-500/30 hover:bg-orange-500/10 hover:text-orange-600 dark:hover:text-orange-400'
		},
		{
			value: 'critical',
			label: 'Critical',
			activeClass: 'border-red-500/40 bg-red-500/15 text-red-600 dark:text-red-400 font-semibold',
			inactiveClass:
				'border-[var(--border)] bg-transparent text-[var(--muted-foreground)] hover:border-red-500/30 hover:bg-red-500/10 hover:text-red-600 dark:hover:text-red-400'
		}
	];

	// GitHub link
	let linkRepo = $state('');
	let linkError = $state('');
	let linkBusy = $state(false);
	let showIssueForm = $state(false);
	let showPRForm = $state(false);
	let addIssueValue = $state('');
	let addPRValue = $state('');
	let showIssueDialog = $state(false);
	let openIssues = $state<GitHubIssue[]>([]);
	let openPRs = $state<GitHubPR[]>([]);

	$effect(() => {
		if (!showIssueForm && !showPRForm) return;
		const projectId = projects.current?.id;
		const githubRepo = projects.current?.github_repo ?? task?.github_repo;
		if (githubRepo) linkRepo = githubRepo;
		if (projectId && githubRepo) {
			githubApi
				.listIssues(projectId)
				.then((issues) => (openIssues = issues))
				.catch(() => {});
			githubApi
				.listPRs(projectId)
				.then((prs) => (openPRs = prs))
				.catch(() => {});
		}
	});

	// Link/replace just the issue, preserving any existing PR
	async function submitIssueLink() {
		if (!task) return;
		const repo = task.github_repo ?? linkRepo.trim();
		if (!repo) { linkError = 'Repository is required (e.g. owner/repo).'; return; }
		const issueNum = addIssueValue ? parseInt(addIssueValue, 10) : null;
		if (!issueNum) return;
		linkError = '';
		linkBusy = true;
		try {
			const linked = await tasksApi.link(task.id, {
				repo,
				issue_number: issueNum,
				pr_number: task.github_pr_id ?? null
			});
			boardStore.applyTaskUpdate(linked);
			telemetry.log('issue_linked_to_card', {
				task_id: linked.id,
				short_id: linked.short_id,
				repo,
				issue_number: issueNum
			});
			const projectId = projects.current?.id;
			if (projectId) await autofillFromIssue(linked.id, projectId, issueNum);
			showIssueForm = false;
			addIssueValue = '';
		} catch (err) {
			linkError = err instanceof Error ? err.message : 'Link failed.';
		} finally {
			linkBusy = false;
		}
	}

	// Link/replace just the PR, preserving any existing issue
	async function submitPRLink() {
		if (!task) return;
		const repo = task.github_repo ?? linkRepo.trim();
		if (!repo) { linkError = 'Repository is required (e.g. owner/repo).'; return; }
		const prNum = addPRValue ? parseInt(addPRValue, 10) : null;
		if (!prNum) return;
		linkError = '';
		linkBusy = true;
		try {
			const linked = await tasksApi.link(task.id, {
				repo,
				issue_number: task.github_issue_id ?? null,
				pr_number: prNum
			});
			boardStore.applyTaskUpdate(linked);
			telemetry.log('pr_linked_to_card', {
				task_id: linked.id,
				short_id: linked.short_id,
				repo,
				pr_number: prNum
			});
			showPRForm = false;
			addPRValue = '';
		} catch (err) {
			linkError = err instanceof Error ? err.message : 'Link failed.';
		} finally {
			linkBusy = false;
		}
	}

	// Remove just the issue link; keep PR if present
	async function removeIssueLink() {
		if (!task?.github_repo) return;
		linkBusy = true;
		linkError = '';
		try {
			let updated;
			if (task.github_pr_id) {
				updated = await tasksApi.link(task.id, {
					repo: task.github_repo,
					issue_number: null,
					pr_number: task.github_pr_id
				});
			} else {
				updated = await tasksApi.unlink(task.id);
			}
			boardStore.applyTaskUpdate(updated);
		} catch (err) {
			linkError = err instanceof Error ? err.message : 'Unlink failed.';
		} finally {
			linkBusy = false;
		}
	}

	// Remove just the PR link; keep issue if present
	async function removePRLink() {
		if (!task?.github_repo) return;
		linkBusy = true;
		linkError = '';
		try {
			let updated;
			if (task.github_issue_id) {
				updated = await tasksApi.link(task.id, {
					repo: task.github_repo,
					issue_number: task.github_issue_id,
					pr_number: null
				});
			} else {
				updated = await tasksApi.unlink(task.id);
			}
			boardStore.applyTaskUpdate(updated);
		} catch (err) {
			linkError = err instanceof Error ? err.message : 'Unlink failed.';
		} finally {
			linkBusy = false;
		}
	}

	async function autofillFromIssue(taskIdToPatch: string, projectId: string, issueNumber: number) {
		let detail;
		try {
			detail = await githubApi.getIssue(projectId, issueNumber);
		} catch {
			return;
		}

		const current = boardStore.taskById(taskIdToPatch);
		if (!current) return;

		const patchData: Parameters<typeof tasksApi.update>[1] = {};
		if (!current.title || current.title.trim() === '') {
			patchData.title = detail.title;
		}
		if ((!current.description || current.description.trim() === '') && detail.body) {
			patchData.description = detail.body;
		}

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
				patchData.labels = merged;
			}
		}

		if (Object.keys(patchData).length === 0) return;
		try {
			const patched = await tasksApi.update(taskIdToPatch, patchData);
			boardStore.applyTaskUpdate(patched);
			if (!titleFocused) titleLocal = patched.title;
		} catch {
			// autofill is best-effort
		}
	}


</script>

<svelte:window onkeydown={onKey} />

<!-- Backdrop -->
<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm"
	role="presentation"
	onclick={close}
>
	<!-- Modal -->
	<div
		role="dialog"
		aria-modal="true"
		aria-labelledby="task-detail-title"
		class="flex h-full max-h-[88vh] w-full max-w-4xl flex-col overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--card)] shadow-2xl"
		onclick={(e) => e.stopPropagation()}
	>
		<!-- Header -->
		<div class="flex shrink-0 items-start gap-3 border-b border-[var(--border)] px-6 py-4">
			<div class="min-w-0 flex-1">
				{#if task?.short_id}
					<p class="mb-1 font-mono text-[10px] font-medium tracking-wider text-[var(--muted-foreground)]">
						{task.short_id}
					</p>
				{/if}
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
					placeholder="Task title"
					class="w-full rounded-md border border-transparent bg-transparent px-1 py-0.5 text-xl font-semibold tracking-tight focus:border-[var(--border)] focus:outline-none disabled:opacity-80"
				/>
			</div>
			<div class="flex shrink-0 items-center gap-2 pt-1">
				{#if saver.saving}
					<span
						class="rounded-full bg-[var(--muted)] px-2.5 py-0.5 text-[10px] font-medium text-[var(--muted-foreground)]"
					>
						Saving…
					</span>
				{/if}
				<button
					type="button"
					class="rounded-md p-1.5 text-[var(--muted-foreground)] transition-colors hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
					onclick={close}
					aria-label="Close"
				>
					<X class="h-4 w-4" />
				</button>
			</div>
		</div>

		<!-- Body: two columns -->
		<div class="flex min-h-0 flex-1">
			<!-- Left: description -->
			<div class="flex flex-1 flex-col overflow-hidden px-6 py-5">
				{#if !task}
					<p class="text-sm text-[var(--muted-foreground)]">Task not found.</p>
				{:else}
					{#if saver.error}
						<p
							role="alert"
							class="mb-4 shrink-0 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-600 dark:text-red-400"
						>
							{saver.error}
						</p>
					{/if}
					<DescriptionEditor
						value={task.description ?? ''}
						disabled={!editable}
						saving={saver.saving}
						fill={true}
						onInput={onDescriptionInput}
						onBlur={() => saver.flush()}
					/>
				{/if}
			</div>

			<!-- Right: sidebar -->
			{#if task}
				<aside
					class="w-72 shrink-0 overflow-y-auto border-l border-[var(--border)] bg-[var(--muted)] px-5 py-5"
				>
					<div class="space-y-5">
						<!-- Assignee -->
						<div class="space-y-1.5">
							<p
								class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
							>
								Assignee
							</p>
							<AssigneePicker
								value={task.assignee_id}
								members={boardStore.members}
								disabled={!editable}
								onChange={onAssigneeChange}
							/>
						</div>

						<!-- Priority -->
						<div class="space-y-1.5">
							<p
								class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
							>
								Priority
							</p>
							<div class="flex flex-wrap gap-1.5">
								{#each priorityOptions as opt (opt.value)}
									<button
										type="button"
										disabled={!editable}
										onclick={() => onPriorityChange(opt.value)}
										class="rounded-full border px-2.5 py-0.5 text-[11px] transition-colors disabled:pointer-events-none disabled:opacity-50 {task.priority ===
										opt.value
											? opt.activeClass
											: opt.inactiveClass}"
									>
										{opt.label}
									</button>
								{/each}
							</div>
						</div>

						<!-- Due Date -->
						<div class="space-y-1.5">
							<p
								class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
							>
								Due date
							</p>
							<DueDatePicker
								value={task.due_date}
								disabled={!editable}
								onChange={onDueChange}
							/>
						</div>

						<!-- Estimate -->
						<div class="space-y-1.5">
							<p
								class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
							>
								Estimate (hours)
							</p>
							<EstimateInput
								value={task.estimate_hours}
								disabled={!editable}
								onChange={onEstimateChange}
							/>
						</div>

						<!-- Labels -->
						<div class="space-y-1.5">
							<p
								class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
							>
								Labels
							</p>
							<LabelPicker value={task.labels} disabled={!editable} onChange={onLabelsChange} />
						</div>

						<hr class="border-[var(--border)]" />

						<!-- GitHub -->
						<div class="space-y-2">
							<p
								class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
							>
								GitHub
							</p>

							<!-- Issue row -->
							{#if task.github_issue_id && task.github_repo}
								<div class="flex items-center gap-1.5">
									<a
										href="https://github.com/{task.github_repo}/issues/{task.github_issue_id}"
										target="_blank"
										rel="noopener noreferrer"
										class="inline-flex flex-1 items-center gap-1.5 rounded-md border border-[var(--border)] bg-[var(--card)] px-2.5 py-1 text-xs font-medium text-[var(--card-foreground)] hover:bg-[var(--accent)]"
										onclick={() =>
											telemetry.log('issue_referenced_from_card', {
												task_id: task?.id,
												issue_number: task?.github_issue_id
											})}
									>
										<svg class="h-3.5 w-3.5 shrink-0 text-green-500" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
											<path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" />
											<path d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z" />
										</svg>
										Issue #{task.github_issue_id}
									</a>
									{#if editable}
										<button
											type="button"
											disabled={linkBusy}
											onclick={removeIssueLink}
											aria-label="Remove issue link"
											class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-red-500 disabled:opacity-50"
										><X class="h-3.5 w-3.5" /></button>
									{/if}
								</div>
							{:else if editable}
								{#if showIssueForm}
									<div class="space-y-2 rounded-lg border border-[var(--border)] bg-[var(--card)] p-2.5">
										{#if !task.github_repo}
											<div class="space-y-1">
												<label class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60" for="gh-issue-repo">
													Repository <span class="text-red-500">*</span>
												</label>
												<input
													id="gh-issue-repo"
													type="text"
													placeholder="owner/repo"
													bind:value={linkRepo}
													class="w-full rounded-md border border-[var(--border)] bg-[var(--background)] px-2 py-1.5 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none"
												/>
											</div>
										{/if}
										<select
											bind:value={addIssueValue}
											class="w-full rounded-md border border-[var(--border)] bg-[var(--background)] px-2 py-1.5 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none"
										>
											<option value="">— select issue —</option>
											{#each openIssues as issue (issue.number)}
												<option value={issue.number.toString()}>#{issue.number} — {issue.title}</option>
											{/each}
										</select>
										<div class="flex items-center gap-2">
											<button
												type="button"
												disabled={linkBusy || !addIssueValue}
												onclick={submitIssueLink}
												class="rounded-md bg-[var(--primary)] px-2.5 py-1 text-xs font-medium text-[var(--primary-foreground)] disabled:opacity-60"
											>{linkBusy ? 'Linking…' : 'Link'}</button>
											<button
												type="button"
												onclick={() => { showIssueForm = false; addIssueValue = ''; linkError = ''; }}
												class="text-xs text-[var(--muted-foreground)] hover:underline"
											>Cancel</button>
										</div>
									</div>
								{:else}
									<button
										type="button"
										onclick={() => { showIssueForm = true; telemetry.log('github_view_opened', { task_id: task?.id, context: 'link_issue_form' }); }}
										class="flex w-full items-center gap-2 rounded-md border border-dashed border-[var(--border)] px-2.5 py-1.5 text-xs text-[var(--muted-foreground)] transition-colors hover:border-[var(--ring)]/50 hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
									>
										<svg class="h-3.5 w-3.5 shrink-0 text-green-500/60" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
											<path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" />
											<path d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z" />
										</svg>
										Link an issue
									</button>
								{/if}
							{/if}

							<!-- PR row -->
							{#if task.github_pr_id && task.github_repo}
								<div class="flex items-center gap-1.5">
									<a
										href="https://github.com/{task.github_repo}/pull/{task.github_pr_id}"
										target="_blank"
										rel="noopener noreferrer"
										class="inline-flex flex-1 items-center gap-1.5 rounded-md border border-[var(--border)] bg-[var(--card)] px-2.5 py-1 text-xs font-medium text-[var(--card-foreground)] hover:bg-[var(--accent)]"
										onclick={() =>
											telemetry.log('pr_referenced_from_card', {
												task_id: task?.id,
												pr_number: task?.github_pr_id
											})}
									>
										<svg class="h-3.5 w-3.5 shrink-0 text-purple-500" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
											<path d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354Z" />
										</svg>
										PR #{task.github_pr_id}
									</a>
									{#if editable}
										<button
											type="button"
											disabled={linkBusy}
											onclick={removePRLink}
											aria-label="Remove PR link"
											class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-red-500 disabled:opacity-50"
										><X class="h-3.5 w-3.5" /></button>
									{/if}
								</div>
							{:else if editable}
								{#if showPRForm}
									<div class="space-y-2 rounded-lg border border-[var(--border)] bg-[var(--card)] p-2.5">
										{#if !task.github_repo}
											<div class="space-y-1">
												<label class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60" for="gh-pr-repo">
													Repository <span class="text-red-500">*</span>
												</label>
												<input
													id="gh-pr-repo"
													type="text"
													placeholder="owner/repo"
													bind:value={linkRepo}
													class="w-full rounded-md border border-[var(--border)] bg-[var(--background)] px-2 py-1.5 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none"
												/>
											</div>
										{/if}
										<select
											bind:value={addPRValue}
											class="w-full rounded-md border border-[var(--border)] bg-[var(--background)] px-2 py-1.5 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none"
										>
											<option value="">— select PR —</option>
											{#each openPRs as pr (pr.number)}
												<option value={pr.number.toString()}>#{pr.number} — {pr.title}</option>
											{/each}
										</select>
										<div class="flex items-center gap-2">
											<button
												type="button"
												disabled={linkBusy || !addPRValue}
												onclick={submitPRLink}
												class="rounded-md bg-[var(--primary)] px-2.5 py-1 text-xs font-medium text-[var(--primary-foreground)] disabled:opacity-60"
											>{linkBusy ? 'Linking…' : 'Link'}</button>
											<button
												type="button"
												onclick={() => { showPRForm = false; addPRValue = ''; linkError = ''; }}
												class="text-xs text-[var(--muted-foreground)] hover:underline"
											>Cancel</button>
										</div>
									</div>
								{:else}
									<button
										type="button"
										onclick={() => { showPRForm = true; telemetry.log('github_view_opened', { task_id: task?.id, context: 'link_pr_form' }); }}
										class="flex w-full items-center gap-2 rounded-md border border-dashed border-[var(--border)] px-2.5 py-1.5 text-xs text-[var(--muted-foreground)] transition-colors hover:border-[var(--ring)]/50 hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
									>
										<svg class="h-3.5 w-3.5 shrink-0 text-purple-500/60" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
											<path d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354Z" />
										</svg>
										Link a PR
									</button>
								{/if}
							{/if}

							{#if linkError}
								<p class="text-xs text-red-600 dark:text-red-400">{linkError}</p>
							{/if}
						</div>

						<hr class="border-[var(--border)]" />

						<!-- Reporter / Created -->
						<div class="space-y-3.5">
							<div class="space-y-0.5">
								<p
									class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
								>
									Reporter
								</p>
								<p class="text-sm text-[var(--card-foreground)]">
									{reporter?.display_name ?? '—'}
								</p>
							</div>
							<div class="space-y-0.5">
								<p
									class="text-xs font-semibold uppercase tracking-wide text-[var(--foreground)]/60"
								>
									Created
								</p>
								<p class="text-sm text-[var(--card-foreground)]">
									{new Date(task.created_at).toLocaleString()}
								</p>
							</div>
						</div>
					</div>
				</aside>
			{/if}
		</div>

		<!-- Footer -->
		{#if task && editable}
			<div class="shrink-0 border-t border-[var(--border)] px-6 py-3">
				{#if confirmDelete}
					<div class="flex items-center gap-3">
						<span class="text-sm text-[var(--muted-foreground)]">
							Delete this task? This cannot be undone.
						</span>
						<button
							type="button"
							disabled={deleting}
							onclick={handleDelete}
							class="rounded-md bg-red-600 px-3 py-1.5 text-sm font-medium text-white disabled:opacity-60"
						>
							{deleting ? 'Deleting…' : 'Confirm delete'}
						</button>
						<button
							type="button"
							onclick={() => (confirmDelete = false)}
							class="text-sm text-[var(--muted-foreground)] hover:underline"
						>
							Cancel
						</button>
					</div>
				{:else}
					<button
						type="button"
						onclick={() => (confirmDelete = true)}
						class="text-sm text-red-600 hover:underline dark:text-red-400"
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
