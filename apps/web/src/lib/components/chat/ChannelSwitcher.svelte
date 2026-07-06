<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { Channel } from '$lib/api/types';
	import { cn } from '$lib/utils';
	import { Hash, Lock, MessageSquare } from '@lucide/svelte';

	interface Props {
		channels: Channel[];
		onClose: () => void;
	}

	const { channels, onClose }: Props = $props();

	let query = $state('');
	let highlight = $state(0);
	let inputEl: HTMLInputElement | null = $state(null);

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (!q) return channels.slice(0, 10);
		return channels.filter((c) => c.name.toLowerCase().includes(q)).slice(0, 10);
	});

	$effect(() => {
		highlight = 0;
	});

	$effect(() => {
		inputEl?.focus();
	});

	function select(channel: Channel) {
		onClose();
		void goto(resolve(`/chat/${channel.id}` as '/chat'));
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
			return;
		}
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			highlight = (highlight + 1) % Math.max(filtered.length, 1);
			return;
		}
		if (e.key === 'ArrowUp') {
			e.preventDefault();
			highlight = (highlight - 1 + Math.max(filtered.length, 1)) % Math.max(filtered.length, 1);
			return;
		}
		if (e.key === 'Enter' && filtered[highlight]) {
			select(filtered[highlight]);
		}
	}

	function onBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) onClose();
	}
</script>

<div
	class="fixed inset-0 z-50 flex items-start justify-center bg-black/40 p-4 pt-24"
	role="dialog"
	tabindex="-1"
	aria-modal="true"
	aria-label="Switch channel"
	onmousedown={onBackdropClick}
>
	<div
		class="w-full max-w-md overflow-hidden rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-xl"
	>
		<input
			bind:this={inputEl}
			bind:value={query}
			onkeydown={onKeydown}
			type="text"
			placeholder="Jump to channel…"
			class="block w-full border-b border-[var(--border)] bg-[var(--background)] px-4 py-3 text-sm outline-none"
			aria-label="Search channels"
		/>
		<ul role="listbox" class="max-h-72 overflow-y-auto py-1">
			{#each filtered as c, i (c.id)}
				<li>
					<button
						type="button"
						role="option"
						aria-selected={i === highlight}
						class={cn(
							'flex w-full items-center gap-2 px-3 py-2 text-left text-sm',
							i === highlight
								? 'bg-[var(--accent)] text-[var(--accent-foreground)]'
								: 'hover:bg-[var(--accent)]/60'
						)}
						onmouseenter={() => (highlight = i)}
						onclick={() => select(c)}
					>
						{#if c.is_direct}
							<MessageSquare class="h-3.5 w-3.5 shrink-0 text-[var(--muted-foreground)]" />
						{:else if c.is_private}
							<Lock class="h-3.5 w-3.5 shrink-0 text-[var(--muted-foreground)]" />
						{:else}
							<Hash class="h-3.5 w-3.5 shrink-0 text-[var(--muted-foreground)]" />
						{/if}
						<span class="truncate">{c.name}</span>
					</button>
				</li>
			{/each}
			{#if filtered.length === 0}
				<li class="px-4 py-3 text-sm text-[var(--muted-foreground)]">No matching channels.</li>
			{/if}
		</ul>
	</div>
</div>
