import { defineEnvVars } from '@sveltejs/kit/env';

// Read at runtime (the old `$env/dynamic/public`). Optional on purpose: when
// unset, API and WebSocket calls go to the same origin and the dev proxy or
// reverse proxy forwards them.
export const variables = defineEnvVars({
	PUBLIC_API_BASE_URL: { public: true, schema: (input) => input }
});
