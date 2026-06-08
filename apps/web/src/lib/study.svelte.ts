import { browser } from '$app/environment';
import { study as studyApi } from '$lib/api';
import { auth } from '$lib/auth.svelte';
import { projects } from '$lib/projects.svelte';
import { STUDY_TASKS, type StudyTask } from '$lib/study-tasks';
import { buildTaskSnapshot } from '$lib/study-snapshot';
import { telemetry } from '$lib/telemetry.svelte';

/**
 * Drives the participant through the study as a stage machine, rendered as an
 * overlay by the root layout (so it doesn't fight the normal route guard):
 *
 *   intro → tasks → survey → done        (ineligible = polite exit)
 *
 * Stage + progress persist in localStorage so a refresh resumes where the
 * participant left off (their session itself is recovered via the refresh
 * cookie by auth.bootstrap). Only meaningful when the backend is in study mode.
 */
export const CONSENT_VERSION = 'v1.0';

export type StudyStage = 'intro' | 'tasks' | 'survey' | 'done' | 'ineligible';

const STORAGE_KEY = 'weld:study';

interface PersistedState {
	stage: StudyStage;
	taskIndex: number;
	participantCode: string | null;
	agentCondition: string | null;
}

class StudyStore {
	stage = $state<StudyStage>('intro');
	taskIndex = $state(0);
	participantCode = $state<string | null>(null);
	agentCondition = $state<string | null>(null);
	provisioning = $state(false);
	error = $state<string | null>(null);
	/** The just-completed task awaiting its quick per-task rating, or null. While
	 * set, the task panel shows the rating step and advancement is gated on it. */
	ratingFor = $state<StudyTask | null>(null);
	#taskStartedAt: number | null = null;

	constructor() {
		if (browser) this.#restore();
	}

	get tasks(): StudyTask[] {
		return STUDY_TASKS;
	}
	get currentTask(): StudyTask | null {
		return STUDY_TASKS[this.taskIndex] ?? null;
	}
	get totalTasks(): number {
		return STUDY_TASKS.length;
	}
	get isLastTask(): boolean {
		return this.taskIndex >= STUDY_TASKS.length - 1;
	}

	/** Route an ineligible respondent to the polite-exit screen. No data stored. */
	markIneligible(): void {
		this.stage = 'ineligible';
		this.#persist();
	}

	/** After consent + screening: provision the isolated workspace and start. */
	async begin(opts: {
		accessCode?: string;
		participantEmail?: string;
		institution?: string;
	} = {}): Promise<void> {
		if (this.provisioning) return;
		this.provisioning = true;
		this.error = null;
		try {
			const res = await studyApi.provision({
				consent_version: CONSENT_VERSION,
				access_code: opts.accessCode?.trim() || undefined,
				participant_email: opts.participantEmail?.trim() || undefined,
				institution: opts.institution?.trim() || undefined
			});
			await auth.adoptSession(res.access_token);
			// Clear any stale project/team data from a previous session then
			// eagerly load the participant's freshly-provisioned workspace. This
			// ensures projects.current is non-null before the board renders, so
			// the participant doesn't see "No active project" on first load.
			projects.reset();
			await projects.ensureLoaded();
			this.participantCode = res.participant_code;
			this.agentCondition = res.agent_condition;

			if (res.resumed) {
				// Returning participant re-entered their code (new device or cleared
				// storage): restore their saved place instead of starting over.
				this.#resumeFrom(res.resume_stage, res.resume_task_index);
			} else {
				this.taskIndex = 0;
				this.stage = 'tasks';
				this.#persist();
				telemetry.markInProgress(true);
				telemetry.log('consent_given', { consent_version: CONSENT_VERSION });
				this.#enterTask(0);
			}
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Could not start the study. Please retry.';
		} finally {
			this.provisioning = false;
		}
	}

	completeCurrentTask(): void {
		const task = this.currentTask;
		if (!task) return;
		const taskTimeMs = this.#taskStartedAt !== null ? Date.now() - this.#taskStartedAt : null;
		telemetry.log('task_final_state', buildTaskSnapshot());
		telemetry.log('task_completed', {
			task_id: task.id,
			task_type: task.type,
			task_time_ms: taskTimeMs
		});
		// Clock already stopped above — gate advancement on a quick per-task rating
		// so it can never inflate the measured task time.
		this.ratingFor = task;
	}

	/** Record the per-task rating (effort, and agent-help on agent tasks), then
	 * advance. `agentHelp` is null for baseline tasks (no agent). */
	submitTaskRating(effort: number, agentHelp: number | null): void {
		const task = this.ratingFor;
		if (!task) return;
		telemetry.log('task_rating', {
			task_id: task.id,
			task_type: task.type,
			phase: task.phase,
			effort,
			agent_help: agentHelp
		});
		this.ratingFor = null;
		this.#advance(task);
	}

