<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { Channel } from '$lib/api/types';
	import { chat } from '$lib/chat.svelte';
	import { cn } from '$lib/utils';
	import { Hash, Lock, Plus, MessageSquare } from 'lucide-svelte';

	interface Props {
		channels: Channel[];
		activeChannelId: string | null;
		onCreateChannel: () => void;
	}

	const { channels, activeChannelId, onCreateChannel }: Props = $props();

	const grouped = $derived.by(() => {
		const dms: Channel[] = [];
		const projectChans: Channel[] = [];
		const teamChans: Channel[] = [];
		for (const c of channels) {
			if (c.is_direct) dms.push(c);
			else if (c.project_id) projectChans.push(c);
			else teamChans.push(c);
		}
		return { dms, projectChans, teamChans };
	});

	function go(id: string) {
		void goto(resolve(`/chat/${id}` as '/chat'));
	}

	function unread(id: string): number {
		return chat.unreadByChannel[id] ?? 0;
	}
</script>

<aside class="flex h-full w-60 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--card)]">
	<div class="flex items-center justify-between border-b border-[var(--border)] px-3 py-2.5">
		<span class="text-sm font-semibold">Channels</span>
		<button
			type="button"
			class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
			onclick={onCreateChannel}
			title="New channel"
			aria-label="New channel"
		>
			<Plus class="h-4 w-4" />
		</button>
	</div>

	<div class="flex-1 overflow-y-auto px-2 py-2 text-sm">
		{#snippet section(label: string, list: Channel[])}
			{#if list.length > 0}
				<div class="mb-3">
					<div
						class="px-2 text-[10px] font-semibold tracking-wide text-[var(--muted-foreground)] uppercase"
					>
						{label}
					</div>
					<ul class="mt-1 space-y-0.5">
						{#each list as c (c.id)}
							{@const u = unread(c.id)}
							<li>
								<button
									type="button"
									onclick={() => go(c.id)}
									class={cn(
										'flex w-full items-center justify-between gap-2 rounded px-2 py-1 text-left',
										c.id === activeChannelId
											? 'bg-[var(--accent)] text-[var(--accent-foreground)]'
											: 'text-[var(--muted-foreground)] hover:bg-[var(--accent)]/50 hover:text-[var(--foreground)]'
									)}
								>
									<span class="flex min-w-0 items-center gap-1.5 truncate">
										{#if c.is_direct}
											<MessageSquare class="h-3.5 w-3.5 shrink-0" />
										{:else if c.is_private}
											<Lock class="h-3.5 w-3.5 shrink-0" />
										{:else}
											<Hash class="h-3.5 w-3.5 shrink-0" />
										{/if}
										<span class="truncate {u > 0 ? 'font-semibold text-[var(--foreground)]' : ''}">
											{c.name}
										</span>
									</span>
									{#if u > 0}
										<span
											class="rounded-full bg-[var(--primary)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--primary-foreground)]"
										>
											{u}
										</span>
									{/if}
								</button>
							</li>
						{/each}
					</ul>
				</div>
			{/if}
		{/snippet}

		{@render section('Channels', grouped.teamChans)}
		{@render section('Projects', grouped.projectChans)}
		{@render section('Direct messages', grouped.dms)}

		{#if channels.length === 0}
			<div class="px-2 py-4 text-center text-xs text-[var(--muted-foreground)]">
				No channels yet.
				<button
					type="button"
					class="mt-2 block w-full rounded-md border border-dashed border-[var(--border)] px-2 py-2 hover:border-[var(--ring)]"
					onclick={onCreateChannel}
				>
					Create your first channel
				</button>
			</div>
		{/if}
	</div>
</aside>
