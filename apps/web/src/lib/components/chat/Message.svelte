<script lang="ts">
	import type { TeamMember } from '$lib/api/types';
	import { renderMessageMarkdown } from '$lib/chat-helpers';
	import type { PendingMessage } from '$lib/chat.svelte';
	import { isPending, chat } from '$lib/chat.svelte';
	import { cn } from '$lib/utils';
	import type { ChatMessage } from '$lib/api/types';
	import { MessageSquare, Pencil, Trash2, RotateCw, X } from 'lucide-svelte';

	interface Props {
		message: ChatMessage | PendingMessage;
		members: TeamMember[];
		selfId: string | null;
		showAvatar?: boolean;
		onReply?: (parentId: string) => void;
		onEdit?: (id: string, content: string) => void;
	}

	const { message, members, selfId, showAvatar = true, onReply, onEdit }: Props = $props();
	const canDelete = $derived(!isPending(message) && message.sender_id === selfId);

	const sender = $derived(members.find((m) => m.user_id === message.sender_id) ?? null);
	const senderName = $derived(sender?.display_name ?? (isPending(message) ? 'You' : 'Unknown'));
	const html = $derived(renderMessageMarkdown(message.content, { members, selfId }));
	const replyCount = $derived(chat.threadReplyCount(message.id));
	const time = $derived(formatTime(message.created_at));

	let editing = $state(false);
	let draft = $state('');

	function startEdit() {
		draft = message.content;
		editing = true;
	}
	function cancelEdit() {
		editing = false;
	}
	function saveEdit() {
		const next = draft.trim();
		if (!next || next === message.content) {
			editing = false;
			return;
		}
		onEdit?.(message.id, next);
		editing = false;
	}

	function formatTime(iso: string): string {
		const d = new Date(iso);
		return d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
	}

	function initials(name: string): string {
		return name
			.split(/\s+/)
			.map((w) => w[0])
			.filter(Boolean)
			.slice(0, 2)
			.join('')
			.toUpperCase();
	}

	const pending = $derived(isPending(message));
	const failed = $derived(pending && (message as PendingMessage).failed === true);
</script>

<div
	class={cn(
		'group relative flex gap-3 rounded-md px-3 py-1.5 hover:bg-[var(--accent)]/40',
		pending && 'opacity-60'
	)}
>
	<div class="w-8 shrink-0">
		{#if showAvatar}
			<div
				class="flex h-8 w-8 items-center justify-center rounded-full bg-[var(--muted)] text-xs font-medium text-[var(--muted-foreground)]"
				aria-hidden="true"
			>
				{initials(senderName)}
			</div>
		{/if}
	</div>

	<div class="min-w-0 flex-1">
		{#if showAvatar}
			<div class="flex items-baseline gap-2">
				<span class="text-sm font-semibold">{senderName}</span>
				<span class="text-[10px] text-[var(--muted-foreground)]">{time}</span>
				{#if message.edited_at}
					<span class="text-[10px] text-[var(--muted-foreground)] italic">(edited)</span>
				{/if}
			</div>
		{/if}

		{#if editing}
			<div class="mt-1 space-y-1">
				<textarea
					bind:value={draft}
					rows={3}
					class="w-full resize-y rounded-md border border-[var(--input)] bg-[var(--background)] p-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				></textarea>
				<div class="flex gap-2 text-xs">
					<button
						type="button"
						class="rounded-md bg-[var(--primary)] px-2 py-1 text-[var(--primary-foreground)] hover:opacity-90"
						onclick={saveEdit}
					>
						Save
					</button>
					<button
						type="button"
						class="rounded-md border border-[var(--border)] px-2 py-1 hover:bg-[var(--accent)]"
						onclick={cancelEdit}
					>
						Cancel
					</button>
				</div>
			</div>
		{:else}
			<div class="chat-message-body text-sm leading-snug">
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				{@html html}
			</div>
		{/if}

		{#if !editing && replyCount > 0}
			<button
				type="button"
				class="mt-1 inline-flex items-center gap-1 text-xs text-[var(--primary)] hover:underline"
				onclick={() => onReply?.(message.id)}
			>
				<MessageSquare class="h-3 w-3" />
				{replyCount}
				{replyCount === 1 ? 'reply' : 'replies'}
			</button>
		{/if}

		{#if failed}
			<div class="mt-1 flex items-center gap-2 text-xs text-red-500">
				<span>Failed to send.</span>
				<button
					type="button"
					class="inline-flex items-center gap-1 hover:underline"
					onclick={() =>
						chat.retryPending(message.channel_id, (message as PendingMessage).clientId)}
				>
					<RotateCw class="h-3 w-3" /> Retry
				</button>
				<button
					type="button"
					class="inline-flex items-center gap-1 hover:underline"
					onclick={() =>
						chat.discardPending(message.channel_id, (message as PendingMessage).clientId)}
				>
					<X class="h-3 w-3" /> Discard
				</button>
			</div>
		{:else if pending}
			<div class="mt-1 text-[10px] text-[var(--muted-foreground)] italic">Sending…</div>
		{/if}
	</div>

	{#if !pending && !editing}
		<div
			class="absolute -top-3 right-3 hidden gap-1 rounded-md border border-[var(--border)] bg-[var(--card)] px-1 py-0.5 shadow-sm group-hover:flex"
		>
			{#if onReply && !message.thread_parent_id}
				<button
					type="button"
					class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
					title="Reply in thread"
					onclick={() => onReply?.(message.id)}
				>
					<MessageSquare class="h-3.5 w-3.5" />
				</button>
			{/if}
			{#if message.sender_id === selfId}
				<button
					type="button"
					class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
					title="Edit"
					onclick={startEdit}
				>
					<Pencil class="h-3.5 w-3.5" />
				</button>
			{/if}
			{#if canDelete}
				<button
					type="button"
					class="rounded p-1 text-[var(--muted-foreground)] hover:bg-red-500/20 hover:text-red-500"
					title="Delete"
					onclick={() => chat.deleteMessage(message.id, message.channel_id)}
				>
					<Trash2 class="h-3.5 w-3.5" />
				</button>
			{/if}
		</div>
	{/if}
</div>

<style>
	:global(.chat-message-body p) {
		margin: 0.15rem 0;
	}
	:global(.chat-message-body p:first-child) {
		margin-top: 0;
	}
	:global(.chat-message-body code) {
		background: var(--muted);
		padding: 0.05rem 0.3rem;
		border-radius: 0.25rem;
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-size: 0.85em;
	}
	:global(.chat-message-body pre) {
		background: var(--muted);
		padding: 0.5rem 0.75rem;
		border-radius: 0.375rem;
		margin: 0.35rem 0;
		overflow-x: auto;
	}
	:global(.chat-message-body pre code) {
		background: transparent;
		padding: 0;
	}
	:global(.chat-message-body a) {
		color: var(--primary);
		text-decoration: underline;
	}
	:global(.chat-mention) {
		display: inline-block;
		padding: 0 0.3rem;
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--primary) 18%, transparent);
		color: var(--primary);
		font-weight: 500;
	}
	:global(.chat-mention-self) {
		background: color-mix(in srgb, var(--primary) 35%, transparent);
		color: var(--primary-foreground, var(--foreground));
	}
</style>
