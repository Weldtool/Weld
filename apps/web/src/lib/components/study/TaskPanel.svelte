<script lang="ts">
	import { study } from '$lib/study.svelte';
	import { PHASE_LABELS } from '$lib/study-tasks';
	import { ClipboardList, X } from '@lucide/svelte';
	import { modal } from '$lib/modal';
	import BaselineTlx from './BaselineTlx.svelte';

	const task = $derived(study.currentTask);
	const number = $derived(study.taskIndex + 1);
	// The just-completed task awaiting its rating (drives the rating step).
	const rating = $derived(study.ratingFor);

	let open = $state(false);

	// Per-task rating inputs, reset whenever a new task enters the rating step.
	let effort = $state<number | null>(null);
	let agentHelp = $state<number | null>(null);
	let ratedTaskId: string | null = null;
	$effect(() => {
		const r = study.ratingFor;
		if (r && r.id !== ratedTaskId) {
			ratedTaskId = r.id;
			effort = null;
			agentHelp = null;
		}
	});

	const isAgentTask = $derived(rating?.phase === 'agent');
	const canSubmitRating = $derived(effort !== null && (!isAgentTask || agentHelp !== null));

	const SCALE = [1, 2, 3, 4, 5, 6, 7];

	function submitRating() {
		if (effort === null) return;
		study.submitTaskRating(effort, isAgentTask ? agentHelp : null);
		// The next task's instruction auto-opens via the task-change effect below.
	}

	// Re-opening the instruction modal is a confusion proxy (§2.3): log
	// instruction_reopened only on genuine re-opens, not the first view per task.
	let seenTaskId: string | null = null;
	function openModal() {
		if (task && task.id === seenTaskId) {
			study.noteInstructionReopened();
		}
		if (task) seenTaskId = task.id;
		open = true;
	}

	// Auto-open the modal whenever the task changes so the participant always sees
	// the new instruction; mark it seen so the first auto-open isn't a "reopen".
	// Suppressed while the post-baseline TLX is up, so the first agent task's
	// instruction auto-opens only once the participant clears the interstitial.
	$effect(() => {
		if (!study.pendingPostBaselineTlx && task && task.id !== seenTaskId) {
			seenTaskId = task.id;
			open = true;
		}
	});
</script>

