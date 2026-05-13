<script lang="ts">
	import { page } from '$app/state';
	import { github as githubApi } from '$lib/api';
	import type { GithubConnection } from '$lib/api/types';
	import { projects } from '$lib/projects.svelte';
	import { onMount } from 'svelte';
	import { GitBranch, Link, Unlink } from 'lucide-svelte';

	const teamId = $derived(projects.team?.id ?? null);

	// Team GitHub connection
	let connection = $state<GithubConnection | null>(null);
	let connectionLoading = $state(false);
	let connectionError = $state<string | null>(null);
	let disconnecting = $state(false);

	// User GitHub account link
	let linkingGitHub = $state(false);

	// Handle redirect callbacks
	const githubConnected = $derived(page.url.searchParams.get('github_connected') === 'true');
	const githubLinked = $derived(page.url.searchParams.get('github_linked') === 'true');

	async function loadConnection() {
		if (!teamId) return;
		connectionLoading = true;
		connectionError = null;
		try {
			connection = await githubApi.getConnection(teamId);
		} catch {
			connection = null;
		} finally {
			connectionLoading = false;
		}
	}

	async function connectGitHub() {
		if (!teamId) return;
		try {
			const { url } = await githubApi.getInstallUrl(teamId);
			window.location.href = url;
		} catch (e) {
			connectionError = e instanceof Error ? e.message : 'Failed to get install URL';
		}
	}

	async function disconnectGitHub() {
		if (!teamId) return;
		disconnecting = true;
		connectionError = null;
		try {
			await githubApi.disconnect(teamId);
			connection = null;
		} catch (e) {
			connectionError = e instanceof Error ? e.message : 'Failed to disconnect';
		} finally {
			disconnecting = false;
		}
	}

	async function linkGitHubAccount() {
		linkingGitHub = true;
		try {
			const { url } = await githubApi.getOAuthUrl();
			window.location.href = url;
		} catch {
			linkingGitHub = false;
		}
	}

	onMount(() => {
		void loadConnection();
	});
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Settings</h1>
	<p class="text-sm text-[var(--muted-foreground)]">Profile, team, and workspace preferences.</p>
</header>

{#if githubConnected}
	<div
		class="mb-6 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-600 dark:text-emerald-400"
	>
		GitHub App connected successfully.
	</div>
{/if}
{#if githubLinked}
	<div
		class="mb-6 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-600 dark:text-emerald-400"
	>
		GitHub account linked successfully.
	</div>
{/if}

<div class="flex flex-col gap-6">
	<!-- Team GitHub App connection -->
	<section
		class="flex flex-col gap-4 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6"
	>
		<div class="flex items-center gap-2">
			<GitBranch class="h-4 w-4 text-[var(--muted-foreground)]" />
			<h2 class="text-base font-semibold">GitHub App</h2>
		</div>
		<p class="text-sm text-[var(--muted-foreground)]">
			Connect a GitHub App installation to your team to receive webhook events and sync issues and
			pull requests.
		</p>

		{#if connectionError}
			<p
				class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
			>
				{connectionError}
			</p>
		{/if}

		{#if connectionLoading}
			<p class="text-sm text-[var(--muted-foreground)]">Loading…</p>
		{:else if connection}
			<div class="flex flex-col gap-3">
				<div
					class="flex items-center gap-3 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2.5 text-sm"
				>
					<span class="h-2 w-2 rounded-full bg-emerald-500"></span>
					<span>
						Connected to <span class="font-semibold">{connection.account_login}</span>
						{#if connection.repos.length > 0}
							· {connection.repos.length} repo{connection.repos.length === 1 ? '' : 's'}
						{/if}
					</span>
				</div>

				{#if connection.repos.length > 0}
					<div class="flex flex-wrap gap-1.5">
						{#each connection.repos as repo (repo)}
							<span
								class="rounded-full border border-[var(--border)] bg-[var(--muted)] px-2 py-0.5 text-xs"
							>
								{repo}
							</span>
						{/each}
					</div>
				{/if}

				<button
					type="button"
					onclick={disconnectGitHub}
					disabled={disconnecting}
					class="inline-flex w-fit items-center gap-1.5 rounded-md border border-red-500/40 px-3 py-2 text-sm font-medium text-red-600 hover:bg-red-500/10 disabled:opacity-60 dark:text-red-400"
				>
					<Unlink class="h-3.5 w-3.5" />
					{disconnecting ? 'Disconnecting…' : 'Disconnect GitHub'}
				</button>
			</div>
		{:else}
			<button
				type="button"
				onclick={connectGitHub}
				disabled={!teamId}
				class="inline-flex w-fit items-center gap-2 rounded-md bg-[#24292e] px-4 py-2 text-sm font-medium text-white hover:opacity-90 disabled:opacity-60"
			>
				<svg viewBox="0 0 16 16" class="h-4 w-4 fill-current" aria-hidden="true">
					<path
						d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
					/>
				</svg>
				Install GitHub App
			</button>
		{/if}
	</section>

	<!-- User GitHub account link -->
	<section
		class="flex flex-col gap-4 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6"
	>
		<div class="flex items-center gap-2">
			<Link class="h-4 w-4 text-[var(--muted-foreground)]" />
			<h2 class="text-base font-semibold">GitHub Account</h2>
		</div>
		<p class="text-sm text-[var(--muted-foreground)]">
			Link your personal GitHub account to sign in with GitHub and associate your commits with your
			Weld profile.
		</p>

		<button
			type="button"
			onclick={linkGitHubAccount}
			disabled={linkingGitHub}
			class="inline-flex w-fit items-center gap-2 rounded-md bg-[#24292e] px-4 py-2 text-sm font-medium text-white hover:opacity-90 disabled:opacity-60"
		>
			<svg viewBox="0 0 16 16" class="h-4 w-4 fill-current" aria-hidden="true">
				<path
					d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
				/>
			</svg>
			{linkingGitHub ? 'Redirecting…' : 'Link GitHub account'}
		</button>
	</section>
</div>
