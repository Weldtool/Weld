import { boardStore } from '$lib/board.svelte';
import { chat } from '$lib/chat.svelte';

/**
 * Build a self-contained snapshot of the workspace's final state for the task
 * the participant just finished, emitted as the `task_final_state` telemetry
 * event. The event log already captures the *process* (every user/agent action);
 * this captures the *outcome* so a coder can score correctness (§2 rubric) from
 * the logs alone, without depending on the live database surviving.
 *
 * Best-effort: it reflects whatever the board/chat stores currently hold (the
 * surfaces the participant actually loaded). The provisioned team in the DB
 * remains the authoritative source; this is a durable backup.
 *
 * Message content IS included here — it is the participant-authored task output
 * a coder must read to score B2/T3/T6, i.e. study data, not the keystroke/screen
 * capture that §5 forbids.
 */
export function buildTaskSnapshot(): Record<string, unknown> {
	const columnName = (id: string | null) =>
		boardStore.columns.find((c) => c.id === id)?.name ?? null;

	const board = {
		project_id: boardStore.projectId,
		columns: boardStore.columns.map((c) => ({
			id: c.id,
			name: c.name,
			position: c.position
		})),
		tasks: boardStore.tasks.map((t) => ({
			id: t.id,
			short_id: t.short_id,
			title: t.title,
			column_id: t.column_id,
			column: columnName(t.column_id),
			priority: t.priority,
			assignee_id: t.assignee_id,
			github_pr_id: t.github_pr_id,
			github_issue_id: t.github_issue_id,
			labels: t.labels
		}))
	};

	// Loaded channels only (the ones the participant opened). Skip optimistic
	// pending messages — they aren't server-confirmed final state.
	const channelName = (id: string) => chat.channels.find((c) => c.id === id)?.name ?? null;
	const channels = Object.entries(chat.messagesByChannel).map(([channelId, view]) => ({
		channel_id: channelId,
		channel: channelName(channelId),
		messages: view.messages
			.filter((m) => !('pending' in m && m.pending))
			.map((m) => ({
				id: m.id,
				sender_id: m.sender_id,
				content: m.content,
				thread_parent_id: m.thread_parent_id,
				task_refs: m.task_refs,
				mentions: m.mentions,
				edited_at: m.edited_at,
				created_at: m.created_at
			}))
	}));

	return { board, chat: channels };
}
