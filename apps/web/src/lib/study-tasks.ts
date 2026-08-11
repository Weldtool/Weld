/**
 * Study task script (paper/replication/WELD_AGENT_TASK_SCRIPTS.md). Loaded as
 * config so wording can change without code edits. Six manual baseline tasks
 * (C0, agent hidden) then six agent tasks in the participant's assigned
 * condition. Order is fixed: baseline → comprehension → single-surface →
 * cross-surface, so difficulty ramps.
 *
 * The baseline battery exercises the integrated workspace manually (cross-
 * references, GitHub view, chat-consulted-before-acting) so RQ1 — the value of
 * the all-in-one workspace itself — is measured without the agent confound, and
 * gives a stabler per-person normalizer for the agent tasks. Baseline content is
 * deliberately disjoint from the agent tasks so it can't teach their answers.
 *
 * Each task carries a `type` (the H2 task-dependence bucket) and the `surfaces`
 * it spans, stamped onto task telemetry so per-task-type analysis (does context
 * breadth help more on cross-surface than comprehension tasks?) is first-class
 * rather than hand-mapped at analysis time.
 *
 * Two wording rules, both load-bearing — don't "helpfully" undo them:
 *
 * 1. Agent tasks say the assistant *is available*, never "use the assistant."
 *    RQ2 measures how much people choose to delegate; instructing them to
 *    delegate would measure compliance instead.
 * 2. Tasks state a goal, not a route. Naming the surfaces and steps ("open this
 *    card, then attach the PR") lets the participant navigate without the
 *    agent's context, which is precisely the variable under test — it collapses
 *    condition differences into UI mechanics. Baseline tasks may be explicit
 *    (they are a per-person normalizer, equally explicit for every condition);
 *    agent tasks must not be.
 */
export type StudyPhase = 'baseline' | 'agent';

/** H2 task-dependence bucket — the dimension the condition effect is sliced by. */
export type StudyTaskType = 'comprehension' | 'single_surface' | 'cross_surface';

/** Workspace surfaces a task touches. */
export type StudySurface = 'chat' | 'board' | 'github';

export interface StudyTask {
	id: string;
	phase: StudyPhase;
	/** H2 bucket for per-task-type comparison. */
	type: StudyTaskType;
	/** Surfaces the task spans (secondary descriptor for integration analysis). */
	surfaces: StudySurface[];
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
		type: 'single_surface',
		surfaces: ['chat', 'board'],
		title: 'Baseline 1',
		instruction:
			'Read the recent messages in #general. Priya mentioned one card she’s blocked on. Find that card on the board and move it to the “In Progress” column.'
	},
	{
		id: 'B2',
		phase: 'baseline',
		type: 'cross_surface',
		surfaces: ['chat', 'board'],
		title: 'Baseline 2',
		instruction:
			'A teammate asked in #general for the status of the “Pet profile creation” work. Check the board to find that card’s current column, then post a one-sentence status reply in #general.'
	},
	{
		id: 'B3',
		phase: 'baseline',
		type: 'cross_surface',
		surfaces: ['chat', 'board'],
		title: 'Baseline 3',
		instruction:
			'In #pettracker, Alex flagged the “Nearby vet directory” card and linked it in his message. Open that card directly from the link in his message, then set its priority to High.'
	},
	{
		id: 'B4',
		phase: 'baseline',
		type: 'cross_surface',
		surfaces: ['github', 'chat'],
		title: 'Baseline 4',
		instruction:
			'On the board, open the “Feeding schedule editor UI” card and open the pull request attached to it. Review its changes, then post its PR number in #general along with a one-line assessment.'
	},
	{
		id: 'B5',
		phase: 'baseline',
		type: 'cross_surface',
		surfaces: ['chat', 'board'],
		title: 'Baseline 5',
		instruction:
			'Earlier in #pettracker the team discussed which local cache database to use and reached a decision. Find the “Decide local cache database” card on the board and update its description to record what they decided.'
	},
	{
		id: 'B6',
		phase: 'baseline',
		type: 'comprehension',
		surfaces: ['board', 'chat'],
		title: 'Baseline 6',
		instruction:
			'Using the board, find the card about setting up the CI pipeline and open it. Then post a one-sentence reply in #general confirming what it does (for example, that tests run on every pull request).'
	},
	// ── Phase 1 — Agent tasks (assigned condition) ────────────────────────────
	{
		id: 'T1',
		phase: 'agent',
		type: 'comprehension',
		surfaces: ['chat', 'board'],
		title: 'Task 1',
		instruction:
			'Figure out what the team most needs done next, based on the recent discussion, then make that change on the board (for example, set a priority or move a card). The assistant is available if you want it.'
	},
	{
		id: 'T2',
		phase: 'agent',
		type: 'comprehension',
		surfaces: ['chat', 'board', 'github'],
		title: 'Task 2',
		instruction:
			'You’re returning after being away. Get caught up on what’s happened recently and what’s waiting on you, then post a short 2–3 sentence summary in #general of where things stand. The assistant is available if you want it.'
	},
	{
		id: 'T3',
		phase: 'agent',
		type: 'single_surface',
		surfaces: ['chat'],
		title: 'Task 3',
		instruction:
			'A short status update on where the vaccination reminders work stands needs to be posted in #pettracker. The assistant is available if you want it.'
	},
	{
		id: 'T4',
		phase: 'agent',
		type: 'single_surface',
		surfaces: ['board'],
		title: 'Task 4',
		instruction:
			'Three new-issue cards in the Backlog need sensible priorities set. The assistant is available if you want it.'
	},
	{
		id: 'T5',
		phase: 'agent',
		type: 'cross_surface',
		surfaces: ['chat', 'board'],
		title: 'Task 5',
		instruction:
			'A bug Sarah reported in #pettracker needs to be moved to In Progress. The assistant is available if you want it.'
	},
	{
		id: 'T6',
		phase: 'agent',
		type: 'cross_surface',
		surfaces: ['chat', 'board', 'github'],
		title: 'Task 6',
		instruction:
			'The vaccination reminders work isn’t connected to its code yet, and it hasn’t been reviewed. Sort that out and let the team know in #pettracker. The assistant is available if you want it.'
	}
];

/** Human-facing phase descriptions for interstitials. */
export const PHASE_LABELS: Record<StudyPhase, string> = {
	baseline: 'Warm-up',
	agent: 'Main tasks'
};
