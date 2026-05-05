<script lang="ts">
	import type { TeamMember } from '$lib/api/types';
	import { chat } from '$lib/chat.svelte';
	import { X } from 'lucide-svelte';
	import Composer from './Composer.svelte';
	import Message from './Message.svelte';

	interface Props {
		channelId: string;
		parentId: string;
		members: TeamMember[];
		selfId: string | null;
		onClose: () => void;
	}

	const { channelId, parentId, members, selfId, onClose }: Props = $props();

	$effect(() => {
		void chat.openThread(parentId);
	});

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}

	const state = $derived(chat.threadByParent[parentId]);
</script>

<svelte:window onkeydown={onKeydown} />

<aside
	class="flex h-full w-96 shrink-0 flex-col border-l border-[var(--border)] bg-[var(--card)]"
	aria-label="Thread"
>
	<div class="flex items-center justify-between border-b border-[var(--border)] px-3 py-2.5">
		<span class="text-sm font-semibold">Thread</span>
		<button
			type="button"
			class="rounded p-1 hover:bg-[var(--accent)]"
			onclick={onClose}
			aria-label="Close thread"
		>
			<X class="h-4 w-4" />
		</button>
	</div>

	<div class="flex-1 overflow-y-auto px-2 py-2">
		{#if state?.loading && !state.parent}
			<div class="py-4 text-center text-xs text-[var(--muted-foreground)]">Loading thread…</div>
		{:else if state?.error}
			<div class="py-4 text-center text-xs text-red-500">{state.error}</div>
		{:else if state?.parent}
			<Message message={state.parent} {members} {selfId} />
			<div class="my-2 px-3 text-[10px] tracking-wide text-[var(--muted-foreground)] uppercase">
				{state.replies.length}
				{state.replies.length === 1 ? 'reply' : 'replies'}
			</div>
			{#each state.replies as r (r.id)}
				<Message message={r} {members} {selfId} />
			{/each}
		{/if}
	</div>

	<Composer {channelId} threadParentId={parentId} {members} placeholder="Reply…" />
</aside>
