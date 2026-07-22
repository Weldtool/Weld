<script lang="ts">
	import { study as studyApi } from '$lib/api';
	import { agentActivity } from '$lib/agent-activity.svelte';
	import { agentDrafts } from '$lib/agent-drafts.svelte';
	import { study } from '$lib/study.svelte';
	import { telemetry } from '$lib/telemetry.svelte';
	import { cn } from '$lib/utils';
	import { Bot, Send, Sparkles, PanelRightClose, PanelRightOpen } from '@lucide/svelte';

	interface Turn {
		role: 'user' | 'agent';
		text: string;
		actions?: string[];
	}

	const COLLAPSED_KEY = 'weld:agent-collapsed';

	let collapsed = $state(false);
	let turns = $state<Turn[]>([]);
	let input = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);
	let scroller: HTMLDivElement | null = $state(null);
	let textarea: HTMLTextAreaElement | null = $state(null);

	$effect(() => {
		collapsed = localStorage.getItem(COLLAPSED_KEY) === '1';
		// Log the initial visibility so we can tell whether a participant ever saw
		// the panel expanded vs started collapsed from a prior session.
		telemetry.log(collapsed ? 'agent_panel_collapsed' : 'agent_panel_opened', {
			task_id: study.currentTask?.id ?? null,
			trigger: 'mount'
		});
	});

	// Each task is independent — clear the conversation when the task changes so
	// the agent doesn't carry context (or the participant a transcript) across
	// tasks, which would confound the per-task measures.
	let lastTaskId: string | null = null;
	$effect(() => {
		const id = study.currentTask?.id ?? null;
		if (id !== lastTaskId) {
			lastTaskId = id;
			turns = [];
			agentDrafts.clear();
			input = '';
			error = null;
			busy = false;
			// Drop any prior task's agent-touched cards so verification/repair
			// signals never bleed across tasks.
			agentActivity.reset();
			resizeSoon();
		}
	});

	// Grow the textarea with its content up to a max height, then scroll inside.
	function autoGrow() {
		const el = textarea;
		if (!el) return;
		el.style.height = 'auto';
		el.style.height = `${Math.min(el.scrollHeight, 120)}px`;
	}
	function resizeSoon() {
		queueMicrotask(autoGrow);
	}

	function toggleCollapsed() {
		collapsed = !collapsed;
		localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : '0');
		telemetry.log(collapsed ? 'agent_panel_collapsed' : 'agent_panel_opened', {
			task_id: study.currentTask?.id ?? null,
			trigger: 'toggle'
		});
	}

	async function send() {
		const message = input.trim();
		if (!message || busy) return;
		input = '';
		resizeSoon();
		error = null;
		turns.push({ role: 'user', text: message });
		busy = true;
		scrollSoon();
		try {
			const res = await studyApi.agent({
				message,
				current_task_id: study.currentTask?.id ?? null
			});
			// Remember the cards the agent touched so a later user edit to them is
			// logged as verification/repair (RQ1).
			agentActivity.note(study.currentTask?.id ?? null, res.entities);
			turns.push({ role: 'agent', text: res.reply, actions: res.actions });
			// Drafted chat messages are edited/sent from the roomier Chat tab (the
			// panel is too cramped); queue them and point the participant there.
			for (const d of res.drafts) agentDrafts.add(d);
		} catch (e) {
			error = e instanceof Error ? e.message : 'The assistant is unavailable. Please try again.';
		} finally {
			busy = false;
			scrollSoon();
		}
	}

	function scrollSoon() {
		queueMicrotask(() => scroller?.scrollTo({ top: scroller.scrollHeight }));
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			void send();
		}
	}
</script>

<aside
	class={cn(
		'flex h-screen shrink-0 flex-col border-l border-[var(--border)] bg-[var(--card)] transition-[width] duration-200',
		collapsed ? 'w-12' : 'w-80'
	)}
	aria-label="AI assistant"
