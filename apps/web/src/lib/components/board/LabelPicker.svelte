<script lang="ts">
	import type { Label } from '$lib/api/types';
	import { boardStore } from '$lib/board.svelte';

	interface Props {
		value: string[];
		disabled?: boolean;
		onChange: (next: string[]) => void;
	}

	const { value, disabled = false, onChange }: Props = $props();

	let creating = $state(false);
	let newName = $state('');
	let newColor = $state('#6366f1');
	let createError = $state<string | null>(null);

	const labelMap = $derived(new Map(boardStore.labels.map((l) => [l.id, l] as const)));
	const selected = $derived(value.map((id) => labelMap.get(id)).filter((l): l is Label => !!l));
	const selectedSet = $derived(new Set(value));

	function toggle(id: string) {
		if (disabled) return;
		const next = selectedSet.has(id) ? value.filter((v) => v !== id) : [...value, id];
		onChange(next);
	}

	async function submitCreate() {
		const name = newName.trim();
		if (!name) return;
		try {
			const created = await boardStore.createLabel(name, newColor);
			if (created) {
				onChange([...value, created.id]);
			}
			newName = '';
			creating = false;
			createError = null;
		} catch (err) {
			createError = err instanceof Error ? err.message : 'Failed to create label.';
		}
	}
</script>

<div class="space-y-2">
	{#if selected.length > 0}
		<div class="flex flex-wrap gap-1">
			{#each selected as label (label.id)}
				<button
					type="button"
					{disabled}
					onclick={() => toggle(label.id)}
					class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] font-medium disabled:opacity-60"
					style="background-color: {label.color}20; color: {label.color};"
				>
					{label.name}
					{#if !disabled}
						<span aria-hidden="true">×</span>
					{/if}
				</button>
			{/each}
		</div>
	{/if}

	{#if !disabled}
		<details class="group">
			<summary class="cursor-pointer text-xs text-[var(--muted-foreground)] hover:underline">
				Add label
			</summary>
			<div class="mt-2 max-h-40 overflow-y-auto rounded-md border border-[var(--border)] p-2">
				{#each boardStore.labels as label (label.id)}
					{@const checked = selectedSet.has(label.id)}
					<label class="flex cursor-pointer items-center gap-2 px-1 py-0.5 text-sm">
						<input type="checkbox" {checked} onchange={() => toggle(label.id)} />
						<span
							class="rounded px-1.5 py-0.5 text-[11px] font-medium"
							style="background-color: {label.color}20; color: {label.color};"
						>
							{label.name}
						</span>
					</label>
				{/each}

				{#if creating}
					<div class="mt-2 flex flex-col gap-2 border-t border-[var(--border)] pt-2">
						<input
							type="text"
							bind:value={newName}
							placeholder="New label name"
							class="rounded-md border border-[var(--border)] bg-[var(--card)] px-2 py-1 text-sm"
						/>
						<input type="color" bind:value={newColor} class="h-8 w-16" />
						{#if createError}
							<p class="text-xs text-red-500">{createError}</p>
						{/if}
						<div class="flex gap-2">
							<button
								type="button"
								class="rounded-md bg-[var(--primary)] px-2 py-1 text-xs text-[var(--primary-foreground)]"
								onclick={submitCreate}
							>
								Create
							</button>
							<button
								type="button"
								class="text-xs text-[var(--muted-foreground)]"
								onclick={() => {
									creating = false;
									createError = null;
								}}
							>
								Cancel
							</button>
						</div>
					</div>
				{:else}
					<button
						type="button"
						class="mt-2 w-full text-left text-xs text-[var(--muted-foreground)] hover:underline"
						onclick={() => (creating = true)}
					>
						+ Create new label
					</button>
				{/if}
			</div>
		</details>
	{/if}
</div>
