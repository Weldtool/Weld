import type { AgentEntity } from '$lib/api';

/**
 * Tracks which board cards the study agent created or modified during the
 * current task, so that a subsequent *user* edit/move/delete of the same card
 * can be attributed as verification/repair behaviour — the delegation/validate-
 * and-repair signal behind RQ1 (CLAUDE_AGENT_BUILD.md §5, parallels Tang et al.).
 *
 * Scoped to one task at a time: registering activity for a new task clears the
 * previous task's set so signals never leak across tasks. Inert outside study
 * mode (nothing ever calls `note`), so the board store can consult it freely.
 */
class AgentActivity {
	#taskId: string | null = null;
	/** cardId → the most recent agent action on that card this task. */
	#touched = new Map<string, AgentEntity['action']>();

	/** Record the entities an agent turn touched, scoped to `taskId`. */
	note(taskId: string | null, entities: AgentEntity[]): void {
		if (taskId !== this.#taskId) {
			this.#taskId = taskId;
			this.#touched.clear();
		}
		for (const e of entities) {
			if (e.kind === 'card') this.#touched.set(e.id, e.action);
		}
	}

	/** The agent's last action on this card in the current task, or null. */
	agentAction(cardId: string): AgentEntity['action'] | null {
		return this.#touched.get(cardId) ?? null;
	}

	/** Forget all tracked activity (e.g. on task change with no agent turn yet). */
	reset(): void {
		this.#taskId = null;
		this.#touched.clear();
	}
}

export const agentActivity = new AgentActivity();