>
	{#if collapsed}
		<button
			type="button"
			onclick={toggleCollapsed}
			class="flex flex-col items-center gap-3 py-3 text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
			aria-label="Open assistant"
			title="Open assistant"
		>
			<PanelRightOpen class="h-4 w-4" />
			<Bot class="h-5 w-5 text-[var(--primary)]" />
		</button>
	{:else}
		<header class="flex shrink-0 items-center gap-2 border-b border-[var(--border)] px-4 py-2.5">
			<Bot class="h-4 w-4 text-[var(--primary)]" />
			<span class="text-sm font-medium">Assistant</span>
			<button
				type="button"
				onclick={toggleCollapsed}
				class="ml-auto flex h-7 w-7 items-center justify-center rounded-md text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
				aria-label="Collapse assistant"
				title="Collapse assistant"
			>
				<PanelRightClose class="h-4 w-4" />
			</button>
		</header>

		<div bind:this={scroller} class="flex-1 space-y-3 overflow-y-auto px-4 py-3">
			{#if turns.length === 0}
				<p class="text-xs text-[var(--muted-foreground)]">
					Ask the assistant for help with the current task.
				</p>
			{/if}
			{#each turns as turn, i (i)}
				{#if turn.role === 'user'}
					<div
						class="ml-6 rounded-lg bg-[var(--primary)] px-3 py-2 text-xs text-[var(--primary-foreground)]"
					>
						{turn.text}
					</div>
				{:else}
					<div class="mr-6 rounded-lg bg-[var(--accent)] px-3 py-2 text-xs">
						<p class="whitespace-pre-wrap">{turn.text}</p>
						{#if turn.actions && turn.actions.length > 0}
							<ul
								class="mt-2 space-y-0.5 border-t border-[var(--border)] pt-2 text-[11px] text-[var(--muted-foreground)]"
							>
								{#each turn.actions as action, j (j)}
									<li>✓ {action}</li>
								{/each}
							</ul>
						{/if}
					</div>
				{/if}
			{/each}
			{#if busy}
				<div
					class="mr-6 rounded-lg bg-[var(--accent)] px-3 py-2 text-xs text-[var(--muted-foreground)]"
				>
					Thinking…
				</div>
			{/if}
			{#if agentDrafts.count > 0}
				<div
					class="rounded-lg border border-[var(--primary)]/40 bg-[var(--primary)]/5 p-2.5 text-xs"
				>
					<p class="flex items-center gap-1.5 font-medium">
						<Sparkles class="h-3.5 w-3.5 text-[var(--primary)]" />
						Draft ready to review
					</p>
					<p class="mt-1 text-[var(--muted-foreground)]">
						Open the <span class="font-medium text-[var(--foreground)]">Chat</span> tab
						{#if agentDrafts.pendingChannels.length > 0}
							(<span class="font-medium text-[var(--foreground)]"
								>#{agentDrafts.pendingChannels.join(', #')}</span
							>)
						{/if}
						to review, edit, and send it.
					</p>
				</div>
			{/if}
			{#if error}
				<p class="text-xs text-red-500">{error}</p>
			{/if}
		</div>

		<div class="border-t border-[var(--border)] p-2">
			<div class="flex items-end gap-2">
				<textarea
					bind:this={textarea}
					bind:value={input}
					onkeydown={onKey}
					oninput={autoGrow}
					rows="1"
					placeholder="Ask the assistant…"
					class="max-h-[120px] flex-1 resize-none overflow-y-auto rounded-md border border-[var(--border)] bg-[var(--background)] px-2 py-1.5 text-xs leading-snug"
				></textarea>
				<button
					type="button"
					onclick={send}
					disabled={busy || input.trim().length === 0}
					aria-label="Send"
					class="rounded-md bg-[var(--primary)] p-1.5 text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
				>
					<Send class="h-4 w-4" />
				</button>
			</div>
		</div>
	{/if}
</aside>