{#if study.pendingPostBaselineTlx}
	<BaselineTlx />
{/if}

<!-- Hide the task launcher + instruction modal while the post-baseline TLX
     interstitial is up, so the next (agent) task doesn't render underneath it.
     `open` may still be true from the just-finished baseline task; the modal
     re-appears (and the auto-open effect re-fires) once the TLX is submitted. -->
{#if task && !study.pendingPostBaselineTlx}
	<!-- Launcher button (bottom-center), always available behind the workspace. -->
	<button
		type="button"
		onclick={openModal}
		class="fixed top-4 left-1/2 z-40 flex -translate-x-1/2 items-center gap-2 rounded-full bg-[var(--primary)] px-4 py-2.5 text-sm font-medium text-[var(--primary-foreground)] shadow-lg hover:opacity-90"
		aria-label="Show current task"
	>
		<ClipboardList class="h-4 w-4" />
		Task {number} of {study.totalTasks}
	</button>

	{#if rating}
		<!-- Per-task rating step: gates advancement, shown after "Next task", so it
		     takes no dismissal callback — Escape and click-off do nothing. -->
		<dialog
			use:modal
			aria-labelledby="task-rating-title"
			class="m-auto flex w-full max-w-md flex-col overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--card)] text-[var(--card-foreground)] shadow-xl backdrop:bg-black/40"
		>
			<div class="border-b border-[var(--border)] px-5 py-3">
				<span id="task-rating-title" class="text-sm font-medium">Quick rating</span>
			</div>

			<div class="space-y-5 px-5 py-4">
				<fieldset>
					<legend class="mb-2 text-sm">How mentally demanding was this task?</legend>
					<div class="flex gap-1.5">
						{#each SCALE as n (n)}
							<button
								type="button"
								onclick={() => (effort = n)}
								class="h-9 flex-1 rounded-md border text-xs transition-colors {effort === n
									? 'border-[var(--ring)] bg-[var(--primary)] text-[var(--primary-foreground)]'
									: 'border-[var(--border)] hover:bg-[var(--accent)]'}"
								aria-pressed={effort === n}
							>
								{n}
							</button>
						{/each}
					</div>
					<div class="mt-1 flex justify-between text-[10px] text-[var(--muted-foreground)]">
						<span>Very low</span>
						<span>Very high</span>
					</div>
				</fieldset>

				{#if isAgentTask}
					<fieldset>
						<legend class="mb-2 text-sm">How much did the assistant help with this task?</legend>
						<div class="flex gap-1.5">
							{#each SCALE as n (n)}
								<button
									type="button"
									onclick={() => (agentHelp = n)}
									class="h-9 flex-1 rounded-md border text-xs transition-colors {agentHelp === n
										? 'border-[var(--ring)] bg-[var(--primary)] text-[var(--primary-foreground)]'
										: 'border-[var(--border)] hover:bg-[var(--accent)]'}"
									aria-pressed={agentHelp === n}
								>
									{n}
								</button>
							{/each}
						</div>
						<div class="mt-1 flex justify-between text-[10px] text-[var(--muted-foreground)]">
							<span>Not at all</span>
							<span>A great deal</span>
						</div>
					</fieldset>
				{/if}
			</div>

			<div class="flex justify-end border-t border-[var(--border)] px-5 py-3">
				<button
					type="button"
					onclick={submitRating}
					disabled={!canSubmitRating}
					class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-xs font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
				>
					{study.isLastTask ? 'Continue to survey' : 'Continue'}
				</button>
			</div>
		</dialog>
	{:else if open}
		<dialog
			use:modal={() => (open = false)}
			aria-labelledby="task-modal-title"
			class="m-auto flex w-full max-w-md flex-col overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--card)] text-[var(--card-foreground)] shadow-xl backdrop:bg-black/40"
		>
			<div
				class="flex shrink-0 items-center justify-between border-b border-[var(--border)] px-5 py-3"
			>
				<div class="flex flex-col">
					<span
						class="text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase"
					>
						{PHASE_LABELS[task.phase]}
					</span>
					<span id="task-modal-title" class="text-sm font-medium"
						>Task {number} of {study.totalTasks}</span
					>
				</div>
				<button
					type="button"
					onclick={() => (open = false)}
					aria-label="Close"
					class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)]"
				>
					<X class="h-4 w-4" />
				</button>
			</div>

			<div class="px-5 py-4">
				{#if task.phase === 'baseline'}
					<p class="mb-2 text-xs text-[var(--muted-foreground)]">
						Do this one yourself (no assistant).
					</p>
				{/if}
				<p class="text-sm leading-relaxed">{task.instruction}</p>
				<p class="mt-3 text-xs text-[var(--muted-foreground)]">
					When you believe you’ve finished, come back here and hit
					<span class="font-medium text-[var(--foreground)]">Next task</span>.
				</p>
			</div>

			<div
				class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--border)] px-5 py-3"
			>
				<button
					type="button"
					onclick={() => {
						study.skipCurrentTask();
						open = false;
					}}
					class="text-xs text-[var(--muted-foreground)] hover:underline"
				>
					Skip
				</button>
				<div class="flex gap-2">
					<button
						type="button"
						onclick={() => (open = false)}
						class="rounded-md border border-[var(--border)] px-3 py-1.5 text-xs hover:bg-[var(--accent)]"
					>
						Keep working
					</button>
					<button
						type="button"
						onclick={() => study.completeCurrentTask()}
						class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-xs font-medium text-[var(--primary-foreground)] hover:opacity-90"
					>
						{study.isLastTask ? 'Finish task' : 'Next task'}
					</button>
				</div>
			</div>
		</dialog>
	{/if}
{/if}
