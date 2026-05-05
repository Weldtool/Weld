<script lang="ts">
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { chat } from '$lib/chat.svelte';
	import ChannelSidebar from '$lib/components/chat/ChannelSidebar.svelte';
	import NewChannelModal from '$lib/components/chat/NewChannelModal.svelte';
	import { projects } from '$lib/projects.svelte';
	import { browser } from '$app/environment';
	import { onDestroy } from 'svelte';

	const LAST_CHANNEL_KEY = 'weld:last-chat-channel';

	let { children } = $props();

	let showNewChannel = $state(false);

	$effect(() => {
		(async () => {
			await projects.ensureLoaded();
			if (projects.team) await chat.loadForTeam(projects.team.id);
		})();
	});

	$effect(() => {
		const id = chat.activeChannelId;
		if (id && browser) localStorage.setItem(LAST_CHANNEL_KEY, id);
	});

	$effect(() => {
		const selfId = auth.user?.id;
		if (!selfId || !browser) return;
		const unsub = chat.onMention(selfId, (msg) => {
			if (msg.channel_id === chat.activeChannelId && document.hasFocus()) return;
			if ('Notification' in window && Notification.permission === 'granted') {
				const sender = chat.memberById(msg.sender_id);
				new Notification(`${sender?.display_name ?? 'Someone'} mentioned you`, {
					body: msg.content.slice(0, 140),
					tag: msg.id
				});
			}
		});
		if ('Notification' in window && Notification.permission === 'default') {
			void Notification.requestPermission().catch(() => {});
		}
		return unsub;
	});

	onDestroy(() => {
		// Keep store hot across route changes within /chat; reset only on logout (handled in root layout).
	});

	const isChannelRoute = $derived(/^\/chat\/[^/]+/.test(page.url.pathname));
</script>

<div class="-m-8 flex h-[calc(100vh)] overflow-hidden">
	<ChannelSidebar
		channels={chat.channels}
		activeChannelId={chat.activeChannelId}
		onCreateChannel={() => (showNewChannel = true)}
	/>
	<div class="flex min-w-0 flex-1 flex-col">
		{@render children()}
		{#if !isChannelRoute && chat.channels.length > 0}
			<div class="m-auto p-6 text-sm text-[var(--muted-foreground)]">
				Select a channel from the sidebar.
			</div>
		{/if}
	</div>
</div>

{#if showNewChannel}
	<NewChannelModal onClose={() => (showNewChannel = false)} />
{/if}
