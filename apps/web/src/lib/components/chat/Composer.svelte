<script lang="ts">
	import type { TaskRef, TeamMember } from '$lib/api/types';
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
	// Which autocomplete is active: '@' for members, '#' for tasks.
	let trigger = $state<'@' | '#' | null>(null);
	let query = $state('');
	let anchor = $state(0);
	let highlight = $state(0);

	const memberMatches = $derived.by(() => {
		if (trigger !== '@') return [] as TeamMember[];
		const lower = query.toLowerCase();
		return members
			.filter((m) => {
				if (!lower) return true;
				const local = m.email.split('@')[0].toLowerCase();
				const display = m.display_name.replace(/\s+/g, '').toLowerCase();
				return local.startsWith(lower) || display.startsWith(lower);
			})
			.slice(0, 6);
	});

	const taskMatches = $derived.by(() => {
		if (trigger !== '#') return [] as TaskRef[];
		const lower = query.toLowerCase();
		return chat.taskRefs
			.filter((t) => {
				if (!lower) return true;
				return String(t.short_id).startsWith(lower) || t.title.toLowerCase().includes(lower);
			})
			.slice(0, 6);
	});

	const matchCount = $derived(trigger === '@' ? memberMatches.length : taskMatches.length);

	function autoResize() {
		if (!textarea) return;
		textarea.style.height = 'auto';
		textarea.style.height = Math.min(textarea.scrollHeight, 240) + 'px';
	}

	function onInput(e: Event) {
		value = (e.currentTarget as HTMLTextAreaElement).value;
		updateContext();
		autoResize();
		if (value.trim()) chat.emitTyping(channelId);
	}

	function updateContext() {
		if (!textarea) return;
		const caret = textarea.selectionStart ?? value.length;
		const prefix = value.slice(0, caret);

		const mention = /(^|\s)@([A-Za-z0-9_.-]*)$/.exec(prefix);
		if (mention) {
			trigger = '@';
			query = mention[2];
			anchor = caret - mention[2].length - 1;
			highlight = 0;
			return;
		}

		const task = /(^|\s)#([A-Za-z0-9_-]*)$/.exec(prefix);
		if (task) {
			trigger = '#';
			query = task[2];
			anchor = caret - task[2].length - 1;
			highlight = 0;
			return;
		}

		trigger = null;
	}

	function replaceAtAnchor(insertion: string) {
		if (!textarea) return;
		const caret = textarea.selectionStart ?? value.length;
		const before = value.slice(0, anchor);
		const after = value.slice(caret);
		value = before + insertion + after;
		trigger = null;
		const newCaret = (before + insertion).length;
		queueMicrotask(() => {
			if (!textarea) return;
			textarea.focus();
			textarea.setSelectionRange(newCaret, newCaret);
			autoResize();
		});
	}

	function selectMember(member: TeamMember) {
		replaceAtAnchor('@' + member.email.split('@')[0] + ' ');
	}

	function selectTask(task: TaskRef) {
		replaceAtAnchor('#' + task.short_id + ' ');
	}

	function applyHighlight() {
		if (trigger === '@' && memberMatches[highlight]) selectMember(memberMatches[highlight]);
		else if (trigger === '#' && taskMatches[highlight]) selectTask(taskMatches[highlight]);
	}

	async function send() {
		const trimmed = value.trim();
		if (!trimmed) return;
		value = '';
		trigger = null;
		queueMicrotask(autoResize);
		await chat.sendMessage(channelId, trimmed, threadParentId);
	}

	function onKeydown(e: KeyboardEvent) {
		if (trigger !== null && matchCount > 0) {
			if (e.key === 'ArrowDown') {
				e.preventDefault();
				highlight = (highlight + 1) % matchCount;
				return;
			}
			if (e.key === 'ArrowUp') {
				e.preventDefault();
				highlight = (highlight - 1 + matchCount) % matchCount;
				return;
			}
			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				applyHighlight();
				return;
			}
			if (e.key === 'Escape') {
				e.preventDefault();
				trigger = null;
				return;
			}
		}
		if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
			e.preventDefault();
			void send();
		}
	}
</script>

<div class="relative border-t border-[var(--border)] bg-[var(--card)] px-4 py-3">
	{#if trigger === '@' && memberMatches.length > 0}
		<ul
			class="absolute bottom-full left-4 mb-1.5 w-64 overflow-hidden rounded-lg border border-[var(--border)] bg-[var(--card)] p-1 shadow-xl"
			role="listbox"
		>
			{#each memberMatches as m, i (m.user_id)}
				<li>
					<button
						type="button"
						class="flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors {i ===
						highlight
							? 'bg-[var(--accent)] text-[var(--accent-foreground)]'
							: 'hover:bg-[var(--accent)]/60'}"
						onmousedown={(e) => {
							e.preventDefault();
							selectMember(m);
						}}
					>
						<span
							class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[var(--muted)] text-[10px] font-semibold"
						>
							{m.display_name.charAt(0).toUpperCase()}
						</span>
						<span class="font-medium">{m.display_name}</span>
						<span class="ml-auto text-xs text-[var(--muted-foreground)]">
							@{m.email.split('@')[0]}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{:else if trigger === '#' && taskMatches.length > 0}
		<ul
			class="absolute bottom-full left-4 mb-1.5 w-80 overflow-hidden rounded-lg border border-[var(--border)] bg-[var(--card)] p-1 shadow-xl"
			role="listbox"
		>
			{#each taskMatches as t, i (t.id)}
				<li>
					<button
						type="button"
						class="flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors {i ===
						highlight
							? 'bg-[var(--accent)] text-[var(--accent-foreground)]'
							: 'hover:bg-[var(--accent)]/60'}"
						onmousedown={(e) => {
							e.preventDefault();
							selectTask(t);
						}}
					>
						<span class="shrink-0 font-mono text-xs text-[var(--muted-foreground)]">
							#{t.short_id}
						</span>
						<span class="truncate">{t.title}</span>
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
		class="block w-full resize-none rounded-lg border border-[var(--input)] bg-[var(--background)] px-3.5 py-2.5 text-sm outline-none transition-colors focus-visible:border-[var(--ring)]/60 focus-visible:ring-2 focus-visible:ring-[var(--ring)]/20"
		aria-label="Message composer"
	></textarea>
	<div class="mt-2 flex items-center justify-between">
		<span class="text-[11px] text-[var(--muted-foreground)]">
			⏎ send &nbsp;·&nbsp; ⇧⏎ newline &nbsp;·&nbsp; @ mention &nbsp;·&nbsp; # task
		</span>
		<button
			type="button"
			class="rounded-md bg-[var(--primary)] px-3.5 py-1.5 text-xs font-semibold text-[var(--primary-foreground)] transition-opacity hover:opacity-90 disabled:opacity-40"
			onclick={() => send()}
			disabled={!value.trim()}
		>
			Send
		</button>
	</div>
</div>
