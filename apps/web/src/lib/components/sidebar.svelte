<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { auth } from '$lib/auth.svelte';
	import { projects } from '$lib/projects.svelte';
	import { cn } from '$lib/utils';
	import {
		Kanban,
		MessageSquare,
		Clock,
		KeyRound,
		Settings,
		LogOut,
		Moon,
		Sun,
		PanelLeftClose,
		PanelLeftOpen,
		ChevronsUpDown,
		Check,
		Plus,
		Pencil,
		Trash2,
		FolderOpen
	} from 'lucide-svelte';
	import { mode, toggleMode } from 'mode-watcher';

	const COLLAPSED_KEY = 'weld:sidebar-collapsed';

	let collapsed = $state(false);

	$effect(() => {
		collapsed = localStorage.getItem(COLLAPSED_KEY) === '1';
	});

	function toggleCollapsed() {
		collapsed = !collapsed;
		localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : '0');
	}

	const navItems = [
		{ href: '/board', label: 'Board', icon: Kanban },
		{ href: '/chat', label: 'Chat', icon: MessageSquare },
		{ href: '/time', label: 'Time', icon: Clock },
		{ href: '/secrets', label: 'Secrets', icon: KeyRound },
		{ href: '/settings', label: 'Settings', icon: Settings }
	] as const;

	function isActive(href: string): boolean {
		if (href === '/') return page.url.pathname === '/';
		return page.url.pathname.startsWith(href);
	}

	// ── Workspace popover ────────────────────────────────────────────────────
	let workspaceOpen = $state(false);

	function teamInitial(name: string): string {
		return name.trim().charAt(0).toUpperCase();
	}

	const currentTeam = $derived(projects.team);

	// ── Create workspace ─────────────────────────────────────────────────────
	let showCreateWorkspace = $state(false);
	let newWorkspaceName = $state('');
	let createBusy = $state(false);
	let createError = $state<string | null>(null);
	let createInputEl: HTMLInputElement | null = $state(null);

	$effect(() => {
		if (showCreateWorkspace) createInputEl?.focus();
	});

	async function submitCreateWorkspace(e: SubmitEvent) {
		e.preventDefault();
		const name = newWorkspaceName.trim();
		if (!name) return;
		createBusy = true;
		createError = null;
		try {
			await projects.createTeam(name);
			newWorkspaceName = '';
			showCreateWorkspace = false;
			workspaceOpen = false;
		} catch (err) {
			createError = err instanceof Error ? err.message : 'Failed to create workspace.';
		} finally {
			createBusy = false;
		}
	}

	// ── Rename workspace ─────────────────────────────────────────────────────
	let showRenameWorkspace = $state(false);
	let renameWorkspaceName = $state('');
	let renameBusy = $state(false);
	let renameError = $state<string | null>(null);
	let renameInputEl: HTMLInputElement | null = $state(null);

	$effect(() => {
		if (showRenameWorkspace) renameInputEl?.focus();
	});

	function openRename() {
		renameWorkspaceName = currentTeam?.name ?? '';
		renameError = null;
		showRenameWorkspace = true;
		workspaceOpen = false;
	}

	async function submitRenameWorkspace(e: SubmitEvent) {
		e.preventDefault();
		const name = renameWorkspaceName.trim();
		if (!name || !currentTeam) return;
		renameBusy = true;
		renameError = null;
		try {
			await projects.updateTeam(currentTeam.id, name);
			showRenameWorkspace = false;
		} catch (err) {
			renameError = err instanceof Error ? err.message : 'Failed to rename workspace.';
		} finally {
			renameBusy = false;
		}
	}

	// ── Delete workspace ─────────────────────────────────────────────────────
	let showDeleteWorkspace = $state(false);
	let deleteWorkspaceBusy = $state(false);
	let deleteWorkspaceError = $state<string | null>(null);

	function openDeleteWorkspace() {
		deleteWorkspaceError = null;
		showDeleteWorkspace = true;
		workspaceOpen = false;
	}

	async function confirmDeleteWorkspace() {
		if (!currentTeam) return;
		deleteWorkspaceBusy = true;
		deleteWorkspaceError = null;
		try {
			await projects.deleteTeam(currentTeam.id);
			showDeleteWorkspace = false;
		} catch (err) {
			deleteWorkspaceError = err instanceof Error ? err.message : 'Failed to delete workspace.';
		} finally {
			deleteWorkspaceBusy = false;
		}
	}
