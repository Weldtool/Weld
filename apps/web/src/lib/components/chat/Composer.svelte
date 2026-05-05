<script lang="ts">
	import type { TeamMember } from '$lib/api/types';
	import { chat } from '$lib/chat.svelte';

	interface Props {
		channelId: string;
		threadParentId?: string;
		members: TeamMember[];
		placeholder?: string;
	}

	const { channelId, threadParentId, members, placeholder = 'Message…' }: Props = $props();

	let textarea: HTMLTextAreaElement | null = $state(null);
	let value = $state('');
	let mentionQuery = $state<string | null>(null);
	let mentionAnchor = $state(0);
	let highlight = $state(0);

	const matches = $derived.by(() => {
		const q = mentionQuery;
		if (q === null) return [] as TeamMember[];
		const lower = q.toLowerCase();
		return members
			.filter((m) => {
				if (!lower) return true;
				const local = m.email.split('@')[0].toLowerCase();
				const display = m.display_name.replace(/\s+/g, '').toLowerCase();
				return local.startsWith(lower) || display.startsWith(lower);
			})
			.slice(0, 6);
	});

	function autoResize() {
		if (!textarea) return;
		textarea.style.height = 'auto';
		textarea.style.height = Math.min(textarea.scrollHeight, 240) + 'px';
	}

	function onInput(e: Event) {
		value = (e.currentTarget as HTMLTextAreaElement).value;
		updateMentionContext();
		autoResize();
		if (value.trim()) chat.emitTyping(channelId);
	}

	function updateMentionContext() {
		if (!textarea) return;
		const caret = textarea.selectionStart ?? value.length;
		const prefix = value.slice(0, caret);
		const m = /(^|\s)@([A-Za-z0-9_.-]*)$/.exec(prefix);
		if (!m) {
			mentionQuery = null;
			return;
		}
		mentionQuery = m[2];
		mentionAnchor = caret - m[2].length - 1;
		highlight = 0;
	}

	function selectMention(member: TeamMember) {
		if (!textarea) return;
		const caret = textarea.selectionStart ?? value.length;
		const handle = member.email.split('@')[0];
		const before = value.slice(0, mentionAnchor);
		const after = value.slice(caret);
		const insertion = '@' + handle + ' ';
		value = before + insertion + after;
		mentionQuery = null;
		const newCaret = (before + insertion).length;
		queueMicrotask(() => {
			if (!textarea) return;
			textarea.focus();
			textarea.setSelectionRange(newCaret, newCaret);
			autoResize();
		});
	}

	async function send() {
		const trimmed = value.trim();
		if (!trimmed) return;
		value = '';
		mentionQuery = null;
		queueMicrotask(autoResize);
		await chat.sendMessage(channelId, trimmed, threadParentId);
	}

	function onKeydown(e: KeyboardEvent) {
		if (mentionQuery !== null && matches.length > 0) {
			if (e.key === 'ArrowDown') {
				e.preventDefault();
				highlight = (highlight + 1) % matches.length;
				return;
			}
			if (e.key === 'ArrowUp') {
				e.preventDefault();
				highlight = (highlight - 1 + matches.length) % matches.length;
				return;
			}
			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				selectMention(matches[highlight]);
				return;
			}
			if (e.key === 'Escape') {
				e.preventDefault();
				mentionQuery = null;
				return;
			}
		}
		if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
			e.preventDefault();
			void send();
		}
	}
</script>

<div class="relative border-t border-[var(--border)] bg-[var(--background)] p-3">
	{#if mentionQuery !== null && matches.length > 0}
		<ul
			class="absolute bottom-full left-3 mb-1 w-64 rounded-md border border-[var(--border)] bg-[var(--card)] p-1 shadow-lg"
			role="listbox"
		>
			{#each matches as m, i (m.user_id)}
				<li>
					<button
						type="button"
						class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm {i ===
						highlight
							? 'bg-[var(--accent)] text-[var(--accent-foreground)]'
							: 'hover:bg-[var(--accent)]/60'}"
						onmousedown={(e) => {
							e.preventDefault();
							selectMention(m);
						}}
					>
						<span class="font-medium">{m.display_name}</span>
						<span class="text-xs text-[var(--muted-foreground)]">
							@{m.email.split('@')[0]}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}

	<textarea
		bind:this={textarea}
		bind:value
		oninput={onInput}
		onkeydown={onKeydown}
		{placeholder}
		rows={1}
		class="block w-full resize-none rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
		aria-label="Message composer"
	></textarea>
	<div class="mt-1 flex justify-between text-[10px] text-[var(--muted-foreground)]">
		<span>Enter to send · Shift+Enter for newline · @ to mention</span>
		<button
			type="button"
			class="rounded-md bg-[var(--primary)] px-3 py-1 text-xs font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
			onclick={() => send()}
			disabled={!value.trim()}
		>
			Send
		</button>
	</div>
</div>
