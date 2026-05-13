<script lang="ts">
	import { github as githubApi } from '$lib/api';
	import type { GitHubIssue, GitHubPR } from '$lib/api/types';
	import { GitPullRequest, CircleDot, RefreshCw, ChevronDown, ChevronRight } from 'lucide-svelte';

	interface Props {
		projectId: string;
	}
	const { projectId }: Props = $props();

	let issues = $state<GitHubIssue[]>([]);
	let prs = $state<GitHubPR[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let showIssues = $state(true);
	let showPRs = $state(true);

	async function load() {
		loading = true;
		error = null;
		try {
			[issues, prs] = await Promise.all([
				githubApi.listIssues(projectId),
				githubApi.listPRs(projectId)
			]);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load GitHub data';
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		if (projectId) void load();
	});
</script>

<!-- eslint-disable svelte/no-navigation-without-resolve -->
<div class="flex flex-col gap-3">
	<div class="flex items-center justify-between">
		<span class="text-xs font-semibold tracking-wide text-[var(--muted-foreground)] uppercase">
			GitHub
		</span>
		<button
			type="button"
			onclick={load}
			disabled={loading}
			aria-label="Refresh"
			class="rounded p-1 text-[var(--muted-foreground)] hover:bg-[var(--accent)] disabled:opacity-50"
		>
			<RefreshCw class="h-3 w-3 {loading ? 'animate-spin' : ''}" />
		</button>
	</div>

	{#if error}
		<p class="text-xs text-red-500">{error}</p>
	{:else}
		<!-- Pull Requests -->
		<div>
			<button
				type="button"
				onclick={() => (showPRs = !showPRs)}
				class="flex w-full items-center gap-1 text-xs font-medium text-[var(--foreground)] hover:text-[var(--primary)]"
			>
				{#if showPRs}
					<ChevronDown class="h-3 w-3 shrink-0" />
				{:else}
					<ChevronRight class="h-3 w-3 shrink-0" />
				{/if}
				<GitPullRequest class="h-3 w-3 shrink-0" />
				Pull Requests
				<span class="ml-auto text-[var(--muted-foreground)]">{prs.length}</span>
			</button>

			{#if showPRs}
				<ul class="mt-1.5 flex flex-col gap-1">
					{#if prs.length === 0 && !loading}
						<li class="text-xs text-[var(--muted-foreground)] italic">No open pull requests</li>
					{/if}
					{#each prs as pr (pr.number)}
						<li>
							<a
								href={pr.html_url}
								target="_blank"
								rel="noopener noreferrer"
								class="flex items-start gap-1.5 rounded px-1.5 py-1 text-xs hover:bg-[var(--accent)]"
							>
								<span class="mt-0.5 shrink-0 text-[var(--muted-foreground)]">#{pr.number}</span>
								<span
									class="flex-1 leading-snug {pr.draft ? 'text-[var(--muted-foreground)]' : ''}"
								>
									{pr.title}
									{#if pr.draft}
										<span class="ml-1 text-[10px] opacity-70">(draft)</span>
									{/if}
								</span>
							</a>
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		<!-- Issues -->
		<div>
			<button
				type="button"
				onclick={() => (showIssues = !showIssues)}
				class="flex w-full items-center gap-1 text-xs font-medium text-[var(--foreground)] hover:text-[var(--primary)]"
			>
				{#if showIssues}
					<ChevronDown class="h-3 w-3 shrink-0" />
				{:else}
					<ChevronRight class="h-3 w-3 shrink-0" />
				{/if}
				<CircleDot class="h-3 w-3 shrink-0" />
				Issues
				<span class="ml-auto text-[var(--muted-foreground)]">{issues.length}</span>
			</button>

			{#if showIssues}
				<ul class="mt-1.5 flex flex-col gap-1">
					{#if issues.length === 0 && !loading}
						<li class="text-xs text-[var(--muted-foreground)] italic">No open issues</li>
					{/if}
					{#each issues as issue (issue.number)}
						<li>
							<a
								href={issue.html_url}
								target="_blank"
								rel="noopener noreferrer"
								class="flex items-start gap-1.5 rounded px-1.5 py-1 text-xs hover:bg-[var(--accent)]"
							>
								<span class="mt-0.5 shrink-0 text-[var(--muted-foreground)]">#{issue.number}</span>
								<span class="flex-1 leading-snug">{issue.title}</span>
							</a>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>
