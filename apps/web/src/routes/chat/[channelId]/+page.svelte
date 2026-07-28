<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { chat } from '$lib/chat.svelte';
	import ChannelSettingsPanel from '$lib/components/chat/ChannelSettingsPanel.svelte';
	import Composer from '$lib/components/chat/Composer.svelte';
	import MessageDraft from '$lib/components/chat/MessageDraft.svelte';
	import MessageList from '$lib/components/chat/MessageList.svelte';
	import ThreadPanel from '$lib/components/chat/ThreadPanel.svelte';
	import TypingIndicator from '$lib/components/chat/TypingIndicator.svelte';
	import { Hash, Lock, MessageSquare, Settings } from '@lucide/svelte';

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
				class="flex shrink-0 items-center gap-3 border-b border-[var(--border)] bg-[var(--card)] px-5 py-3.5"
			>
				{#if channel.is_direct}
					<MessageSquare class="h-4 w-4 shrink-0 text-[var(--muted-foreground)]" />
				{:else if channel.is_private}
					<Lock class="h-4 w-4 shrink-0 text-[var(--muted-foreground)]" />
				{:else}
					<Hash class="h-4 w-4 shrink-0 text-[var(--muted-foreground)]" />
				{/if}
				<span class="text-base font-semibold tracking-tight">{channel.name}</span>
				{#if channel.topic}
					<span class="text-[var(--muted-foreground)]">·</span>
					<span class="min-w-0 truncate text-sm text-[var(--muted-foreground)]"
						>{channel.topic}</span
					>
				{/if}
				<button
					type="button"
					class="ml-auto rounded-md p-1.5 text-[var(--muted-foreground)] transition-colors hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
					title="Channel settings"
					aria-label="Channel settings"
					onclick={() => (showSettings = !showSettings)}
				>
					<Settings class="h-4 w-4" />
				</button>
			</header>

			<MessageList channelId={channel.id} members={chat.members} {selfId} onReply={openThread} />

			<TypingIndicator userIds={typingIds} members={chat.members} {selfId} />

			<MessageDraft channelId={channel.id} />

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
		<!-- Keyed on the channel: the panel seeds its form from `channel` once, but
		     `showSettings` survives a channel switch. Without the remount you could
		     open #general's settings, click through to #pettracker, hit Save, and
		     write #general's repo subscriptions onto #pettracker. -->
		{#key channel.id}
			<ChannelSettingsPanel {channel} onClose={() => (showSettings = false)} />
		{/key}
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
