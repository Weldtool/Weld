<script lang="ts">
	import { chat } from '$lib/chat.svelte';
	import { auth } from '$lib/auth.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { SvelteSet } from 'svelte/reactivity';
	import { X, Search } from 'lucide-svelte';

	interface Props {
		onClose: () => void;
	}

	const { onClose }: Props = $props();

	let name = $state('');
	let topic = $state('');
	let memberSearch = $state('');
	let selectedMembers = $state(new SvelteSet<string>());
	let busy = $state(false);
	let err = $state<string | null>(null);

	// Exclude the current user — they're added automatically by the backend
	const otherMembers = $derived(chat.members.filter((m) => m.user_id !== auth.user?.id));

	const filteredMembers = $derived(
		memberSearch.trim()
			? otherMembers.filter(
					(m) =>
						m.display_name.toLowerCase().includes(memberSearch.toLowerCase()) ||
						m.email.toLowerCase().includes(memberSearch.toLowerCase())
				)
			: otherMembers
	);

	function toggleMember(userId: string) {
		if (selectedMembers.has(userId)) selectedMembers.delete(userId);
		else selectedMembers.add(userId);
	}

	function toggleAll() {
		if (selectedMembers.size === otherMembers.length) {
			selectedMembers.clear();
		} else {
			selectedMembers.clear();
			for (const m of otherMembers) selectedMembers.add(m.user_id);
		}
	}

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
				members: [...selectedMembers]
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
		class="flex w-full max-w-md flex-col rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-xl"
	>
		<!-- Header -->
		<div class="flex items-center justify-between border-b border-[var(--border)] px-5 py-4">
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

		<form onsubmit={submit} class="flex flex-col gap-0">
			<div class="space-y-3 px-5 py-4">
				<!-- Name -->
				<label class="block text-sm">
					<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Name</span>
					<input
						type="text"
						bind:value={name}
						placeholder="design-reviews"
						required
						autofocus
						class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					/>
					<span class="mt-1 block text-[10px] text-[var(--muted-foreground)]">
						Lowercase, dashes/underscores only.
					</span>
				</label>

				<!-- Topic -->
				<label class="block text-sm">
					<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Topic (optional)</span>
					<input
						type="text"
						bind:value={topic}
						class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					/>
				</label>
			</div>

			<!-- Members section -->
			<div class="border-t border-[var(--border)]">
				<div class="flex items-center justify-between px-5 py-3">
					<span class="text-sm font-medium">
						Members
						{#if selectedMembers.size > 0}
							<span class="ml-1 text-xs text-[var(--muted-foreground)]">
								({selectedMembers.size} selected)
							</span>
						{/if}
					</span>
					{#if otherMembers.length > 0}
						<button
							type="button"
							onclick={toggleAll}
							class="text-xs text-[var(--primary)] hover:underline"
						>
							{selectedMembers.size === otherMembers.length ? 'Deselect all' : 'Select all'}
						</button>
					{/if}
				</div>

				{#if otherMembers.length === 0}
					<p class="px-5 pb-3 text-sm text-[var(--muted-foreground)]">
						No other team members to add.
					</p>
				{:else}
					<!-- Search -->
					{#if otherMembers.length > 5}
						<div class="relative mx-5 mb-2">
							<Search
								class="absolute top-1/2 left-2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--muted-foreground)]"
							/>
							<input
								type="search"
								placeholder="Search members…"
								bind:value={memberSearch}
								class="w-full rounded-md border border-[var(--input)] bg-[var(--background)] py-1.5 pr-3 pl-7 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
							/>
						</div>
					{/if}

					<!-- Member list -->
					<div class="max-h-48 overflow-y-auto px-2 pb-2">
						{#if filteredMembers.length === 0}
							<p class="px-3 py-2 text-sm text-[var(--muted-foreground)]">No members match.</p>
						{/if}
						{#each filteredMembers as member (member.user_id)}
							<label
								class="flex cursor-pointer items-center gap-3 rounded-md px-3 py-2 hover:bg-[var(--accent)]"
							>
								<input
									type="checkbox"
									checked={selectedMembers.has(member.user_id)}
									onchange={() => toggleMember(member.user_id)}
									class="h-4 w-4 rounded accent-[var(--primary)]"
								/>
								<span
									class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--muted)] text-xs font-semibold"
								>
									{member.display_name.charAt(0).toUpperCase()}
								</span>
								<div class="min-w-0 flex-1">
									<div class="truncate text-sm font-medium">{member.display_name}</div>
									<div class="truncate text-xs text-[var(--muted-foreground)]">{member.email}</div>
								</div>
							</label>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Footer -->
			<div class="flex items-center justify-between border-t border-[var(--border)] px-5 py-4">
				{#if err}
					<p class="text-xs text-red-500">{err}</p>
				{:else}
					<span class="text-xs text-[var(--muted-foreground)]">
						You'll be added automatically.
					</span>
				{/if}
				<div class="flex gap-2">
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
			</div>
		</form>
	</div>
</div>
