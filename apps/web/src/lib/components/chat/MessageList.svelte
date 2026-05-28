<script lang="ts">
	import { tick } from 'svelte';
	import type { TeamMember } from '$lib/api/types';
	import type { PendingMessage } from '$lib/chat.svelte';
	import { chat } from '$lib/chat.svelte';
	import type { ChatMessage } from '$lib/api/types';
	import Message from './Message.svelte';

	interface Props {
		channelId: string;
		members: TeamMember[];
		selfId: string | null;
		onReply: (id: string) => void;
	}

	const { channelId, members, selfId, onReply }: Props = $props();

	let scrollEl: HTMLDivElement | null = $state(null);
	let loadingOlder = $state(false);
	let pinnedToBottom = $state(true);
	let lastMessageId: string | null = null;

	const channelState = $derived(chat.messagesByChannel[channelId]);
	const messages = $derived<(ChatMessage | PendingMessage)[]>(
		(channelState?.messages ?? []).filter((m) => m.thread_parent_id === null)
	);
	const hasMore = $derived(channelState ? !channelState.loadedAll : true);
	const initialLoading = $derived(chat.loadingChannel === channelId);
	const loadError = $derived(channelState?.loadError ?? null);

	$effect(() => {
		// new message arrived: stick to bottom if user was already there
		const newest = messages[messages.length - 1];
		const newestId = newest?.id ?? null;
		if (newestId !== lastMessageId) {
			lastMessageId = newestId;
			if (pinnedToBottom) void tick().then(scrollToBottom);
		}
	});

	function scrollToBottom() {
		if (!scrollEl) return;
		scrollEl.scrollTop = scrollEl.scrollHeight;
	}

	async function onScroll() {
		if (!scrollEl) return;
		const distanceToBottom = scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight;
		pinnedToBottom = distanceToBottom < 80;

		if (scrollEl.scrollTop < 60 && hasMore && !loadingOlder) {
			loadingOlder = true;
			const prevHeight = scrollEl.scrollHeight;
			await chat.loadOlder(channelId);
			await tick();
			if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight - prevHeight;
			loadingOlder = false;
		}
	}

	function showAvatarFor(idx: number): boolean {
		if (idx === 0) return true;
		const cur = messages[idx];
		const prev = messages[idx - 1];
		if (cur.sender_id !== prev.sender_id) return true;
		const dt = new Date(cur.created_at).getTime() - new Date(prev.created_at).getTime();
		return dt > 5 * 60 * 1000;
	}

	function dayLabel(idx: number): string | null {
		const cur = messages[idx];
		const prev = idx > 0 ? messages[idx - 1] : null;
		const curDay = new Date(cur.created_at).toDateString();
		if (!prev) return curDay;
		const prevDay = new Date(prev.created_at).toDateString();
		return curDay === prevDay ? null : curDay;
	}
</script>

<div
	bind:this={scrollEl}
	onscroll={onScroll}
	class="flex-1 overflow-y-auto px-2 py-2"
	role="log"
	aria-live="polite"
	aria-relevant="additions"
>
	{#if initialLoading}
		<div
			class="flex flex-1 items-center justify-center py-8 text-xs text-[var(--muted-foreground)]"
		>
			Loading messages…
		</div>
	{:else if loadError}
		<div
			class="flex flex-1 flex-col items-center justify-center gap-2 py-8 text-center text-xs text-red-500"
		>
			<span>Failed to load messages: {loadError}</span>
			<button
				type="button"
				class="underline hover:no-underline"
				onclick={() => chat.openChannel(channelId)}
			>
				Retry
			</button>
		</div>
	{:else if loadingOlder}
		<div class="py-2 text-center text-xs text-[var(--muted-foreground)]">
			Loading older messages…
		</div>
	{:else if !hasMore && messages.length > 0}
		<div class="py-2 text-center text-[10px] text-[var(--muted-foreground)]">
			You've reached the start of the channel.
		</div>
	{/if}

	{#if !initialLoading && !loadError}
		{#each messages as m, i (m.id)}
			{@const day = dayLabel(i)}
			{#if day}
				<div class="my-2 flex items-center gap-2 text-[10px] text-[var(--muted-foreground)]">
					<div class="h-px flex-1 bg-[var(--border)]"></div>
					<span>{day}</span>
					<div class="h-px flex-1 bg-[var(--border)]"></div>
				</div>
			{/if}
			<Message message={m} {members} {selfId} showAvatar={showAvatarFor(i)} {onReply} />
		{/each}

		{#if messages.length === 0 && !loadingOlder}
			<div class="flex h-full items-center justify-center text-sm text-[var(--muted-foreground)]">
				No messages yet. Say hi!
			</div>
		{/if}
	{/if}
</div>
