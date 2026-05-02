<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import favicon from '$lib/assets/favicon.svg';
	import Sidebar from '$lib/components/sidebar.svelte';
	import { auth } from '$lib/auth.svelte';
	import { projects } from '$lib/projects.svelte';
	import { ModeWatcher } from 'mode-watcher';

	let { children } = $props();

	const PUBLIC_ROUTES = new Set(['/login', '/register']);
	const isPublicRoute = $derived(PUBLIC_ROUTES.has(page.url.pathname));

	$effect(() => {
		if (!auth.ready) {
			void auth.bootstrap();
		}
	});

	$effect(() => {
		if (!auth.ready) return;
		if (!auth.isAuthenticated && !isPublicRoute) {
			void goto(resolve('/login'));
		} else if (auth.isAuthenticated && isPublicRoute) {
			void goto(resolve('/'));
		}
	});

	$effect(() => {
		if (!auth.ready) return;
		if (auth.isAuthenticated) {
			void projects.ensureLoaded();
		} else {
			projects.reset();
		}
	});
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

<ModeWatcher />

{#if !auth.ready}
	<div class="flex min-h-screen items-center justify-center text-sm text-[var(--muted-foreground)]">
		Loading…
	</div>
{:else if isPublicRoute}
	<div class="min-h-screen">
		{@render children()}
	</div>
{:else if auth.isAuthenticated}
	<div class="flex h-screen">
		<Sidebar />
		<main class="flex flex-1 flex-col overflow-y-auto p-8">
			{@render children()}
		</main>
	</div>
{/if}
