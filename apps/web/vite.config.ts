import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	build: {
		// Ship source maps so production stack traces map back to original files.
		// They sit alongside the JS chunks and are only fetched by browsers when
		// devtools is open, so there's no perf cost for normal users.
		sourcemap: true
	},
	server: {
		proxy: {
			'/api': {
				target: process.env.VITE_API_PROXY_TARGET ?? 'http://localhost:3000',
				changeOrigin: true,
				ws: true
			}
		}
	},
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