	skipCurrentTask(): void {
		const task = this.currentTask;
		if (!task) return;
		const taskTimeMs = this.#taskStartedAt !== null ? Date.now() - this.#taskStartedAt : null;
		telemetry.log('task_final_state', buildTaskSnapshot());
		telemetry.log('task_skipped', {
			task_id: task.id,
			task_type: task.type,
			task_time_ms: taskTimeMs
		});
		this.#advance(task);
	}

	/** Participant expanded/reopened the instruction — a confusion proxy (§2.3). */
	noteInstructionReopened(): void {
		const task = this.currentTask;
		if (task) telemetry.log('instruction_reopened', { task_id: task.id });
	}

	/** Survey component calls this when it mounts. */
	noteSurveyStarted(): void {
		telemetry.log('survey_started', {});
	}

	/** Survey submitted successfully → final screen. */
	finishSurvey(): void {
		telemetry.markInProgress(false);
		telemetry.log('survey_completed', {});
		this.stage = 'done';
		this.#persist();
		void telemetry.end();
	}

	#advance(completed: StudyTask): void {
		if (this.isLastTask) {
			telemetry.log('phase_ended', { phase: completed.phase });
			telemetry.setTask(null);
			this.stage = 'survey';
			this.#persist();
			return;
		}
		const next = this.taskIndex + 1;
		const nextTask = STUDY_TASKS[next];
		const phaseChanged = nextTask.phase !== completed.phase;
		if (phaseChanged) telemetry.log('phase_ended', { phase: completed.phase });
		this.taskIndex = next;
		this.#persist();
		this.#enterTask(next);
	}

	#enterTask(index: number): void {
		const task = STUDY_TASKS[index];
		if (!task) return;
		// Emit phase_started at the first task of a phase.
		if (index === 0 || STUDY_TASKS[index - 1].phase !== task.phase) {
			telemetry.log('phase_started', { phase: task.phase });
		}
		this.#taskStartedAt = Date.now();
		telemetry.setTask(task.id);
		telemetry.log('task_shown', { task_id: task.id, task_type: task.type });
	}

	/** Restore a resumed participant into their saved stage/task. */
	#resumeFrom(stage: string | null, taskIndex: number): void {
		const validStages: StudyStage[] = ['tasks', 'survey', 'done'];
		const resumed = (validStages as string[]).includes(stage ?? '')
			? (stage as StudyStage)
			: 'tasks';
		this.stage = resumed;
		this.taskIndex = Math.min(Math.max(taskIndex, 0), STUDY_TASKS.length - 1);
		this.#persist();
		telemetry.log('session_resumed', { stage: resumed, task_index: this.taskIndex });
		if (resumed === 'tasks' && this.currentTask) {
			this.#taskStartedAt = Date.now();
			telemetry.setTask(this.currentTask.id);
		}
		if (resumed === 'tasks' || resumed === 'survey') {
			telemetry.markInProgress(true);
		}
	}

	#persist(): void {
		if (!browser) return;
		const data: PersistedState = {
			stage: this.stage,
			taskIndex: this.taskIndex,
			participantCode: this.participantCode,
			agentCondition: this.agentCondition
		};
		localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
		this.#syncProgress();
	}

	/** Mirror progress to the backend (fire-and-forget) so it survives a cleared
	 * browser / new device. Only meaningful once a session exists (post-provision
	 * stages); failures are non-fatal — localStorage remains the fast path. */
	#syncProgress(): void {
		if (!browser) return;
		if (this.stage !== 'tasks' && this.stage !== 'survey' && this.stage !== 'done') return;
		void studyApi.progress({ stage: this.stage, task_index: this.taskIndex }).catch(() => {});
	}

	#restore(): void {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return;
		try {
			const data = JSON.parse(raw) as PersistedState;
			this.stage = data.stage ?? 'intro';
			this.taskIndex = data.taskIndex ?? 0;
			this.participantCode = data.participantCode ?? null;
			this.agentCondition = data.agentCondition ?? null;
			// Re-tag telemetry with the resumed task so events stay attributed, and
			// re-arm the dropout marker while mid-study (tasks/survey).
			if (this.stage === 'tasks' && this.currentTask) {
				telemetry.setTask(this.currentTask.id);
			}
			if (this.stage === 'tasks' || this.stage === 'survey') {
				telemetry.markInProgress(true);
			}
		} catch {
			// Corrupt state — start fresh.
		}
	}
}

export const study = new StudyStore();
