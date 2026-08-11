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
	/** Approving is a signal; merging is the act that ships it. Tracked
	 *  separately so the UI can offer the merge step only after an approval. */
	merged: boolean;
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

/** Short human label for a review's state, used on badges. */
export function verdictLabel(review: PrReview): string {
	if (review.merged) return 'Merged';
	return review.verdict === 'approve'
		? 'Approved'
		: review.verdict === 'request_changes'
			? 'Changes requested'
			: 'Commented';
}
