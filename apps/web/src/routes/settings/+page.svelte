<script lang="ts">
	import { page } from '$app/state';
	import { github as githubApi, teams as teamsApi } from '$lib/api';
	import type { GithubConnection, TeamMember, TeamRole } from '$lib/api/types';
	import { auth } from '$lib/auth.svelte';
	import { projects } from '$lib/projects.svelte';
	import { GitBranch, Link, LogOut, Unlink, User, Users, X } from '@lucide/svelte';

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

	let loadedForTeam = $state<string | null>(null);
	$effect(() => {
		if (teamId && teamId !== loadedForTeam) {
			loadedForTeam = teamId;
			void loadConnection();
			void loadMembers();
		}
	});

	// ── Team members ────────────────────────────────────────────────────────

	let members = $state<TeamMember[]>([]);
	let membersLoading = $state(false);
	let membersError = $state<string | null>(null);

	let inviteEmail = $state('');
	let inviteRole = $state<TeamRole>('member');
	let inviting = $state(false);

	const role = $derived(projects.team?.role ?? 'viewer');
	const canManageMembers = $derived(role === 'owner' || role === 'admin');
	const ownerId = $derived(projects.team?.owner_id ?? null);
	const selfId = $derived(auth.user?.id ?? null);

	async function loadMembers() {
		if (!teamId) return;
		membersLoading = true;
		membersError = null;
		try {
			members = await teamsApi.listMembers(teamId);
		} catch (e) {
			membersError = e instanceof Error ? e.message : 'Failed to load team members.';
		} finally {
			membersLoading = false;
		}
	}

	async function inviteMember() {
		if (!teamId) return;
		const email = inviteEmail.trim();
		if (!email) return;
		inviting = true;
		membersError = null;
		try {
			const added = await teamsApi.addMember(teamId, { email, role: inviteRole });
			// Insert sorted by display_name to match server ordering.
			members = [...members, added].sort((a, b) => a.display_name.localeCompare(b.display_name));
			inviteEmail = '';
			inviteRole = 'member';
		} catch (e) {
			membersError = e instanceof Error ? e.message : 'Failed to add member.';
		} finally {
			inviting = false;
		}
	}

	async function removeMember(userId: string) {
		if (!teamId) return;
		membersError = null;
		try {
			await teamsApi.removeMember(teamId, userId);
			members = members.filter((m) => m.user_id !== userId);
		} catch (e) {
			membersError = e instanceof Error ? e.message : 'Failed to remove member.';
		}
	}
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Settings</h1>
	<p class="text-sm text-[var(--muted-foreground)]">Profile and workspace preferences.</p>
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
	<!-- Profile / account -->
	<section
		class="flex flex-col gap-4 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6"
	>
		<div class="flex items-center gap-2">
			<User class="h-4 w-4 text-[var(--muted-foreground)]" />
			<h2 class="text-base font-semibold">Account</h2>
		</div>

		{#if auth.user}
			<dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
				<dt class="text-[var(--muted-foreground)]">Name</dt>
				<dd>{auth.user.display_name}</dd>
				<dt class="text-[var(--muted-foreground)]">Email</dt>
				<dd>{auth.user.email}</dd>
			</dl>
		{/if}

		<div class="flex items-center justify-between border-t border-[var(--border)] pt-4">
			<div>
				<p class="text-sm font-medium">Sign out</p>
				<p class="text-xs text-[var(--muted-foreground)]">
					End your session on this device. You'll need to sign in again to access your workspace.
				</p>
			</div>
			<button
				type="button"
				onclick={() => auth.logout()}
				class="inline-flex items-center gap-1.5 rounded-md border border-red-500/40 px-3 py-2 text-sm font-medium text-red-600 hover:bg-red-500/10 dark:text-red-400"
			>
				<LogOut class="h-3.5 w-3.5" />
				Sign out
			</button>
		</div>
	</section>

	<!-- Team members -->
	<section
		class="flex flex-col gap-4 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6"
	>
		<div class="flex items-center gap-2">
			<Users class="h-4 w-4 text-[var(--muted-foreground)]" />
			<h2 class="text-base font-semibold">Workspace members</h2>
			{#if projects.team}
				<span class="ml-1 text-xs text-[var(--muted-foreground)]">· {projects.team.name}</span>
			{/if}
		</div>

		{#if membersError}
			<p
				class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
			>
				{membersError}
			</p>
		{/if}

		{#if !teamId}
			<p class="text-sm text-[var(--muted-foreground)]">
				No active workspace. Create or pick one to manage members.
			</p>
		{:else if membersLoading}
			<p class="text-sm text-[var(--muted-foreground)]">Loading…</p>
		{:else}
			<ul class="divide-y divide-[var(--border)]">
				{#each members as m (m.user_id)}
					<li class="flex items-center gap-3 py-2 text-sm">
						{#if m.avatar_url}
							<img src={m.avatar_url} alt="" class="h-7 w-7 rounded-full" />
						{:else}
							<span
								class="grid h-7 w-7 place-items-center rounded-full bg-[var(--muted)] text-xs font-medium text-[var(--muted-foreground)]"
							>
								{m.display_name.charAt(0).toUpperCase()}
							</span>
						{/if}
						<div class="min-w-0 flex-1">
							<div class="truncate font-medium">{m.display_name}</div>
							<div class="truncate text-xs text-[var(--muted-foreground)]">{m.email}</div>
						</div>
						<span
							class="rounded-full border border-[var(--border)] bg-[var(--muted)] px-2 py-0.5 text-xs"
						>
							{m.role}{m.user_id === ownerId ? ' (owner)' : ''}
						</span>
						{#if canManageMembers && m.user_id !== ownerId && m.user_id !== selfId}
							<button
								type="button"
								onclick={() => removeMember(m.user_id)}
								aria-label="Remove {m.display_name}"
								title="Remove member"
								class="rounded p-1 text-[var(--muted-foreground)] hover:bg-red-500/10 hover:text-red-600 dark:hover:text-red-400"
							>
								<X class="h-4 w-4" />
							</button>
						{/if}
					</li>
				{/each}
			</ul>

			{#if canManageMembers}
				<form
					class="flex flex-col gap-2 border-t border-[var(--border)] pt-4 sm:flex-row sm:items-end"
					onsubmit={(e) => {
						e.preventDefault();
						void inviteMember();
					}}
				>
					<div class="flex-1">
						<label for="invite-email" class="mb-1 block text-xs text-[var(--muted-foreground)]">
							Add by email
						</label>
						<input
							id="invite-email"
							type="email"
							bind:value={inviteEmail}
							placeholder="teammate@example.com"
							required
							class="w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
						/>
					</div>
					<div>
						<label for="invite-role" class="mb-1 block text-xs text-[var(--muted-foreground)]">
							Role
						</label>
						<select
							id="invite-role"
							bind:value={inviteRole}
							class="rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm"
						>
							<option value="viewer">viewer</option>
							<option value="member">member</option>
							<option value="admin">admin</option>
						</select>
					</div>
					<button
						type="submit"
						disabled={inviting || !inviteEmail.trim()}
						class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-sm font-medium text-[var(--primary-foreground)] disabled:opacity-50"
					>
						{inviting ? 'Adding…' : 'Add member'}
					</button>
				</form>
				<p class="text-[11px] text-[var(--muted-foreground)]">
					The user must already have a Weld account; we look them up by email.
				</p>
			{/if}
		{/if}
	</section>

	<!-- Team GitHub App connection -->
	<section
		class="flex flex-col gap-4 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6"
	>
		<div class="flex items-center gap-2">
			<GitBranch class="h-4 w-4 text-[var(--muted-foreground)]" />
			<h2 class="text-base font-semibold">GitHub App</h2>
		</div>
		<p class="text-sm text-[var(--muted-foreground)]">
			Connect a GitHub App installation to your workspace to receive webhook events and sync issues
			and pull requests.
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
					</span>
				</div>
				<p class="text-xs text-[var(--muted-foreground)]">
					This installs the GitHub App at the workspace level. Each project picks its own repo in
					project settings.
				</p>

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
