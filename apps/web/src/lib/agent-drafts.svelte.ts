/**
 * Chat messages the study agent drafted, awaiting the participant's review and
 * send. Shared state so the agent panel can *create* drafts while the roomier
 * Chat tab renders the editor — the narrow agent panel is cramped for editing
 * (participant feedback). Cleared per task by the agent panel so drafts never
 * bleed across tasks.
 */
export interface AgentDraft {
	id: number;
	/** Channel display name, e.g. "general". */
	channel: string;
	channelId: string;
	/** The agent's original text, kept to detect edit-before-send (RQ1). */
	original: string;
	/** Editable working copy. */
	content: string;
	sending: boolean;
	error: string | null;
}

class AgentDraftStore {
	items = $state<AgentDraft[]>([]);
	#seq = 0;

	/** Queue a draft the agent produced. */
	add(d: { channel: string; channel_id: string; content: string }): void {
		this.items.push({
			id: this.#seq++,
			channel: d.channel,
			channelId: d.channel_id,
			original: d.content,
			content: d.content,
			sending: false,
			error: null
		});
	}

	forChannel(channelId: string): AgentDraft[] {
		return this.items.filter((d) => d.channelId === channelId);
	}

	get count(): number {
		return this.items.length;
	}

	/** Distinct channel names with a pending draft (for the agent-panel pointer). */
	get pendingChannels(): string[] {
		return [...new Set(this.items.map((d) => d.channel))];
	}

	remove(id: number): void {
		this.items = this.items.filter((d) => d.id !== id);
	}

	clear(): void {
		this.items = [];
	}
}

export const agentDrafts = new AgentDraftStore();
