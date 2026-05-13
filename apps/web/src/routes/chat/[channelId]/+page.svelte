<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { chat } from '$lib/chat.svelte';
	import ChannelSettingsPanel from '$lib/components/chat/ChannelSettingsPanel.svelte';
	import Composer from '$lib/components/chat/Composer.svelte';
	import MessageList from '$lib/components/chat/MessageList.svelte';
	import ThreadPanel from '$lib/components/chat/ThreadPanel.svelte';
	import TypingIndicator from '$lib/components/chat/TypingIndicator.svelte';
	import { Hash, Lock, MessageSquare, Settings } from 'lucide-svelte';

	let showSettings = $state(false);

	const channelId = $derived(page.params.channelId ?? null);
	const threadId = $derived(page.url.searchParams.get('thread'));
	const channel = $derived(
		channelId ? (chat.channels.find((c) => c.id === channelId) ?? null) : null
	);
	const selfId = $derived(auth.user?.id ?? null);

	$effect(() => {
		if (!channelId) return;
		void chat.openChannel(channelId);
	});

	const typingIds = $derived(channelId ? chat.typingUsersFor(channelId) : []);

	function openThread(parentId: string) {
		if (!channelId) return;
		void goto(resolve(`/chat/${channelId}?thread=${encodeURIComponent(parentId)}` as '/chat'), {
			replaceState: false,
			keepFocus: true,
			noScroll: true
		});
	}

	function closeThread() {
		if (!channelId) return;
		void goto(resolve(`/chat/${channelId}` as '/chat'), {
			replaceState: false,
			keepFocus: true,
			noScroll: true
		});
		if (threadId) chat.closeThread(threadId);
	}
</script>

<svelte:head>
	<title>{channel ? '#' + channel.name : 'Chat'} · Weld</title>
</svelte:head>

<div class="flex h-full min-h-0 flex-1">
	<div class="flex min-w-0 flex-1 flex-col">
		{#if channel}
			<header
				class="flex shrink-0 items-center gap-2 border-b border-[var(--border)] bg-[var(--background)] px-4 py-3"
			>
				{#if channel.is_direct}
					<MessageSquare class="h-4 w-4 text-[var(--muted-foreground)]" />
				{:else if channel.is_private}
					<Lock class="h-4 w-4 text-[var(--muted-foreground)]" />
				{:else}
					<Hash class="h-4 w-4 text-[var(--muted-foreground)]" />
				{/if}
				<span class="font-semibold">{channel.name}</span>
				{#if channel.topic}
					<span class="ml-3 truncate text-xs text-[var(--muted-foreground)]">{channel.topic}</span>
				{/if}
				<button
					type="button"
					class="ml-auto rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
					title="Channel settings"
					aria-label="Channel settings"
					onclick={() => (showSettings = !showSettings)}
				>
					<Settings class="h-4 w-4" />
				</button>
			</header>

			<MessageList channelId={channel.id} members={chat.members} {selfId} onReply={openThread} />

			<TypingIndicator userIds={typingIds} members={chat.members} {selfId} />

			<Composer
				channelId={channel.id}
				members={chat.members}
				placeholder={'Message #' + channel.name}
			/>
		{:else}
			<div class="flex flex-1 items-center justify-center text-sm text-[var(--muted-foreground)]">
				{#if chat.activeTeamId}
					Channel not found.
					<button type="button" class="ml-2 underline" onclick={() => goto(resolve('/chat'))}>
						Back to chat
					</button>
				{:else}
					Loading…
				{/if}
			</div>
		{/if}
	</div>

	{#if showSettings && channel}
		<ChannelSettingsPanel {channel} onClose={() => (showSettings = false)} />
	{:else if threadId && channel}
		<ThreadPanel
			channelId={channel.id}
			parentId={threadId}
			members={chat.members}
			{selfId}
			onClose={closeThread}
		/>
	{/if}
</div>
