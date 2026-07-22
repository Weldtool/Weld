<script lang="ts">
	import { messages as messagesApi } from '$lib/api';
	import { agentDrafts } from '$lib/agent-drafts.svelte';
	import { study } from '$lib/study.svelte';
	import { telemetry } from '$lib/telemetry.svelte';
	import { Sparkles, X } from '@lucide/svelte';

	interface Props {
		channelId: string;
	}
	const { channelId }: Props = $props();

	const drafts = $derived(agentDrafts.forChannel(channelId));

	/** Send a reviewed/edited draft as the participant's own message, logging
	 *  whether they accepted it verbatim or edited it first (RQ1). */
	async function sendDraft(id: number) {
		const d = agentDrafts.items.find((x) => x.id === id);
		if (!d || d.sending) return;
		const content = d.content.trim();
		if (!content) {
			d.error = 'Message is empty.';
			return;
		}
		d.sending = true;
		d.error = null;
		try {
			await messagesApi.post(d.channelId, { content });
			const edited = content !== d.original.trim();
			telemetry.log('message_sent', {
				channel: d.channel,
				length: content.length,
				contains_mention: /@\w/.test(content),
				contains_link: /https?:\/\//.test(content),
				agent_drafted: true
			});
			telemetry.log(edited ? 'agent_output_edited' : 'agent_suggestion_accepted', {
				task_id: study.currentTask?.id ?? null,
				surface: 'chat',
				channel: d.channel,
				edited,
				original_length: d.original.trim().length,
				sent_length: content.length
			});
			agentDrafts.remove(id);
		} catch (e) {
			d.error = e instanceof Error ? e.message : 'Could not send. Please try again.';
		} finally {
			d.sending = false;
		}
	}

	/** Discard a draft without sending — an explicit rejection of agent output. */
	function discardDraft(id: number) {
		const d = agentDrafts.items.find((x) => x.id === id);
		if (!d) return;
		telemetry.log('agent_suggestion_rejected', {
			task_id: study.currentTask?.id ?? null,
			surface: 'chat',
			channel: d.channel
		});
		agentDrafts.remove(id);
	}
</script>

{#each drafts as draft (draft.id)}
	<div class="border-t border-[var(--border)] bg-[var(--accent)]/40 px-4 pt-3 pb-1">
		<div class="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-[var(--muted-foreground)]">
			<Sparkles class="h-3.5 w-3.5 text-[var(--primary)]" />
			The assistant drafted this message — review, edit, then send.
		</div>
		<textarea
			bind:value={draft.content}
			rows="4"
			aria-label="Drafted message for #{draft.channel}"
			class="block w-full resize-y rounded-lg border border-[var(--input)] bg-[var(--background)] px-3.5 py-2.5 text-sm leading-relaxed outline-none focus-visible:border-[var(--ring)]/60 focus-visible:ring-2 focus-visible:ring-[var(--ring)]/20"
		></textarea>
		{#if draft.error}
			<p class="mt-1 text-xs text-red-500">{draft.error}</p>
		{/if}
		<div class="mt-2 flex items-center gap-2">
			<button
				type="button"
				onclick={() => sendDraft(draft.id)}
				disabled={draft.sending || draft.content.trim().length === 0}
				class="rounded-md bg-[var(--primary)] px-3.5 py-1.5 text-xs font-semibold text-[var(--primary-foreground)] transition-opacity hover:opacity-90 disabled:opacity-40"
			>
				{draft.sending ? 'Sending…' : 'Send to #' + draft.channel}
			</button>
			<button
				type="button"
				onclick={() => discardDraft(draft.id)}
				disabled={draft.sending}
				class="flex items-center gap-1 rounded-md border border-[var(--border)] px-3 py-1.5 text-xs hover:bg-[var(--accent)]"
			>
				<X class="h-3.5 w-3.5" />
				Discard
			</button>
		</div>
	</div>
{/each}
