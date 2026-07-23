/**
 * Session-scoped record of which pull requests the participant has reviewed, so
 * a "Reviewed" badge shows on the PR list and re-opening the review view
 * restores their verdict. In-memory only (resets on a full reload); the durable
 * record is the `pr_review_submitted` telemetry event. Deliberately not
 * persisted to localStorage — that would leak one participant's reviews into the
 * next session on a shared machine.
 */
export type ReviewVerdict = 'approve' | 'comment' | 'request_changes';

export interface PrReview {
	verdict: ReviewVerdict;
	comment: string;
}

class PrReviews {
	#byNumber = $state<Record<number, PrReview>>({});

	get(number: number): PrReview | null {
		return this.#byNumber[number] ?? null;
	}

	set(number: number, review: PrReview): void {
		this.#byNumber = { ...this.#byNumber, [number]: review };
	}
}

export const prReviews = new PrReviews();

/** Short human label for a verdict, used on badges. */
export function verdictLabel(v: ReviewVerdict): string {
	return v === 'approve' ? 'Approved' : v === 'request_changes' ? 'Changes requested' : 'Commented';
}
