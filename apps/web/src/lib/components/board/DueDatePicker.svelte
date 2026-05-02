<script lang="ts">
	interface Props {
		/** ISO 8601 datetime string, or null. */
		value: string | null;
		disabled?: boolean;
		/** Receives an ISO datetime, or null when cleared. */
		onChange: (next: string | null) => void;
	}

	const { value, disabled = false, onChange }: Props = $props();

	// Convert the ISO datetime to a `yyyy-mm-dd` string for the date input.
	const dateValue = $derived(value ? value.slice(0, 10) : '');

	function handleInput(e: Event) {
		const raw = (e.currentTarget as HTMLInputElement).value;
		if (!raw) {
			onChange(null);
			return;
		}
		// Anchor at midnight UTC so round-trips are stable across time zones.
		onChange(`${raw}T00:00:00Z`);
	}
</script>

<input
	type="date"
	value={dateValue}
	{disabled}
	oninput={handleInput}
	class="rounded-md border border-[var(--border)] bg-[var(--card)] px-2 py-1 text-sm text-[var(--card-foreground)] disabled:opacity-60"
/>
