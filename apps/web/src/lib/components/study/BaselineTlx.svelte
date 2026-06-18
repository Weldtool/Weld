<script lang="ts">
	import { study } from '$lib/study.svelte';
	import { baselineTlxItems } from '$lib/survey-config';

	// Same six NASA-TLX subscales as the exit survey, ids suffixed `_baseline`.
	const items = baselineTlxItems();

	// Sliders default to the midpoint visually but count as answered only once
	// touched, mirroring the exit-survey TLX so the two measures are comparable.
	let responses = $state<Record<string, string>>({});
	const missing = $derived(items.filter((it) => responses[it.id] === undefined).map((it) => it.id));
	let submitted = false;

	function submit() {
		if (missing.length > 0 || submitted) return;
		submitted = true;
		study.submitPostBaselineTlx({ ...responses });
	}
</script>

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
	role="dialog"
	aria-modal="true"
	aria-labelledby="baseline-tlx-title"
>
	<div
		class="flex max-h-[90vh] w-full max-w-md flex-col overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--card)] shadow-xl"
	>
		<div class="border-b border-[var(--border)] px-5 py-3">
			<span id="baseline-tlx-title" class="text-sm font-medium">Before you continue</span>
			<p class="mt-1 text-xs text-[var(--muted-foreground)]">
				Thinking about the warm-up tasks you just did on your own, rate each item. Next you’ll do a
				new set of tasks with an AI assistant.
			</p>
		</div>

		<div class="space-y-6 overflow-y-auto px-5 py-4">
			{#each items as item (item.id)}
				{@const touched = responses[item.id] !== undefined}
				<div>
					<p class="text-sm font-medium">{item.label}</p>
					<input
						type="range"
						min="1"
						max="100"
						step="1"
						value={touched ? responses[item.id] : '50'}
						oninput={(e) => (responses[item.id] = e.currentTarget.value)}
						class="mt-2 w-full accent-[var(--primary)]"
						aria-label={item.label}
					/>
					<div class="mt-1 flex items-center justify-between text-[11px] text-[var(--muted-foreground)]">
						<span>{item.lowAnchor ?? ''}</span>
						<span class="text-sm font-medium text-[var(--foreground)]">
							{touched ? responses[item.id] : 'Drag to set'}
						</span>
						<span>{item.highAnchor ?? ''}</span>
					</div>
				</div>
			{/each}
		</div>

		<div class="flex items-center justify-end border-t border-[var(--border)] px-5 py-3">
			<button
				type="button"
				onclick={submit}
				disabled={missing.length > 0}
				class="rounded-md bg-[var(--primary)] px-4 py-1.5 text-xs font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
			>
				Continue to main tasks
			</button>
		</div>
	</div>
</div>
