<script lang="ts">
	interface Props {
		value: string;
		disabled?: boolean;
		saving?: boolean;
		onInput: (next: string) => void;
		onFocus?: () => void;
		onBlur?: () => void;
	}

	const { value, disabled = false, saving = false, onInput, onFocus, onBlur }: Props = $props();
</script>

<div class="space-y-1">
	<label
		for="task-description"
		class="flex items-center justify-between text-xs text-[var(--muted-foreground)]"
	>
		<span>Description (Markdown)</span>
		{#if saving}
			<span class="text-[10px] italic">Saving…</span>
		{/if}
	</label>
	<textarea
		id="task-description"
		rows="8"
		{disabled}
		{value}
		oninput={(e) => onInput((e.currentTarget as HTMLTextAreaElement).value)}
		onfocus={onFocus}
		onblur={onBlur}
		placeholder={disabled ? '' : 'Add a description… markdown is fine.'}
		class="w-full resize-y rounded-md border border-[var(--border)] bg-[var(--card)] px-3 py-2 font-mono text-sm text-[var(--card-foreground)] focus:border-[var(--ring)] focus:outline-none disabled:opacity-60"
	></textarea>
</div>
