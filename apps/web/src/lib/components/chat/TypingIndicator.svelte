<script lang="ts">
	import type { TeamMember } from '$lib/api/types';

	interface Props {
		userIds: string[];
		members: TeamMember[];
		selfId: string | null;
	}

	const { userIds, members, selfId }: Props = $props();

	const names = $derived(
		userIds
			.filter((id) => id !== selfId)
			.map((id) => members.find((m) => m.user_id === id)?.display_name ?? 'Someone')
	);

	const text = $derived.by(() => {
		if (names.length === 0) return '';
		if (names.length === 1) return `${names[0]} is typing…`;
		if (names.length === 2) return `${names[0]} and ${names[1]} are typing…`;
		return 'Several people are typing…';
	});
</script>

<div class="h-4 px-2 text-[11px] text-[var(--muted-foreground)] italic" aria-live="polite">
	{text}
</div>
