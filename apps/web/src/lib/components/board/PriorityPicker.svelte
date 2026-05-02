<script lang="ts">
	import type { Priority } from '$lib/api/types';

	interface Props {
		value: Priority;
		disabled?: boolean;
		onChange: (next: Priority) => void;
	}

	const { value, disabled = false, onChange }: Props = $props();

	const options: Priority[] = ['none', 'low', 'medium', 'high', 'critical'];
</script>

<select
	{value}
	{disabled}
	onchange={(e) => onChange((e.currentTarget as HTMLSelectElement).value as Priority)}
	class="rounded-md border border-[var(--border)] bg-[var(--card)] px-2 py-1 text-sm text-[var(--card-foreground)] disabled:opacity-60"
>
	{#each options as opt (opt)}
		<option value={opt}>{opt}</option>
	{/each}
</select>
