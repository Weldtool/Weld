<script lang="ts">
	import { study } from '$lib/study.svelte';
	import { PHASE_LABELS } from '$lib/study-tasks';
	import { ClipboardList, X } from 'lucide-svelte';

	const task = $derived(study.currentTask);
	const number = $derived(study.taskIndex + 1);

	let open = $state(false);

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
	$effect(() => {
		if (task && task.id !== seenTaskId) {
			seenTaskId = task.id;
			open = true;
		}
	});
</script>

{#if task}
	<!-- Launcher button (bottom-right), always available behind the workspace. -->
	<button
		type="button"
		onclick={openModal}
		class="fixed bottom-6 left-1/2 z-40 flex -translate-x-1/2 items-center gap-2 rounded-full bg-[var(--primary)] px-4 py-2.5 text-sm font-medium text-[var(--primary-foreground)] shadow-lg hover:opacity-90"
		aria-label="Show current task"
	>
		<ClipboardList class="h-4 w-4" />
		Task {number} of {study.totalTasks}
	</button>

	{#if open}
		<div
			class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
			role="dialog"
			aria-modal="true"
			aria-labelledby="task-modal-title"
		>
			<div
				class="flex w-full max-w-md flex-col overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--card)] shadow-xl"
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
							onclick={() => {
								study.completeCurrentTask();
								open = false;
							}}
							class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-xs font-medium text-[var(--primary-foreground)] hover:opacity-90"
						>
							{study.isLastTask ? 'Finish — go to survey' : 'Next task'}
						</button>
					</div>
				</div>
			</div>
		</div>
	{/if}
{/if}