</script>

<!-- Workspace popover click-away -->
{#if workspaceOpen}
	<button
		type="button"
		aria-label="Close workspace menu"
		class="fixed inset-0 z-10 cursor-default"
		onclick={() => (workspaceOpen = false)}
	></button>
{/if}

<aside
	class={cn(
		'relative flex h-screen shrink-0 flex-col border-r border-[var(--border)] bg-[var(--card)] transition-[width] duration-200',
		collapsed ? 'w-14 px-2 py-3' : 'w-56 px-2 py-3'
	)}
>
	<!-- ── Workspace header ──────────────────────────────────────────── -->
	<div class="relative mb-2">
		<button
			type="button"
			onclick={() => (workspaceOpen = !workspaceOpen)}
			class={cn(
				'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-[var(--accent)]',
				collapsed ? 'justify-center' : ''
			)}
			aria-label="Switch workspace"
			title={collapsed
				? currentTeam?.is_personal
					? 'Personal'
					: (currentTeam?.name ?? 'Workspace')
				: undefined}
		>
			<span
				class="flex h-6 w-6 shrink-0 items-center justify-center rounded bg-[var(--primary)] text-xs font-semibold text-[var(--primary-foreground)]"
			>
				{currentTeam ? teamInitial(currentTeam.is_personal ? 'Personal' : currentTeam.name) : 'W'}
			</span>
			{#if !collapsed}
				<span class="min-w-0 flex-1 truncate text-sm font-medium">
					{currentTeam?.is_personal ? 'Personal' : (currentTeam?.name ?? 'No workspace')}
				</span>
				<ChevronsUpDown class="h-3.5 w-3.5 shrink-0 text-[var(--muted-foreground)]" />
			{/if}
		</button>

		<!-- Workspace popover -->
		{#if workspaceOpen}
			<div
				class="absolute top-full left-0 z-20 mt-1 w-56 overflow-hidden rounded-lg border border-[var(--border)] bg-[var(--card)] py-1 shadow-lg"
			>
				<p class="px-3 pt-2 pb-1 text-xs font-medium text-[var(--muted-foreground)]">Workspaces</p>
				{#each projects.teams as team (team.id)}
					<button
						type="button"
						onclick={() => {
							void projects.selectTeam(team.id);
							workspaceOpen = false;
						}}
						class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-[var(--accent)]"
					>
						<span
							class="flex h-5 w-5 shrink-0 items-center justify-center rounded bg-[var(--primary)]/20 text-xs font-semibold text-[var(--primary)]"
						>
							{teamInitial(team.is_personal ? 'Personal' : team.name)}
						</span>
						<span class="min-w-0 flex-1 truncate">
							{team.is_personal ? 'Personal' : team.name}
						</span>
						{#if team.id === projects.selectedTeamId}
							<Check class="h-3.5 w-3.5 shrink-0 text-[var(--primary)]" />
						{/if}
					</button>
				{/each}

				<div class="my-1 border-t border-[var(--border)]"></div>

				{#if !showCreateWorkspace}
					<button
						type="button"
						onclick={() => (showCreateWorkspace = true)}
						class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-[var(--accent)]"
					>
						<Plus class="h-3.5 w-3.5 text-[var(--muted-foreground)]" />
						New workspace
					</button>
				{:else}
					<form onsubmit={submitCreateWorkspace} class="px-3 py-2">
						<input
							bind:this={createInputEl}
							type="text"
							bind:value={newWorkspaceName}
							placeholder="Workspace name"
							class="w-full rounded border border-[var(--input)] bg-[var(--background)] px-2 py-1 text-sm outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring)]"
						/>
						{#if createError}
							<p class="mt-1 text-xs text-red-500">{createError}</p>
						{/if}
						<div class="mt-2 flex gap-1.5">
							<button
								type="submit"
								disabled={createBusy}
								class="flex-1 rounded bg-[var(--primary)] px-2 py-1 text-xs font-medium text-[var(--primary-foreground)] disabled:opacity-50"
							>
								{createBusy ? 'Creating…' : 'Create'}
							</button>
							<button
								type="button"
								onclick={() => {
									showCreateWorkspace = false;
									newWorkspaceName = '';
									createError = null;
								}}
								class="flex-1 rounded border border-[var(--border)] px-2 py-1 text-xs hover:bg-[var(--accent)]"
							>
								Cancel
							</button>
						</div>
					</form>
				{/if}

				{#if currentTeam && !currentTeam.is_personal && (currentTeam.role === 'owner' || currentTeam.role === 'admin')}
					<button
						type="button"
						onclick={openRename}
						class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-[var(--accent)]"
					>
						<Pencil class="h-3.5 w-3.5 text-[var(--muted-foreground)]" />
						Rename workspace
					</button>
				{/if}

				{#if currentTeam && !currentTeam.is_personal && currentTeam.role === 'owner'}
					<button
						type="button"
						onclick={openDeleteWorkspace}
						class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-red-600 hover:bg-[var(--accent)] dark:text-red-400"
					>
						<Trash2 class="h-3.5 w-3.5" />
						Delete workspace
					</button>
				{/if}
			</div>
		{/if}
	</div>

	<!-- ── Projects list ─────────────────────────────────────────────── -->
	{#if !collapsed}
		<div class="mb-3">
			<div class="mb-1 flex items-center justify-between px-2">
				<span class="text-xs font-medium text-[var(--muted-foreground)]">Projects</span>
				<a
					href={resolve('/projects')}
					title="New project"
					aria-label="New project"
					class="rounded p-0.5 text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
				>
					<Plus class="h-3.5 w-3.5" />
				</a>
			</div>

			{#if projects.loading && projects.projects.length === 0}
				<p class="px-2 text-xs text-[var(--muted-foreground)]">Loading…</p>
			{:else if projects.projects.length === 0}
				<a
					href={resolve('/projects')}
					class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]"
				>
					<Plus class="h-3.5 w-3.5" />
					New project
				</a>
			{:else}
				{#each projects.projects as project (project.id)}
					<button
						type="button"
						onclick={() => projects.select(project.id)}
						class={cn(
							'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors',
							project.id === projects.selectedId
								? 'bg-[var(--accent)] font-medium text-[var(--accent-foreground)]'
								: 'text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--foreground)]'
						)}
					>
						<FolderOpen class="h-3.5 w-3.5 shrink-0" />
						<span class="min-w-0 flex-1 truncate text-xs">{project.name}</span>
					</button>
				{/each}
			{/if}
		</div>
	{/if}

	<!-- ── Navigation ────────────────────────────────────────────────── -->
	{#if !collapsed}
		<p class="mb-1 px-2 text-xs font-medium text-[var(--muted-foreground)]">Views</p>
	{/if}
	<nav class="flex flex-1 flex-col gap-0.5">
		{#each navItems as item (item.href)}
			{@const Icon = item.icon}
			<a
				href={resolve(item.href)}
				title={collapsed ? item.label : undefined}
				aria-label={item.label}
				class={cn(
					'flex items-center gap-2.5 rounded-md py-1.5 text-sm font-medium transition-colors',
					collapsed ? 'justify-center px-1' : 'px-2',
					isActive(item.href)
						? 'bg-[var(--accent)] text-[var(--accent-foreground)]'
						: 'text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]'
				)}
			>
				<Icon class="h-4 w-4 shrink-0" />
				{#if !collapsed}{item.label}{/if}
			</a>
		{/each}
	</nav>

	<!-- ── Footer ────────────────────────────────────────────────────── -->
	<div class="mt-3 flex flex-col gap-1 border-t border-[var(--border)] pt-3">
		{#if auth.user && !collapsed}
			<div class="mb-1 flex items-center gap-2 px-2">
				<span
					class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[var(--muted)] text-xs font-semibold"
				>
					{auth.user.display_name.charAt(0).toUpperCase()}
				</span>
				<div class="min-w-0 flex-1">
					<div class="truncate text-xs font-medium">{auth.user.display_name}</div>
					<div class="truncate text-xs text-[var(--muted-foreground)]">{auth.user.email}</div>
				</div>
			</div>
		{/if}

		<div class={cn('flex gap-1', collapsed ? 'flex-col items-center' : 'px-1')}>
			<button
				type="button"
				onclick={toggleMode}
				class="flex h-8 w-8 items-center justify-center rounded-md text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
				aria-label="Toggle theme"
				title={mode.current === 'dark' ? 'Light mode' : 'Dark mode'}
			>
				{#if mode.current === 'dark'}
					<Sun class="h-4 w-4" />
				{:else}
					<Moon class="h-4 w-4" />
				{/if}
			</button>

			<button
				type="button"
				onclick={() => auth.logout()}
				class="flex h-8 w-8 items-center justify-center rounded-md text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
				aria-label="Sign out"
				title="Sign out"
			>
				<LogOut class="h-4 w-4" />
			</button>

			{#if !collapsed}
				<div class="ml-auto">
					<button
						type="button"
						onclick={toggleCollapsed}
						class="flex h-8 w-8 items-center justify-center rounded-md text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
						aria-label="Collapse sidebar"
						title="Collapse sidebar"
					>
						<PanelLeftClose class="h-4 w-4" />
					</button>
				</div>
			{/if}
		</div>

		{#if collapsed}
			<button
				type="button"
				onclick={toggleCollapsed}
				class="mt-1 flex h-8 w-8 items-center justify-center rounded-md text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
				aria-label="Expand sidebar"
				title="Expand sidebar"
			>
				<PanelLeftOpen class="h-4 w-4" />
			</button>
		{/if}
	</div>
</aside>

<!-- ── Rename workspace dialog ───────────────────────────────────────────── -->
{#if showRenameWorkspace}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
		role="dialog"
		aria-modal="true"
		aria-labelledby="rename-workspace-title"
	>
		<div
			class="w-full max-w-sm rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-xl"
		>
			<h2 id="rename-workspace-title" class="text-base font-semibold">Rename workspace</h2>
			<form onsubmit={submitRenameWorkspace} class="mt-3">
				<input
					bind:this={renameInputEl}
					type="text"
					bind:value={renameWorkspaceName}
					placeholder="Workspace name"
					class="w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
				{#if renameError}
					<p class="mt-1.5 text-xs text-red-500">{renameError}</p>
				{/if}
				<div class="mt-4 flex justify-end gap-2">
					<button
						type="button"
						onclick={() => (showRenameWorkspace = false)}
						class="rounded-md border border-[var(--border)] px-3 py-1.5 text-sm hover:bg-[var(--accent)]"
					>
						Cancel
					</button>
					<button
						type="submit"
						disabled={renameBusy}
						class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-sm font-medium text-[var(--primary-foreground)] disabled:opacity-50"
					>
						{renameBusy ? 'Saving…' : 'Save'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

<!-- ── Delete workspace dialog ───────────────────────────────────────────── -->
{#if showDeleteWorkspace}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
		role="dialog"
		aria-modal="true"
		aria-labelledby="delete-workspace-title"
	>
		<div
			class="w-full max-w-sm rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-xl"
		>
			<h2 id="delete-workspace-title" class="text-base font-semibold">
				Delete "{currentTeam?.name}"?
			</h2>
			<p class="mt-2 text-sm text-[var(--muted-foreground)]">
				This will permanently delete the workspace and all its projects, tasks, and channels. This
				cannot be undone.
			</p>
			{#if deleteWorkspaceError}
				<p class="mt-2 text-xs text-red-500">{deleteWorkspaceError}</p>
			{/if}
			<div class="mt-4 flex justify-end gap-2">
				<button
					type="button"
					onclick={() => (showDeleteWorkspace = false)}
					class="rounded-md border border-[var(--border)] px-3 py-1.5 text-sm hover:bg-[var(--accent)]"
				>
					Cancel
				</button>
				<button
					type="button"
					onclick={confirmDeleteWorkspace}
					disabled={deleteWorkspaceBusy}
					class="rounded-md bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:opacity-90 disabled:opacity-50"
				>
					{deleteWorkspaceBusy ? 'Deleting…' : 'Delete workspace'}
				</button>
			</div>
		</div>
	</div>
{/if}
