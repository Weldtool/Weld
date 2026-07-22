/**
 * Exit-survey instrument (research-paper/WELD_EXIT_SURVEY.md + the trust block
 * from WELD_AGENT_TASK_SCRIPTS.md §3). Config-driven so wording/scales change
 * without touching the renderer. Stored keyed by `item_id`, never question text.
 */
export type SurveyItemType = 'slider100' | 'likert5' | 'likert7' | 'text' | 'single' | 'multi';

export interface SurveyItem {
	id: string;
	label: string;
	type: SurveyItemType;
	lowAnchor?: string;
	highAnchor?: string;
	options?: { value: string; label: string }[];
	required?: boolean;
}

export interface SurveySection {
	title: string;
	instruction?: string;
	items: SurveyItem[];
}

const SUS_LABEL = (s: string) => s;

/**
 * NASA-TLX (6 raw subscales). Administered twice with the SAME items so the two
 * load measurements are comparable: once right after the manual baseline phase
 * (ids suffixed `_baseline`, via the interstitial — see `baselineTlxItems()`)
 * and once in the exit survey for the agent phase (bare ids, below). The
 * within-person baseline→agent delta is the H3 cognitive-load signal.
 */
export const TLX_ITEMS: SurveyItem[] = [
	{
		id: 'tlx_mental',
		type: 'slider100',
		label: 'How mentally demanding were the tasks?',
		lowAnchor: 'Very low',
		highAnchor: 'Very high',
		required: true
	},
	{
		id: 'tlx_physical',
		type: 'slider100',
		label: 'How physically demanding were the tasks?',
		lowAnchor: 'Very low',
		highAnchor: 'Very high',
		required: true
	},
	{
		id: 'tlx_temporal',
		type: 'slider100',
		label: 'How hurried or rushed was the pace of the tasks?',
		lowAnchor: 'Very low',
		highAnchor: 'Very high',
		required: true
	},
	{
		// Anchored Failure(low)→Perfect(high): high = good performance, opposite the
		// other five TLX items. Analysis must reverse-score this (101 - raw) before
		// it enters the NASA-TLX workload composite. See WELD_EXIT_SURVEY.md.
		id: 'tlx_performance',
		type: 'slider100',
		label: 'How successful were you in accomplishing what you were asked to do?',
		lowAnchor: 'Failure',
		highAnchor: 'Perfect',
		required: true
	},
	{
		id: 'tlx_effort',
		type: 'slider100',
		label: 'How hard did you have to work to accomplish your level of performance?',
		lowAnchor: 'Very low',
		highAnchor: 'Very high',
		required: true
	},
	{
		id: 'tlx_frustration',
		type: 'slider100',
		label: 'How insecure, discouraged, irritated, stressed, and annoyed were you?',
		lowAnchor: 'Very low',
		highAnchor: 'Very high',
		required: true
	}
];

/** Suffix distinguishing the post-baseline TLX rows from the exit (agent) ones. */
export const BASELINE_TLX_SUFFIX = '_baseline';

/**
 * The TLX items for the post-baseline interstitial: same questions, ids suffixed
 * so they store as separate rows (`tlx_mental_baseline`, …) alongside the exit
 * TLX without colliding on the `(participant_id, item_id)` key.
 */
export function baselineTlxItems(): SurveyItem[] {
	return TLX_ITEMS.map((it) => ({ ...it, id: `${it.id}${BASELINE_TLX_SUFFIX}` }));
}

