<script lang="ts">
	import type { GitHubPR, GitHubPRFile } from '$lib/api/types';
	import { github } from '$lib/api';
	import { appConfig } from '$lib/config.svelte';
	import { telemetry } from '$lib/telemetry.svelte';
	import { prReviews, type PrReview, type ReviewVerdict } from '$lib/pr-reviews.svelte';
	import { boardStore } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';
	import { X, GitPullRequest, FileDiff, Check, GitMerge } from '@lucide/svelte';
	import { modal } from '$lib/modal';

	interface Props {
		pr: GitHubPR;
		repo: string;
		/** The card this PR is linked to, so the review can drive its column the
		 *  way a real merge/close webhook would. */
		taskId?: string;
		onClose: () => void;
	}
	const { pr, repo, taskId, onClose }: Props = $props();

	// Restore any prior review for this PR so re-opening shows the recorded state.
	// Read once: the overlay is mounted per-open (its `{#if}` clears to null on
	// close), so `pr` cannot change identity underneath it.
	// svelte-ignore state_referenced_locally
	const existing = prReviews.get(pr.number);
	let verdict = $state<ReviewVerdict>(existing?.verdict ?? 'approve');
	let comment = $state(existing?.comment ?? '');
	let submitted = $state<PrReview | null>(existing);
	let hint = $state<string | null>(null);
	let busy = $state(false);
	let moved = $state<string | null>(null);

	// The list endpoint the caller got `pr` from carries no file diffs outside
	// study mode, so the real product fetches the full PR on open. Until it
	// lands, `pr` still drives the header.
	// svelte-ignore state_referenced_locally
	let detail = $state<GitHubPR>(pr);
	let loadingFiles = $state(!appConfig.studyMode);
	let loadError = $state<string | null>(null);

	$effect(() => {
		if (appConfig.studyMode) return;
		const projectId = projects.current?.id;
		if (!projectId) return;
		let cancelled = false;
		github
			.getPR(projectId, pr.number)
			.then((full) => {
				if (!cancelled) detail = full;
			})
			.catch((e) => {
				if (!cancelled) loadError = e instanceof Error ? e.message : 'Could not load the diff.';
			})
			.finally(() => {
				if (!cancelled) loadingFiles = false;
			});
		return () => {
			cancelled = true;
		};
	});

	const VERDICTS: { value: ReviewVerdict; label: string }[] = [
		{ value: 'approve', label: 'Approve' },
		{ value: 'comment', label: 'Comment' },
		{ value: 'request_changes', label: 'Request changes' }
	];

	// Record that the participant opened the review view (T6 / B4 telemetry). The
	// telemetry helper is inert outside study mode, so this is a no-op in the
	// normal product.
	$effect(() => {
		telemetry.log('pr_review_opened', { number: pr.number });
	});

	function fileTone(status: string): string {
		switch (status) {
			case 'added':
				return 'text-green-600 dark:text-green-400';
			case 'removed':
				return 'text-red-600 dark:text-red-400';
			case 'renamed':
				return 'text-blue-600 dark:text-blue-400';
			default:
				return 'text-amber-600 dark:text-amber-400';
		}
	}

	// Classify a unified-diff line for coloring.
	function lineKind(line: string): 'hunk' | 'add' | 'del' | 'ctx' {
		if (line.startsWith('@@')) return 'hunk';
		if (line.startsWith('+')) return 'add';
		if (line.startsWith('-')) return 'del';
		return 'ctx';
	}

	function patchLines(file: GitHubPRFile): string[] {
		return file.patch ? file.patch.split('\n') : [];
	}

	// Mirrors what GitHub's webhooks do to a linked card: a merged PR sends it to
	// Done, a PR closed without merging sends it back to In Progress. Study mode
	// has no webhooks (GitHub is fixtures), so the review view drives it directly.
	async function moveCard(columnId: string | null, label: string) {
		if (!taskId || !columnId) return;
		const task = boardStore.taskById(taskId);
		if (!task || task.column_id === columnId) return;
		busy = true;
		const ok = await boardStore.moveToColumnEnd(taskId, columnId);
		busy = false;
		if (ok) moved = label;
	}

	async function submit() {
		if (verdict !== 'approve' && comment.trim() === '') {
			hint = 'Add a comment to leave this kind of review.';
			return;
		}
		hint = null;
		const payload: PrReview = { verdict, comment: comment.trim(), merged: false };

		// Outside study mode the verdict has to actually reach GitHub, or this
		// is just a button that colours a badge.
		const projectId = projects.current?.id;
		if (!appConfig.studyMode && projectId) {
			busy = true;
			try {
				await github.submitReview(projectId, pr.number, {
					verdict,
					body: payload.comment
				});
			} catch (e) {
				hint = e instanceof Error ? e.message : 'GitHub rejected the review.';
				return;
			} finally {
				busy = false;
			}
		}

		telemetry.log('pr_review_submitted', {
			number: pr.number,
			task_id: taskId,
			verdict,
			has_comment: payload.comment.length > 0,
			comment_length: payload.comment.length,
			comment: payload.comment
		});
		submitted = payload;
		prReviews.set(pr.number, payload);

		// Requesting changes hands the work back to the author — same as a PR
		// closed unmerged. Approving alone ships nothing; that needs the merge.
		if (verdict === 'request_changes') {
			await moveCard(projects.current?.in_progress_column_id ?? null, 'In Progress');
		}
	}

	async function merge() {
		if (!submitted || submitted.verdict !== 'approve' || submitted.merged) return;
		const payload: PrReview = { ...submitted, merged: true };

		const projectId = projects.current?.id;
		if (!appConfig.studyMode && projectId) {
			busy = true;
			try {
				await github.mergePR(projectId, pr.number);
			} catch (e) {
				// Branch protection, conflicts and failing checks all land here.
				hint = e instanceof Error ? e.message : 'GitHub could not merge this pull request.';
				return;
			} finally {
				busy = false;
			}
			// The merge webhook moves the card, so skip the local nudge below.
			telemetry.log('pr_merged', { number: pr.number, task_id: taskId });
			submitted = payload;
			prReviews.set(pr.number, payload);
			return;
		}

		telemetry.log('pr_merged', { number: pr.number, task_id: taskId });
		submitted = payload;
		prReviews.set(pr.number, payload);
		await moveCard(projects.current?.done_column_id ?? null, 'Done');
	}

	function verdictText(v: ReviewVerdict): string {
		return v === 'approve'
			? 'approved'
			: v === 'request_changes'
				? 'requested changes on'
				: 'commented on';
	}
