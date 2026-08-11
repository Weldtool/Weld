import { config as configApi } from '$lib/api';

/**
 * Runtime app config fetched once from the backend's public `/api/v1/config`
 * endpoint. Holds non-sensitive feature flags — currently just `studyMode`,
 * which gates the research-study flow (see `paper/instrument/`). Load it early
 * (root layout) so gating decisions can read `appConfig.studyMode` synchronously.
 */
class AppConfigStore {
	studyMode = $state(false);
	/** When true, the study intro requires a single-use access code to begin. */
	requireAccessCode = $state(false);
	loaded = $state(false);
	#loading: Promise<void> | null = null;

	/** Idempotent: fetches once, subsequent calls await/return the same result. */
	async load(): Promise<void> {
		if (this.loaded) return;
		if (this.#loading) return this.#loading;

		this.#loading = (async () => {
			try {
				const cfg = await configApi.get();
				this.studyMode = cfg.study_mode;
				this.requireAccessCode = cfg.study_require_access_code;
				this.loaded = true;
			} finally {
				this.#loading = null;
			}
		})();

		return this.#loading;
	}
}

export const appConfig = new AppConfigStore();
