<script lang="ts">
	import { chat } from '$lib/chat.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { X } from 'lucide-svelte';

	interface Props {
		onClose: () => void;
	}

	const { onClose }: Props = $props();

	let name = $state('');
	let topic = $state('');
	let isPrivate = $state(false);
	let busy = $state(false);
	let err = $state<string | null>(null);

	async function submit(e: Event) {
		e.preventDefault();
		const slug = name
			.trim()
			.toLowerCase()
			.replace(/\s+/g, '-')
			.replace(/[^a-z0-9_-]/g, '');
		if (!slug) {
			err = 'Channel name required.';
			return;
		}
		busy = true;
		err = null;
		try {
			const created = await chat.createChannel({
				name: slug,
				topic: topic.trim() || undefined,
				is_private: isPrivate
			});
			if (!created) {
				err = chat.error ?? 'Failed to create channel.';
				return;
			}
			onClose();
			void goto(resolve(`/chat/${created.id}` as '/chat'));
		} finally {
			busy = false;
		}
	}
</script>

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
	role="dialog"
	aria-modal="true"
	aria-labelledby="new-channel-title"
>
	<div
		class="w-full max-w-md rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-xl"
	>
		<div class="mb-3 flex items-center justify-between">
			<h2 id="new-channel-title" class="text-base font-semibold">Create channel</h2>
			<button
				type="button"
				class="rounded p-1 hover:bg-[var(--accent)]"
				onclick={onClose}
				aria-label="Close"
			>
				<X class="h-4 w-4" />
			</button>
		</div>

		<form onsubmit={submit} class="space-y-3">
			<label class="block text-sm">
				<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Name</span>
				<input
					type="text"
					bind:value={name}
					placeholder="design-reviews"
					required
					class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
				<span class="mt-1 block text-[10px] text-[var(--muted-foreground)]">
					Lowercase, dashes/underscores only.
				</span>
			</label>

			<label class="block text-sm">
				<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Topic (optional)</span>
				<input
					type="text"
					bind:value={topic}
					class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
			</label>

			<label class="flex items-center gap-2 text-sm">
				<input type="checkbox" bind:checked={isPrivate} />
				<span>Private (invitation only)</span>
			</label>

			{#if err}
				<p class="text-xs text-red-500">{err}</p>
			{/if}

			<div class="flex justify-end gap-2 pt-1">
				<button
					type="button"
					onclick={onClose}
					class="rounded-md border border-[var(--border)] px-3 py-1.5 text-sm hover:bg-[var(--accent)]"
				>
					Cancel
				</button>
				<button
					type="submit"
					disabled={busy}
					class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
				>
					{busy ? 'Creating…' : 'Create'}
				</button>
			</div>
		</form>
	</div>
</div>
