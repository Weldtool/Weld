import { browser } from '$app/environment';
import { study as studyApi } from '$lib/api';
import { auth } from '$lib/auth.svelte';
import { STUDY_TASKS, type StudyTask } from '$lib/study-tasks';
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
	async begin(): Promise<void> {
		if (this.provisioning) return;
		this.provisioning = true;
		this.error = null;
		try {
			const res = await studyApi.provision({ consent_version: CONSENT_VERSION });
			await auth.adoptSession(res.access_token);
			this.participantCode = res.participant_code;
			this.agentCondition = res.agent_condition;
			this.taskIndex = 0;
			this.stage = 'tasks';
			this.#persist();
			telemetry.log('consent_given', { consent_version: CONSENT_VERSION });
			this.#enterTask(0);
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Could not start the study. Please retry.';
		} finally {
			this.provisioning = false;
		}
	}

	completeCurrentTask(): void {
		const task = this.currentTask;
		if (!task) return;
		telemetry.log('task_completed', { task_id: task.id });
		this.#advance(task);
	}

	skipCurrentTask(): void {
		const task = this.currentTask;
		if (!task) return;
		telemetry.log('task_skipped', { task_id: task.id });
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
		telemetry.setTask(task.id);
		telemetry.log('task_shown', { task_id: task.id });
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
			// Re-tag telemetry with the resumed task so events stay attributed.
			if (this.stage === 'tasks' && this.currentTask) {
				telemetry.setTask(this.currentTask.id);
			}
		} catch {
			// Corrupt state — start fresh.
		}
	}
}

export const study = new StudyStore();
