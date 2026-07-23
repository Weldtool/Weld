<script lang="ts">
	import type { GitHubPR, GitHubPRFile } from '$lib/api/types';
	import { telemetry } from '$lib/telemetry.svelte';
	import { prReviews, type ReviewVerdict } from '$lib/pr-reviews.svelte';
	import { X, GitPullRequest, FileDiff, Check } from '@lucide/svelte';

	interface Props {
		pr: GitHubPR;
		repo: string;
		onClose: () => void;
	}
	const { pr, repo, onClose }: Props = $props();

	// Restore any prior review for this PR so re-opening shows the recorded state.
	const existing = prReviews.get(pr.number);
	let verdict = $state<ReviewVerdict>(existing?.verdict ?? 'approve');
	let comment = $state(existing?.comment ?? '');
	let submitted = $state<{ verdict: ReviewVerdict; comment: string } | null>(existing);
	let hint = $state<string | null>(null);

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

	function submit() {
		if (verdict !== 'approve' && comment.trim() === '') {
			hint = 'Add a comment to leave this kind of review.';
			return;
		}
		hint = null;
		const payload = { verdict, comment: comment.trim() };
		telemetry.log('pr_review_submitted', {
			number: pr.number,
			verdict,
			has_comment: payload.comment.length > 0,
			comment_length: payload.comment.length,
			comment: payload.comment
		});
		submitted = payload;
		prReviews.set(pr.number, payload);
	}

	function verdictText(v: ReviewVerdict): string {
		return v === 'approve' ? 'approved' : v === 'request_changes' ? 'requested changes on' : 'commented on';
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
	role="dialog"
	aria-modal="true"
	aria-labelledby="pr-review-title"
>
	<div
		class="flex h-[85vh] w-full max-w-3xl flex-col rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-xl"
	>
		<!-- Header -->
		<div class="flex items-start justify-between gap-3 border-b border-[var(--border)] px-5 py-3">
			<div class="min-w-0">
				<h2 id="pr-review-title" class="flex items-center gap-2 text-base font-semibold">
					<GitPullRequest class="h-4 w-4 shrink-0 text-green-600 dark:text-green-400" />
					<span class="truncate">{pr.title}</span>
				</h2>
				<p class="mt-0.5 text-xs text-[var(--muted-foreground)]">
					{repo} · #{pr.number} · {pr.user.login}
					{#if pr.changed_files}
						· {pr.changed_files} file{pr.changed_files === 1 ? '' : 's'}
						<span class="text-green-600 dark:text-green-400">+{pr.additions ?? 0}</span>
						<span class="text-red-600 dark:text-red-400">−{pr.deletions ?? 0}</span>
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
			{#if pr.body}
				<section>
					<h3 class="mb-1 text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase">
						Description
					</h3>
					<p class="text-sm whitespace-pre-wrap text-[var(--foreground)]">{pr.body}</p>
				</section>
			{/if}

			<!-- Files changed -->
			<section>
				<h3 class="mb-2 flex items-center gap-1.5 text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase">
					<FileDiff class="h-3.5 w-3.5" />
					Files changed
				</h3>
				<div class="flex flex-col gap-3">
					{#each pr.files ?? [] as file (file.filename)}
						<div class="overflow-hidden rounded-md border border-[var(--border)]">
							<div class="flex items-center justify-between gap-2 border-b border-[var(--border)] bg-[var(--accent)] px-3 py-1.5">
								<span class="truncate font-mono text-xs text-[var(--foreground)]">{file.filename}</span>
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
														: 'text-[var(--foreground)]'}"
											>{line || ' '}</span
										>{/each}</code
								></pre>
						</div>
					{:else}
						<p class="text-xs text-[var(--muted-foreground)] italic">No file changes to show.</p>
					{/each}
				</div>
			</section>
		</div>

		<!-- Review action -->
		<div class="border-t border-[var(--border)] p-4">
			{#if submitted}
				<div class="flex items-center gap-2 text-sm text-green-700 dark:text-green-400">
					<Check class="h-4 w-4" />
					<span>You {verdictText(submitted.verdict)} this pull request.</span>
					<button
						type="button"
						class="ml-auto text-xs text-[var(--muted-foreground)] underline hover:text-[var(--foreground)]"
						onclick={() => (submitted = null)}
					>
						Update review
					</button>
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
							class="rounded-md bg-[var(--primary)] px-4 py-1.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90"
						>
							Submit review
						</button>
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>
