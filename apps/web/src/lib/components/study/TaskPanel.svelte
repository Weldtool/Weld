<script lang="ts">
	import { study } from '$lib/study.svelte';
	import { PHASE_LABELS } from '$lib/study-tasks';
	import { ChevronDown, ChevronUp } from 'lucide-svelte';

	const task = $derived(study.currentTask);
	const number = $derived(study.taskIndex + 1);

	let expanded = $state(true);

	// Collapsing then re-expanding the instruction is a confusion proxy (§2.3):
	// log instruction_reopened only on a genuine re-open, not the first render.
	function toggle() {
		const reopening = !expanded;
		expanded = !expanded;
		if (reopening) study.noteInstructionReopened();
	}

	// Reset to expanded whenever the task changes.
	let lastTaskId: string | null = null;
	$effect(() => {
		if (task && task.id !== lastTaskId) {
			lastTaskId = task.id;
			expanded = true;
		}
	});
</script>

{#if task}
	<aside
		class="fixed right-4 bottom-4 z-40 w-80 rounded-xl border border-[var(--border)] bg-[var(--card)] shadow-lg"
		aria-label="Study task panel"
	>
		<div class="flex items-center justify-between border-b border-[var(--border)] px-4 py-2.5">
			<div class="flex flex-col">
				<span class="text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase">
					{PHASE_LABELS[task.phase]}
				</span>
				<span class="text-sm font-medium">Task {number} of {study.totalTasks}</span>
			</div>
			<button
				type="button"
				onclick={toggle}
				aria-label={expanded ? 'Hide instructions' : 'Show instructions'}
				class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)]"
			>
				{#if expanded}<ChevronDown class="h-4 w-4" />{:else}<ChevronUp class="h-4 w-4" />{/if}
			</button>
		</div>

		{#if expanded}
			<div class="px-4 py-3">
				{#if task.phase === 'baseline'}
					<p class="mb-2 text-xs text-[var(--muted-foreground)]">
						Do this one yourself (no assistant).
					</p>
				{/if}
				<p class="text-sm leading-relaxed">{task.instruction}</p>
			</div>
		{/if}

		<div
			class="flex items-center justify-between gap-2 border-t border-[var(--border)] px-4 py-2.5"
		>
			<button
				type="button"
				onclick={() => study.skipCurrentTask()}
				class="text-xs text-[var(--muted-foreground)] hover:underline"
			>
				Skip
			</button>
			<button
				type="button"
				onclick={() => study.completeCurrentTask()}
				class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-xs font-medium text-[var(--primary-foreground)] hover:opacity-90"
			>
				{study.isLastTask ? 'Finish — go to survey' : "I'm done with this task"}
			</button>
		</div>
	</aside>
{/if}
