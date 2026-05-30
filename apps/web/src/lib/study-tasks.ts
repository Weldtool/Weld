/**
 * Study task script (research-paper/WELD_AGENT_TASK_SCRIPTS.md). Loaded as
 * config so wording can change without code edits. Two manual baseline tasks
 * (C0, agent hidden) then six agent tasks in the participant's assigned
 * condition. Order is fixed: baseline → comprehension → single-surface →
 * cross-surface, so difficulty ramps.
 *
 * The agent tasks reference "the agent"; the agent UI ships separately. Until
 * then the panel still drives task sequencing + timing telemetry.
 */
export type StudyPhase = 'baseline' | 'agent';

export interface StudyTask {
	id: string;
	phase: StudyPhase;
	/** Short label for the progress header. */
	title: string;
	/** Verbatim instruction shown to the participant. */
	instruction: string;
}

export const STUDY_TASKS: StudyTask[] = [
	// ── Phase 0 — Manual baseline (no agent) ──────────────────────────────────
	{
		id: 'B1',
		phase: 'baseline',
		title: 'Baseline 1',
		instruction:
			'Read the recent messages in #general. Priya mentioned one card she’s blocked on. Find that card on the board and move it to the “In Progress” column.'
	},
	{
		id: 'B2',
		phase: 'baseline',
		title: 'Baseline 2',
		instruction:
			'A teammate asked in #general for the status of the “Pet profile creation” work. Check the board to find that card’s current column, then post a one-sentence status reply in #general.'
	},
	// ── Phase 1 — Agent tasks (assigned condition) ────────────────────────────
	{
		id: 'T1',
		phase: 'agent',
		title: 'Task 1',
		instruction:
			'Ask the agent what the team most needs done next, based on the recent discussion. Then act on its suggestion.'
	},
	{
		id: 'T2',
		phase: 'agent',
		title: 'Task 2',
		instruction:
			'You’re returning after being away. Ask the agent to summarize what’s happened recently and what’s waiting on you.'
	},
	{
		id: 'T3',
		phase: 'agent',
		title: 'Task 3',
		instruction:
			'Ask the agent to draft a short status update for #pettracker summarizing where the vaccination reminders work stands, then post it.'
	},
	{
		id: 'T4',
		phase: 'agent',
		title: 'Task 4',
		instruction:
			'Three new-issue cards are in the Backlog. Ask the agent to help you set their priorities sensibly, then make sure the priorities are set.'
	},
	{
		id: 'T5',
		phase: 'agent',
		title: 'Task 5',
		instruction:
			'Sarah reported a bug in #pettracker. Ask the agent to find it, open the related card, and move it to In Progress. Confirm it’s done.'
	},
	{
		id: 'T6',
		phase: 'agent',
		title: 'Task 6',
		instruction:
			'The team needs the vaccination reminders work connected to its code. Ask the agent to find the open pull request related to the vaccination/feeding work, link it to the right board card, and post a note in #pettracker that they’re linked.'
	}
];

/** Human-facing phase descriptions for interstitials. */
export const PHASE_LABELS: Record<StudyPhase, string> = {
	baseline: 'Warm-up',
	agent: 'Main tasks'
};
