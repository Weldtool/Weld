<script lang="ts">
	import type { TeamMember } from '$lib/api/types';

	interface Props {
		value: string | null;
		members: TeamMember[];
		disabled?: boolean;
		onChange: (next: string | null) => void;
	}

	const { value, members, disabled = false, onChange }: Props = $props();

	function handleChange(e: Event) {
		const raw = (e.currentTarget as HTMLSelectElement).value;
		onChange(raw === '' ? null : raw);
	}
</script>

<select
	value={value ?? ''}
	{disabled}
	onchange={handleChange}
	class="w-full rounded-md border border-[var(--border)] bg-[var(--card)] px-2 py-1 text-sm text-[var(--card-foreground)] disabled:opacity-60"
>
	<option value="">Unassigned</option>
	{#each members as member (member.user_id)}
		<option value={member.user_id}>
			{member.display_name} ({member.email})
		</option>
	{/each}
</select>
