<script lang="ts">
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { projects } from '$lib/projects.svelte';
	import { chat } from '$lib/chat.svelte';

	const LAST_CHANNEL_KEY = 'weld:last-chat-channel';

	$effect(() => {
		(async () => {
			await projects.ensureLoaded();
			if (!projects.team) return;
			await chat.loadForTeam(projects.team.id);
			const last = browser ? localStorage.getItem(LAST_CHANNEL_KEY) : null;
			const target = chat.channels.find((c) => c.id === last) ?? chat.channels[0];
			if (target) {
				void goto(resolve(`/chat/${target.id}` as '/chat'), { replaceState: true });
			}
		})();
	});
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Chat</h1>
	<p class="text-sm text-[var(--muted-foreground)]">Loading channels…</p>
</header>

{#if chat.activeTeamId && chat.channels.length === 0}
	<div
		class="rounded-lg border border-dashed border-[var(--border)] bg-[var(--card)] p-10 text-center text-sm text-[var(--muted-foreground)]"
	>
		No channels in this team yet. Create one from the sidebar.
	</div>
{/if}
