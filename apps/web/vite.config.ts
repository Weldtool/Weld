import adapter from '@sveltejs/adapter-node';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			// adapter-auto only supports some environments, see https://svelte.dev/docs/kit/adapter-auto for a list.
			// If your environment is not supported, or you settled on a specific environment, switch out the adapter.
			// See https://svelte.dev/docs/kit/adapters for more information about adapters.
			adapter: adapter(),

			// SvelteKit 3 dropped the built-in `$lib` alias in favour of `#lib`
			// subpath imports. Kept as an alias so the upgrade didn't rewrite every
			// import. `alias` is deprecated (a build warning says so), so move to
			// `#lib` before a SvelteKit release removes it.
			alias: { $lib: 'src/lib' }
		})
	],
	build: {
		// Ship source maps so production stack traces map back to original files.
		// They sit alongside the JS chunks and are only fetched by browsers when
		// devtools is open, so there's no perf cost for normal users.
		sourcemap: true
	},
	server: {
		port: 3000,
		proxy: {
			'/api': {
				target: process.env.VITE_API_PROXY_TARGET ?? 'http://localhost:3001',
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
