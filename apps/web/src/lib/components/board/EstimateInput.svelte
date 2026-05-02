<script lang="ts">
	interface Props {
		value: number | null;
		disabled?: boolean;
		onChange: (next: number | null) => void;
	}

	const { value, disabled = false, onChange }: Props = $props();

	const display = $derived(value === null ? '' : String(value));

	function handleInput(e: Event) {
		const raw = (e.currentTarget as HTMLInputElement).value;
		if (raw === '') {
			onChange(null);
			return;
		}
		const parsed = Number(raw);
		if (Number.isFinite(parsed) && parsed >= 0) {
			onChange(parsed);
		}
	}
</script>

<input
	type="number"
	min="0"
	step="0.25"
	placeholder="hrs"
	value={display}
	{disabled}
	oninput={handleInput}
	class="w-24 rounded-md border border-[var(--border)] bg-[var(--card)] px-2 py-1 text-sm text-[var(--card-foreground)] disabled:opacity-60"
/>
