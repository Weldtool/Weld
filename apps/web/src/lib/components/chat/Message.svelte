<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { TeamMember, GitHubMessageMetadata } from '$lib/api/types';
	import { renderMessageMarkdown } from '$lib/chat-helpers';
	import type { PendingMessage } from '$lib/chat.svelte';
	import { isPending, chat } from '$lib/chat.svelte';
	import { telemetry } from '$lib/telemetry.svelte';
	import { cn } from '$lib/utils';
	import type { ChatMessage } from '$lib/api/types';
	import { MessageSquare, Pencil, Trash2, RotateCw, X } from '@lucide/svelte';
	import GitHubCard from './GitHubCard.svelte';

	const GITHUB_BOT_ID = '00000000-0000-0000-0000-000000000002';

	interface Props {
		message: ChatMessage | PendingMessage;
		members: TeamMember[];
		selfId: string | null;
		showAvatar?: boolean;
		onReply?: (parentId: string) => void;
	}

	const { message, members, selfId, showAvatar = true, onReply }: Props = $props();
	const canDelete = $derived(!isPending(message) && message.sender_id === selfId);

	const sender = $derived(members.find((m) => m.user_id === message.sender_id) ?? null);
	const senderName = $derived(sender?.display_name ?? (isPending(message) ? 'You' : 'Unknown'));
	const html = $derived(
		renderMessageMarkdown(message.content, { members, selfId, taskRefs: chat.taskRefs })
	);
	const replyCount = $derived(chat.threadReplyCount(message.id));
	const time = $derived(formatTime(message.created_at));

	let editing = $state(false);
	let draft = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	function startEdit() {
		draft = message.content;
		editError = null;
		editing = true;
	}
	function cancelEdit() {
		editing = false;
		editError = null;
	}
	// The textarea is only mounted while `editing`, so this runs exactly when the
	// editor opens. Caret goes to the end rather than selecting all, so typing
	// appends instead of wiping the message (Slack/Discord behaviour).
	function focusEditor(node: HTMLTextAreaElement) {
		node.focus();
		node.setSelectionRange(node.value.length, node.value.length);
	}
	async function saveEdit() {
		const next = draft.trim();
		if (!next || next === message.content) {
			editing = false;
			editError = null;
			return;
		}
		saving = true;
		editError = null;
		try {
			await chat.editMessage(message.id, next);
			editing = false;
		} catch (err) {
			// Keep the editor open so the draft isn't lost on a failed save.
			editError = err instanceof Error ? err.message : 'Failed to save edit.';
		} finally {
			saving = false;
		}
	}

	// Open a task-detail deep link (from a card/task/PR chat reference) as a modal
	// over the current route instead of following the `/board?task=…` href, which
	// would yank the participant off the chat screen. The global TaskDetailOverlay
	// picks up the `?task=`/`?project=` params.
	function openTaskInPlace(link: HTMLAnchorElement, e: Event) {
		const href = new URL(link.href, window.location.origin);
		const task = href.searchParams.get('task') ?? link.getAttribute('data-card-ref');
		if (!task) return;
		e.preventDefault();
		const url = new URL(page.url);
		url.searchParams.set('task', task);
		const project = href.searchParams.get('project');
		if (project) url.searchParams.set('project', project);
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- stay on the current chat route; only the query string changes
		void goto(url.pathname + url.search, { keepFocus: true, noScroll: true });
	}

	// Delegate clicks on rendered card/task cross-reference links so the study
	// can record a `card_referenced_from_chat` event (headline integration
	// metric). Inert outside study mode. An action avoids a11y warnings that an
	// inline click handler on the (non-interactive) body div would trigger.
	function trackMessageInteractions(node: HTMLElement) {
		const handler = (e: Event) => {
			const target = e.target as HTMLElement;
			const msgId = isPending(message) ? null : message.id;
			const chanId = message.channel_id;

			// PR cross-references → pr_referenced_from_chat. The bare badge (no
			// linked task) is a <span>, not an <a>, so it simply won't match here.
			const prLink = target.closest('a.chat-prref') as HTMLAnchorElement | null;
			if (prLink) {
				telemetry.log('pr_referenced_from_chat', {
					pr_number: prLink.getAttribute('data-pr-number'),
					task_id: new URL(prLink.href, window.location.origin).searchParams.get('task'),
					message_id: msgId,
					channel_id: chanId
				});
				openTaskInPlace(prLink, e);
				return;
			}

			// Card / task cross-references → card_referenced_from_chat
			const cardLink = target.closest('.chat-cardref, .chat-taskref') as HTMLAnchorElement | null;
			if (cardLink) {
				const taskId =
					cardLink.getAttribute('data-card-ref') ??
					new URL(cardLink.href, window.location.origin).searchParams.get('task');
				telemetry.log('card_referenced_from_chat', {
					task_id: taskId,
					message_id: msgId,
					channel_id: chanId
				});
				openTaskInPlace(cardLink, e);
				return;
			}

			// @mention badges → mention_clicked
			const mention = target.closest('.chat-mention') as HTMLElement | null;
			if (mention) {
				telemetry.log('mention_clicked', {
					user_id: mention.getAttribute('data-user-id'),
					message_id: msgId,
					channel_id: chanId
				});
				return;
			}

			// Plain external links → link_clicked
			const anchor = target.closest('a') as HTMLAnchorElement | null;
			if (anchor) {
				telemetry.log('link_clicked', {
					href: anchor.href,
					message_id: msgId,
					channel_id: chanId
				});
			}
		};
		node.addEventListener('click', handler);
		return {
			destroy() {
				node.removeEventListener('click', handler);
			}
		};
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

	const isGitHubBot = $derived(!isPending(message) && message.sender_id === GITHUB_BOT_ID);
	const githubMeta = $derived(
		isGitHubBot && !isPending(message) && message.metadata?.kind
			? (message.metadata as unknown as GitHubMessageMetadata)
			: null
	);
	const isMuted = $derived(
		typeof localStorage !== 'undefined' &&
			(() => {
				try {
					const muted: string[] = JSON.parse(localStorage.getItem('weld_muted_senders') ?? '[]');
					return muted.includes(GITHUB_BOT_ID);
				} catch {
					return false;
				}
			})()
	);
</script>

{#if isMuted}<!-- message from muted sender, render nothing -->
{:else}
	<div
		class={cn(
			'group relative flex gap-3 rounded-lg px-4 py-2 hover:bg-[var(--accent)]/40',
			pending && 'opacity-60'
		)}
	>
		<div class="w-8 shrink-0">
			{#if showAvatar}
				{#if isGitHubBot}
					<div
						class="flex h-8 w-8 items-center justify-center rounded-full bg-[#24292e] text-white shadow-sm"
						aria-hidden="true"
					>
						<svg viewBox="0 0 16 16" class="h-5 w-5 fill-current" aria-hidden="true">
							<path
								d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
							/>
						</svg>
					</div>
				{:else}
					<div
						class="flex h-8 w-8 items-center justify-center rounded-full bg-[var(--accent)] text-xs font-semibold text-[var(--foreground)]/70 shadow-sm"
						aria-hidden="true"
					>
						{initials(senderName)}
					</div>
				{/if}
			{/if}
		</div>

		<div class="min-w-0 flex-1">
			{#if showAvatar}
				<div class="mb-0.5 flex items-baseline gap-2">
					<span class="text-sm font-semibold">{senderName}</span>
					<span class="text-xs text-[var(--muted-foreground)]">{time}</span>
					{#if !isPending(message) && message.edited_at}
						<span class="text-xs text-[var(--muted-foreground)] italic">(edited)</span>
					{/if}
				</div>
			{/if}

			{#if githubMeta}
				<GitHubCard meta={githubMeta} />
			{:else if editing}
				<div class="mt-1 space-y-1.5">
					<textarea
						bind:value={draft}
						use:focusEditor
						rows={3}
						disabled={saving}
						class="w-full resize-y rounded-lg border border-[var(--input)] bg-[var(--background)] p-2.5 text-sm transition-colors outline-none focus-visible:border-[var(--ring)]/60 focus-visible:ring-2 focus-visible:ring-[var(--ring)]/20 disabled:opacity-60"
					></textarea>
					<div class="flex items-center gap-2 text-xs">
						<button
							type="button"
							disabled={saving}
							class="rounded-md bg-[var(--primary)] px-2.5 py-1 font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-60"
							onclick={saveEdit}
						>
							{saving ? 'Saving…' : 'Save'}
						</button>
						<button
							type="button"
							disabled={saving}
							class="rounded-md border border-[var(--border)] px-2.5 py-1 hover:bg-[var(--accent)] disabled:opacity-60"
							onclick={cancelEdit}
						>
							Cancel
						</button>
						{#if editError}
							<span class="text-red-500">{editError}</span>
						{/if}
					</div>
				</div>
			{:else}
				<div class="chat-message-body text-sm leading-relaxed" use:trackMessageInteractions>
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					{@html html}
				</div>
			{/if}

			{#if !editing && replyCount > 0}
				<button
					type="button"
					class="mt-1.5 inline-flex items-center gap-1.5 rounded-md border border-[var(--border)] bg-[var(--accent)]/50 px-2 py-0.5 text-xs font-medium text-[var(--foreground)]/70 hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
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
				<div class="mt-1 text-xs text-[var(--muted-foreground)] italic">Sending…</div>
			{/if}
		</div>

		{#if !pending && !editing && !githubMeta}
			<div
				class="absolute -top-3 right-3 hidden gap-0.5 rounded-lg border border-[var(--border)] bg-[var(--card)] px-1 py-0.5 shadow-md group-hover:flex"
			>
				{#if onReply && !message.thread_parent_id}
					<button
						type="button"
						class="rounded-md p-1.5 text-[var(--muted-foreground)] transition-colors hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
						title="Reply in thread"
						onclick={() => onReply?.(message.id)}
					>
						<MessageSquare class="h-3.5 w-3.5" />
					</button>
				{/if}
				{#if message.sender_id === selfId}
					<button
						type="button"
						class="rounded-md p-1.5 text-[var(--muted-foreground)] transition-colors hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
						title="Edit"
						onclick={startEdit}
					>
						<Pencil class="h-3.5 w-3.5" />
					</button>
				{/if}
				{#if canDelete}
					<button
						type="button"
						class="rounded-md p-1.5 text-[var(--muted-foreground)] transition-colors hover:bg-red-500/15 hover:text-red-500"
						title="Delete"
						onclick={() => chat.deleteMessage(message.id, message.channel_id)}
					>
						<Trash2 class="h-3.5 w-3.5" />
					</button>
				{/if}
			</div>
		{/if}
	</div>
{/if}

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
	:global(.chat-taskref) {
		display: inline-flex;
		align-items: baseline;
		gap: 0.25rem;
		max-width: 22rem;
		padding: 0 0.35rem;
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--foreground) 8%, transparent);
		border: 1px solid var(--border);
		color: var(--foreground);
		font-weight: 500;
		text-decoration: none;
		vertical-align: baseline;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	:global(.chat-taskref:hover) {
		background: color-mix(in srgb, var(--primary) 14%, transparent);
		border-color: color-mix(in srgb, var(--primary) 40%, transparent);
	}
	:global(.chat-taskref .chat-taskref-id) {
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-size: 0.85em;
		color: var(--primary);
	}
	:global(.chat-prref) {
		display: inline-flex;
		align-items: baseline;
		gap: 0.25rem;
		max-width: 22rem;
		padding: 0 0.35rem;
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--foreground) 8%, transparent);
		border: 1px solid var(--border);
		color: var(--foreground);
		font-weight: 500;
		text-decoration: none;
		vertical-align: baseline;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	/* Bare badge has no linked task: keep it inert (no pointer, no hover). */
	:global(.chat-prref.chat-prref-bare) {
		cursor: default;
		opacity: 0.85;
	}
	:global(a.chat-prref:hover) {
		background: color-mix(in srgb, var(--primary) 14%, transparent);
		border-color: color-mix(in srgb, var(--primary) 40%, transparent);
	}
	:global(.chat-prref .chat-prref-id) {
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-size: 0.85em;
		color: var(--primary);
	}
</style>
