<script lang="ts">
	import type { Channel, GitHubEventFilter } from '$lib/api/types';
	import { channels as channelsApi } from '$lib/api';
	import { chat } from '$lib/chat.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { X, GitBranch, Trash2 } from '@lucide/svelte';

	interface Props {
		channel: Channel;
		onClose: () => void;
	}
	const { channel, onClose }: Props = $props();

	const GITHUB_BOT_ID = '00000000-0000-0000-0000-000000000002';

	// The form is seeded from the channel once and then owned by the user — an
	// inbound channel update must not clobber what they're typing. Safe because
	// the call site keys this component on `channel.id`, so switching channels
	// remounts it with a fresh seed rather than leaving a stale form pointed at a
	// different channel.
	// svelte-ignore state_referenced_locally
	const seed = channel;
	let reposInput = $state((seed.github_repos ?? []).join(', '));
	let prEnabled = $state(seed.github_event_filter?.pr ?? false);
	let issueEnabled = $state(seed.github_event_filter?.issue ?? false);
	let reviewEnabled = $state(seed.github_event_filter?.review ?? false);
	let pushEnabled = $state(seed.github_event_filter?.push ?? false);
	let saving = $state(false);
	let error = $state<string | null>(null);

	function getMuted(): boolean {
		try {
			const muted: string[] = JSON.parse(localStorage.getItem('weld_muted_senders') ?? '[]');
			return muted.includes(GITHUB_BOT_ID);
		} catch {
			return false;
		}
	}
	let muted = $state(getMuted());

	let confirmArchive = $state('');
	let archiving = $state(false);

	async function archive() {
		if (confirmArchive !== channel.name) return;
		archiving = true;
		const ok = await chat.archiveChannel(channel.id);
		archiving = false;
		if (ok) {
			onClose();
			void goto(resolve('/chat'));
		} else {
			error = chat.error ?? 'Failed to archive channel.';
		}
	}

	function toggleMute() {
		try {
			const muted_list: string[] = JSON.parse(localStorage.getItem('weld_muted_senders') ?? '[]');
			const idx = muted_list.indexOf(GITHUB_BOT_ID);
			if (idx >= 0) {
				muted_list.splice(idx, 1);
			} else {
				muted_list.push(GITHUB_BOT_ID);
			}
			localStorage.setItem('weld_muted_senders', JSON.stringify(muted_list));
			muted = !muted;
		} catch {
			// ignore
		}
	}

	async function save() {
		saving = true;
		error = null;
		const repos = reposInput
			.split(',')
			.map((r) => r.trim())
			.filter(Boolean);
		const filter: GitHubEventFilter = {
			pr: prEnabled,
			issue: issueEnabled,
			review: reviewEnabled,
			push: pushEnabled
		};
		try {
			const updated = await channelsApi.update(channel.id, {
				github_repos: repos,
				github_event_filter: filter
			});
			chat.updateChannel(updated);
			onClose();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save settings';
		} finally {
			saving = false;
		}
	}
</script>

<div class="flex h-full w-72 shrink-0 flex-col border-l border-[var(--border)] bg-[var(--card)]">
	<header class="flex items-center justify-between border-b border-[var(--border)] px-4 py-3">
		<span class="text-sm font-semibold">Channel settings</span>
		<button
			type="button"
			class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)]"
			onclick={onClose}
			aria-label="Close settings"
		>
			<X class="h-4 w-4" />
		</button>
	</header>

	<div class="flex-1 space-y-6 overflow-y-auto px-4 py-4">
		<section>
			<h3
				class="mb-2 flex items-center gap-2 text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase"
			>
				<GitBranch class="h-3.5 w-3.5" />
				GitHub activity feed
			</h3>

			<label class="mb-1 block text-xs font-medium" for="github-repos"> Subscribed repos </label>
			<input
				id="github-repos"
				type="text"
				bind:value={reposInput}
				placeholder="owner/repo, owner/repo2"
				class="w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-xs outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			/>
			<p class="mt-1 text-[10px] text-[var(--muted-foreground)]">
				Comma-separated, e.g. <code>acme/api, acme/web</code>
			</p>

			<div class="mt-3 space-y-2">
				<p class="text-xs font-medium">Event types</p>
				<label class="flex cursor-pointer items-center gap-2 text-xs">
					<input type="checkbox" bind:checked={prEnabled} class="rounded" />
					Pull requests
				</label>
				<label class="flex cursor-pointer items-center gap-2 text-xs">
					<input type="checkbox" bind:checked={issueEnabled} class="rounded" />
					Issues
				</label>
				<label class="flex cursor-pointer items-center gap-2 text-xs">
					<input type="checkbox" bind:checked={reviewEnabled} class="rounded" />
					Reviews
				</label>
				<label class="flex cursor-pointer items-center gap-2 text-xs">
					<input type="checkbox" bind:checked={pushEnabled} class="rounded" />
					Pushes
				</label>
			</div>
		</section>

		<section>
			<h3 class="mb-2 text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase">
				GitHub bot
			</h3>
			<label class="flex cursor-pointer items-center gap-2 text-xs">
				<input type="checkbox" bind:checked={muted} onchange={toggleMute} class="rounded" />
				Mute GitHub activity messages
			</label>
			<p class="mt-1 text-[10px] text-[var(--muted-foreground)]">
				Hides all GitHub event cards in this browser session.
			</p>
		</section>

		{#if error}
			<p class="text-xs text-red-500">{error}</p>
		{/if}

		<section class="border-t border-[var(--border)] pt-4">
			<h3
				class="mb-2 flex items-center gap-2 text-xs font-semibold tracking-wide text-red-500 uppercase"
			>
				<Trash2 class="h-3.5 w-3.5" />
				Danger zone
			</h3>
			<p class="mb-2 text-[11px] text-[var(--muted-foreground)]">
				Archiving removes <code>#{channel.name}</code> from the sidebar. Message history is kept but not
				visible. Type the channel name to confirm.
			</p>
			<input
				type="text"
				bind:value={confirmArchive}
				placeholder={channel.name}
				class="mb-2 w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-xs outline-none focus-visible:ring-2 focus-visible:ring-red-500"
			/>
			<button
				type="button"
				onclick={archive}
				disabled={archiving || confirmArchive !== channel.name}
				class="w-full rounded-md border border-red-500 px-3 py-1.5 text-xs font-medium text-red-500 hover:bg-red-500 hover:text-white disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-red-500"
			>
				{archiving ? 'Archiving…' : 'Archive channel'}
			</button>
		</section>
	</div>

	<div class="border-t border-[var(--border)] px-4 py-3">
		<button
			type="button"
			class="w-full rounded-md bg-[var(--primary)] px-3 py-2 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
			onclick={save}
			disabled={saving}
		>
			{saving ? 'Saving…' : 'Save settings'}
		</button>
	</div>
</div>