export const SURVEY: SurveySection[] = [
	{
		title: 'Task load',
		instruction:
			'Think about the main tasks you just completed with the assistant. For each item, mark the point on the scale that best matches your experience.',
		items: TLX_ITEMS
	},
	{
		title: 'Usability',
		instruction:
			'For each statement, mark how much you agree or disagree, thinking about your overall experience with Weld today.',
		items: [
			{
				id: 'sus_01',
				type: 'likert5',
				label: SUS_LABEL('I think that I would like to use Weld frequently.'),
				required: true
			},
			{
				id: 'sus_02',
				type: 'likert5',
				label: SUS_LABEL('I found Weld unnecessarily complex.'),
				required: true
			},
			{
				id: 'sus_03',
				type: 'likert5',
				label: SUS_LABEL('I thought Weld was easy to use.'),
				required: true
			},
			{
				id: 'sus_04',
				type: 'likert5',
				label: SUS_LABEL(
					'I think that I would need the support of a technical person to be able to use Weld.'
				),
				required: true
			},
			{
				id: 'sus_05',
				type: 'likert5',
				label: SUS_LABEL('I found the various functions in Weld were well integrated.'),
				required: true
			},
			{
				id: 'sus_06',
				type: 'likert5',
				label: SUS_LABEL('I thought there was too much inconsistency in Weld.'),
				required: true
			},
			{
				id: 'sus_07',
				type: 'likert5',
				label: SUS_LABEL('I would imagine that most people would learn to use Weld very quickly.'),
				required: true
			},
			{
				id: 'sus_08',
				type: 'likert5',
				label: SUS_LABEL('I found Weld very cumbersome to use.'),
				required: true
			},
			{
				id: 'sus_09',
				type: 'likert5',
				label: SUS_LABEL('I felt very confident using Weld.'),
				required: true
			},
			{
				id: 'sus_10',
				type: 'likert5',
				label: SUS_LABEL('I needed to learn a lot of things before I could get going with Weld.'),
				required: true
			}
		]
	},
	{
		title: 'Working across the app',
		items: [
			{
				id: 'int_findacross',
				type: 'likert7',
				label:
					'When I needed to find information across different parts of Weld (board, chat, GitHub), it was easy.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'int_noticed',
				type: 'likert7',
				label:
					'I noticed moments where features in Weld were connected to each other in helpful ways.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'int_vs_separate',
				type: 'likert7',
				label:
					'Compared to my usual workflow of using separate apps for tasks, chat, and code, working in Weld felt:',
				lowAnchor: 'Much harder',
				highAnchor: 'Much easier',
				required: true
			},
			{
				id: 'int_reuse',
				type: 'likert7',
				label: 'I would consider using Weld for a real class or personal project.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'int_switch_perceived',
				type: 'likert7',
				label: 'While working, I felt like I was constantly switching between separate tools.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			}
		]
	},
	{
		title: 'The AI assistant',
		instruction: 'Thinking about the AI assistant you used during the tasks.',
		items: [
			{
				id: 'trust_reliable',
				type: 'likert7',
				label: 'The AI agent was reliable.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'trust_predictable',
				type: 'likert7',
				label: 'I could predict what the AI agent would do.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'trust_competent',
				type: 'likert7',
				label: 'The AI agent was capable of doing what I needed.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'trust_rely',
				type: 'likert7',
				label: 'I would be comfortable relying on the AI agent without double-checking its work.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'trust_context',
				type: 'likert7',
				label: 'The AI agent had enough context to actually help me.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'trust_again',
				type: 'likert7',
				label: 'I would want to use this AI agent for my own real work.',
				lowAnchor: 'Strongly disagree',
				highAnchor: 'Strongly agree',
				required: true
			},
			{
				id: 'trust_verify',
				type: 'likert7',
				label: 'I felt I had to check or redo what the agent did.',
				lowAnchor: 'Never',
				highAnchor: 'Constantly',
				required: true
			},
			{
				id: 'trust_speed_perceived',
				type: 'likert7',
				label: 'Using the agent made me faster.',
				lowAnchor: 'Much slower',
				highAnchor: 'Much faster',
				required: true
			}
		]
	},
	{
		title: 'In your own words',
		instruction: 'Optional, but we’d love your thoughts.',
		items: [
			{
				id: 'open_useful',
				type: 'text',
				label: 'What was the most useful moment of working in Weld today?'
			},
			{ id: 'open_frustrating', type: 'text', label: 'What was the most frustrating moment?' },
			{
				id: 'open_change',
				type: 'text',
				label: 'If you could change one thing about Weld, what would it be?'
			},
			{
				id: 'open_integration',
				type: 'text',
				label:
					'Did you notice the board, chat, and GitHub being connected? If so, was that helpful, distracting, or neither?'
			}
		]
	},
	{
		title: 'About you',
		items: [
			{
				id: 'dem_age',
				type: 'single',
				label: 'Age range',
				required: true,
				options: opts(['18–24', '25–34', '35–44', '45+', 'Prefer not to say'])
			},
			{
				id: 'dem_student',
				type: 'single',
				label: 'Are you currently a student?',
				required: true,
				options: opts(['Yes – undergraduate', 'Yes – graduate', 'No', 'Prefer not to say'])
			},
			{
				id: 'dem_experience',
				type: 'single',
				label: 'Years of programming experience',
				required: true,
				options: opts(['< 1', '1–2', '3–5', '6–10', '11+'])
			},
			{
				id: 'dem_languages',
				type: 'multi',
				label: 'Primary programming language(s) (select up to 3)',
				options: opts([
					'Python',
					'JavaScript/TypeScript',
					'Java',
					'C/C++',
					'C#',
					'Go',
					'Rust',
					'Swift/Kotlin',
					'Other'
				])
			},
			{
				id: 'dem_tools_pm',
				type: 'multi',
				label: 'Project/issue trackers you’ve used',
				options: opts(['Jira', 'Linear', 'GitHub Projects', 'Trello', 'Asana', 'None', 'Other'])
			},
			{
				id: 'dem_tools_chat',
				type: 'multi',
				label: 'Team chat tools you’ve used',
				options: opts(['Slack', 'Discord', 'MS Teams', 'None', 'Other'])
			},
			{
				id: 'dem_github_freq',
				type: 'single',
				label: 'How often do you use GitHub?',
				required: true,
				options: opts(['Daily', 'Weekly', 'Occasionally', 'Rarely or never'])
			},
			{
				id: 'dem_integrated_before',
				type: 'single',
				label:
					'Have you used an all-in-one dev workspace before (one app combining tasks + chat + code)?',
				required: true,
				options: opts(['Yes', 'No', 'Not sure'])
			}
		]
	},
	{
		title: 'Follow-up (optional)',
		items: [
			{
				id: 'followup_ok',
				type: 'single',
				label: 'Would you be willing to be contacted about future research related to this study?',
				options: opts(['Yes', 'No'])
			},
			{
				id: 'followup_contact',
				type: 'text',
				label:
					'If yes, an email to reach you (stored separately from your study data, used only to schedule).'
			}
		]
	}
];

function opts(labels: string[]): { value: string; label: string }[] {
	return labels.map((l) => ({ value: l, label: l }));
}