</script>

<!-- Opened from inside the task modal. Both are top-layer dialogs, so this one
     stacks above it and Escape / click-off dismiss only this one. -->
<dialog
	use:modal={onClose}
	aria-labelledby="pr-review-title"
	class="m-auto flex h-[85vh] w-full max-w-3xl flex-col rounded-lg border border-[var(--border)] bg-[var(--card)] text-[var(--card-foreground)] shadow-xl backdrop:bg-black/40"
>
	<!-- Header -->
	<div class="flex items-start justify-between gap-3 border-b border-[var(--border)] px-5 py-3">
		<div class="min-w-0">
			<h2 id="pr-review-title" class="flex items-center gap-2 text-base font-semibold">
				<GitPullRequest class="h-4 w-4 shrink-0 text-green-600 dark:text-green-400" />
				<span class="truncate">{detail.title}</span>
			</h2>
			<p class="mt-0.5 text-xs text-[var(--muted-foreground)]">
				{repo} · #{pr.number} · {pr.user.login}
				{#if detail.changed_files}
					· {detail.changed_files} file{detail.changed_files === 1 ? '' : 's'}
					<span class="text-green-600 dark:text-green-400">+{detail.additions ?? 0}</span>
					<span class="text-red-600 dark:text-red-400">−{detail.deletions ?? 0}</span>
				{/if}
			</p>
		</div>
		<button
			type="button"
			class="rounded p-1 hover:bg-[var(--accent)]"
			onclick={onClose}
			aria-label="Close"
		>
			<X class="h-4 w-4" />
		</button>
	</div>

	<div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-5">
		<!-- Description -->
		{#if detail.body}
			<section>
				<h3
					class="mb-1 text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase"
				>
					Description
				</h3>
				<p class="text-sm whitespace-pre-wrap text-[var(--foreground)]">{detail.body}</p>
			</section>
		{/if}

		<!-- Files changed -->
		<section>
			<h3
				class="mb-2 flex items-center gap-1.5 text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase"
			>
				<FileDiff class="h-3.5 w-3.5" />
				Files changed
			</h3>
			<div class="flex flex-col gap-3">
				{#if loadingFiles}
					<p class="text-xs text-[var(--muted-foreground)] italic">Loading the diff…</p>
				{:else if loadError}
					<p class="text-xs text-amber-600 dark:text-amber-400">{loadError}</p>
				{/if}
				{#each detail.files ?? [] as file (file.filename)}
					<div class="overflow-hidden rounded-md border border-[var(--border)]">
						<div
							class="flex items-center justify-between gap-2 border-b border-[var(--border)] bg-[var(--accent)] px-3 py-1.5"
						>
							<span class="truncate font-mono text-xs text-[var(--foreground)]"
								>{file.filename}</span
							>
							<span class="flex shrink-0 items-center gap-2 text-[10px]">
								<span class="font-medium {fileTone(file.status)}">{file.status}</span>
								<span class="text-green-600 dark:text-green-400">+{file.additions}</span>
								<span class="text-red-600 dark:text-red-400">−{file.deletions}</span>
							</span>
						</div>
						<pre class="overflow-x-auto text-xs leading-relaxed"><code
								>{#each patchLines(file) as line, i (i)}<span
										class="block px-3 {lineKind(line) === 'add'
											? 'bg-green-500/10 text-green-700 dark:text-green-300'
											: lineKind(line) === 'del'
												? 'bg-red-500/10 text-red-700 dark:text-red-300'
												: lineKind(line) === 'hunk'
													? 'bg-blue-500/10 text-[var(--muted-foreground)]'
													: 'text-[var(--foreground)]'}">{line || ' '}</span
									>{/each}</code
							></pre>
					</div>
				{:else}
					{#if !loadingFiles && !loadError}
						<p class="text-xs text-[var(--muted-foreground)] italic">No file changes to show.</p>
					{/if}
				{/each}
			</div>
		</section>
	</div>

	<!-- Review action -->
	<div class="border-t border-[var(--border)] p-4">
		{#if submitted}
			<div class="flex flex-col gap-2">
				<div class="flex items-center gap-2 text-sm">
					{#if submitted.merged}
						<GitMerge class="h-4 w-4 shrink-0 text-purple-600 dark:text-purple-400" />
						<span class="text-purple-700 dark:text-purple-400">
							You merged this pull request.
						</span>
					{:else}
						<Check
							class="h-4 w-4 shrink-0 {submitted.verdict === 'request_changes'
								? 'text-amber-600 dark:text-amber-400'
								: 'text-green-600 dark:text-green-400'}"
						/>
						<span
							class={submitted.verdict === 'request_changes'
								? 'text-amber-700 dark:text-amber-400'
								: 'text-green-700 dark:text-green-400'}
						>
							You {verdictText(submitted.verdict)} this pull request.
						</span>
						<button
							type="button"
							class="ml-auto text-xs text-[var(--muted-foreground)] underline hover:text-[var(--foreground)]"
							onclick={() => {
								submitted = null;
								moved = null;
							}}
						>
							Update review
						</button>
					{/if}
				</div>

				{#if moved}
					<p class="text-xs text-[var(--muted-foreground)]">
						Card moved to <span class="font-medium text-[var(--foreground)]">{moved}</span>.
					</p>
				{/if}

				<!-- Approving is a signal, not a ship: merging is the separate act that
				     closes the loop, exactly as on GitHub. -->
				{#if submitted.verdict === 'approve' && !submitted.merged}
					<div class="flex items-center gap-2">
						<button
							type="button"
							disabled={busy}
							onclick={merge}
							class="inline-flex items-center gap-1.5 rounded-md bg-purple-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-purple-700 disabled:opacity-60"
						>
							<GitMerge class="h-4 w-4" />
							{busy ? 'Merging…' : 'Merge pull request'}
						</button>
						<span class="text-xs text-[var(--muted-foreground)]">
							Approving doesn't ship it — merging does.
						</span>
					</div>
				{/if}
			</div>
		{:else}
			<div class="flex flex-col gap-2">
				<div class="flex flex-wrap gap-2">
					{#each VERDICTS as v (v.value)}
						<button
							type="button"
							onclick={() => (verdict = v.value)}
							class="rounded-md border px-3 py-1 text-xs font-medium transition-colors {verdict ===
							v.value
								? 'border-[var(--primary)] bg-[var(--primary)] text-[var(--primary-foreground)]'
								: 'border-[var(--input)] hover:bg-[var(--accent)]'}"
						>
							{v.label}
						</button>
					{/each}
				</div>
				<textarea
					bind:value={comment}
					rows="2"
					placeholder="Leave a comment (optional for approve)…"
					class="w-full resize-y rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				></textarea>
				{#if hint}
					<p class="text-xs text-red-500">{hint}</p>
				{/if}
				<div class="flex justify-end">
					<button
						type="button"
						onclick={submit}
						disabled={busy}
						class="rounded-md bg-[var(--primary)] px-4 py-1.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-60"
					>
						{busy ? 'Submitting…' : 'Submit review'}
					</button>
				</div>
			</div>
		{/if}
	</div>
</dialog>
