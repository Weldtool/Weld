<script lang="ts">
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { chat } from '$lib/chat.svelte';
	import ChannelSidebar from '$lib/components/chat/ChannelSidebar.svelte';
	import ChannelSwitcher from '$lib/components/chat/ChannelSwitcher.svelte';
	import NewChannelModal from '$lib/components/chat/NewChannelModal.svelte';
	import { projects } from '$lib/projects.svelte';
	import { browser } from '$app/environment';
	import { onDestroy } from 'svelte';
	import { Menu, X } from 'lucide-svelte';

	const LAST_CHANNEL_KEY = 'weld:last-chat-channel';

	let { children } = $props();

	let showNewChannel = $state(false);
	let showSwitcher = $state(false);
	let sidebarOpen = $state(false);

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

	function onGlobalKeydown(e: KeyboardEvent) {
		if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
			e.preventDefault();
			showSwitcher = !showSwitcher;
		}
	}

	// Close sidebar when navigating to a channel on mobile.
	$effect(() => {
		if (chat.activeChannelId) sidebarOpen = false;
	});
</script>

<svelte:window onkeydown={onGlobalKeydown} />

<div class="-m-8 flex h-[calc(100vh)] overflow-hidden">
	<!-- Mobile sidebar toggle button -->
	<button
		type="button"
		class="absolute top-2 left-2 z-20 rounded-md p-1.5 text-[var(--muted-foreground)] hover:bg-[var(--accent)] sm:hidden"
		onclick={() => (sidebarOpen = !sidebarOpen)}
		aria-label={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
	>
		{#if sidebarOpen}
			<X class="h-5 w-5" />
		{:else}
			<Menu class="h-5 w-5" />
		{/if}
	</button>

	<!-- Sidebar: always visible on sm+, toggled on mobile -->
	<div
		class="absolute inset-y-0 left-0 z-10 sm:relative sm:flex sm:h-full
		       {sidebarOpen ? 'flex' : 'hidden'}"
	>
		<ChannelSidebar
			channels={chat.channels}
			activeChannelId={chat.activeChannelId}
			onCreateChannel={() => (showNewChannel = true)}
		/>
	</div>

	<!-- Mobile backdrop -->
	{#if sidebarOpen}
		<div
			class="fixed inset-0 z-[5] bg-black/30 sm:hidden"
			role="presentation"
			onmousedown={() => (sidebarOpen = false)}
		></div>
	{/if}

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

{#if showSwitcher}
	<ChannelSwitcher channels={chat.channels} onClose={() => (showSwitcher = false)} />
{/if}
