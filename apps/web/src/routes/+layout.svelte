<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import favicon from '$lib/assets/favicon.svg';
	import Sidebar from '$lib/components/sidebar.svelte';
	import { auth } from '$lib/auth.svelte';
	import { appConfig } from '$lib/config.svelte';
	import { projects } from '$lib/projects.svelte';
	import { realtime } from '$lib/realtime.svelte';
	import { telemetry } from '$lib/telemetry.svelte';
	import { study } from '$lib/study.svelte';
	import StudyIntro from '$lib/components/study/StudyIntro.svelte';
	import StudyDone from '$lib/components/study/StudyDone.svelte';
	import TaskPanel from '$lib/components/study/TaskPanel.svelte';
	import AgentPanel from '$lib/components/study/AgentPanel.svelte';
	import ExitSurvey from '$lib/components/study/ExitSurvey.svelte';
	import { ModeWatcher } from 'mode-watcher';

	/** Coarse view name for the `view_changed` telemetry metric. */
	function viewOf(pathname: string): string {
		if (pathname.startsWith('/board')) return 'board';
		if (pathname.startsWith('/chat')) return 'chat';
		if (pathname.startsWith('/projects')) return 'project_settings';
		if (pathname.startsWith('/settings')) return 'settings';
		if (pathname.startsWith('/time')) return 'time';
		if (pathname.startsWith('/secrets')) return 'secrets';
		if (pathname === '/' || pathname === '') return 'home';
		return 'other';
	}
	let prevView: string | null = null;

	let { children } = $props();

	const PUBLIC_ROUTES = new Set(['/login', '/register']);
	const isPublicRoute = $derived(PUBLIC_ROUTES.has(page.url.pathname));

	$effect(() => {
		if (!auth.ready) {
			void auth.bootstrap();
		}
	});

	$effect(() => {
		if (!appConfig.loaded) {
			void appConfig.load();
		}
	});

	$effect(() => {
		if (!auth.ready) return;
		// In study mode the stage machine owns navigation (see below) — skip the
		// normal login/route guard entirely.
		if (appConfig.studyMode) return;
		if (!auth.isAuthenticated && !isPublicRoute) {
			void goto(resolve('/login'));
		} else if (auth.isAuthenticated && isPublicRoute) {
			void goto(resolve('/'));
		}
	});

	// During the tasks stage, keep participants in the working app (board/chat).
	// Bounce them off the dashboard/auth pages onto the board so the panel has a
	// real workspace behind it.
	$effect(() => {
		if (!appConfig.studyMode || study.stage !== 'tasks' || !auth.isAuthenticated) return;
		const p = page.url.pathname;
		if (p === '/' || p === '/login' || p === '/register') {
			void goto(resolve('/board'));
		}
	});

	$effect(() => {
		if (!auth.ready) return;
		if (auth.isAuthenticated) {
			void projects.ensureLoaded();
			if (auth.accessToken) realtime.connect(auth.accessToken);
		} else {
			projects.reset();
			realtime.disconnect();
		}
	});

	// Study telemetry: in study mode every authenticated user is a participant.
	// Start the session once authenticated; view_changed tracks navigation.
	$effect(() => {
		if (appConfig.studyMode && auth.ready && auth.isAuthenticated) {
			telemetry.start();
		}
	});

	$effect(() => {
		if (!appConfig.studyMode || !auth.isAuthenticated) return;
		const view = viewOf(page.url.pathname);
		if (view !== prevView) {
			telemetry.log('view_changed', { from: prevView, to: view });
			prevView = view;
		}
	});
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

<ModeWatcher />

{#if appConfig.studyMode}
	<!-- Research-study flow: the stage machine decides what the participant sees,
	     layered over the normal app so it never fights the route guard. -->
	{#if study.stage === 'intro'}
		<StudyIntro />
	{:else if study.stage === 'ineligible' || study.stage === 'done'}
		<StudyDone />
	{:else if !auth.ready}
		<div
			class="flex min-h-screen items-center justify-center text-sm text-[var(--muted-foreground)]"
		>
			Loading…
		</div>
	{:else if study.stage === 'survey'}
		<ExitSurvey />
	{:else}
		<!-- tasks: the real workspace with the task panel alongside -->
		<div class="flex h-screen">
			<Sidebar />
			<main class="flex flex-1 flex-col overflow-y-auto p-8">
				{@render children()}
			</main>
			<!-- Agent docked on the right, present only for the agent-task phase;
			     absent during C0 baseline. -->
			{#if study.currentTask?.phase === 'agent'}
				<AgentPanel />
			{/if}
		</div>
		<TaskPanel />
	{/if}
{:else if !auth.ready}
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
