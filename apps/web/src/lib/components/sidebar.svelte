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
		LayoutDashboard,
		LogOut,
		Moon,
		Sun,
		FolderKanban,
		PanelLeftClose,
		PanelLeftOpen
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

	const items = [
		{ href: '/', label: 'Dashboard', icon: LayoutDashboard },
		{ href: '/projects', label: 'Projects', icon: FolderKanban },
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

	function onProjectChange(event: Event) {
		const value = (event.currentTarget as HTMLSelectElement).value;
		projects.select(value || null);
	}

	function onTeamChange(event: Event) {
		const value = (event.currentTarget as HTMLSelectElement).value;
		if (value) void projects.selectTeam(value);
	}
</script>

<aside
	class={cn(
		'flex h-screen shrink-0 flex-col border-r border-[var(--border)] bg-[var(--card)] py-4 transition-[width] duration-200',
		collapsed ? 'w-16 px-2' : 'w-56 px-3'
	)}
>
	<div class={cn('mb-4 flex items-center', collapsed ? 'justify-center' : 'justify-between px-2')}>
		{#if !collapsed}
			<span class="text-lg font-semibold tracking-tight">Weld</span>
		{/if}
		<button
			type="button"
			onclick={toggleCollapsed}
			class="flex h-8 w-8 items-center justify-center rounded-md text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
			aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
			title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			{#if collapsed}
				<PanelLeftOpen class="h-4 w-4" />
			{:else}
				<PanelLeftClose class="h-4 w-4" />
			{/if}
		</button>
	</div>

	{#if !collapsed && projects.teams.length > 0}
		<div class="mb-3 flex flex-col gap-1 px-1">
			<label for="team-switcher" class="px-1 text-xs text-[var(--muted-foreground)]">
				Workspace
			</label>
			<select
				id="team-switcher"
				value={projects.selectedTeamId ?? ''}
				onchange={onTeamChange}
				class="rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			>
				{#each projects.teams as team (team.id)}
					<option value={team.id}>{team.is_personal ? 'Personal' : team.name}</option>
				{/each}
			</select>
		</div>
	{/if}

	{#if !collapsed}
		<div class="mb-4 flex flex-col gap-1 px-1">
			<label for="project-switcher" class="px-1 text-xs text-[var(--muted-foreground)]">
				Active project
			</label>
			{#if projects.projects.length === 0}
				<a
					href={resolve('/projects')}
					class="rounded-md border border-dashed border-[var(--border)] px-3 py-2 text-center text-xs text-[var(--muted-foreground)] hover:border-[var(--ring)] hover:text-[var(--foreground)]"
				>
					No projects yet — create one
				</a>
			{:else}
				<select
					id="project-switcher"
					value={projects.selectedId ?? ''}
					onchange={onProjectChange}
					class="rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				>
					{#each projects.projects as project (project.id)}
						<option value={project.id}>{project.name}</option>
					{/each}
				</select>
			{/if}
		</div>
	{/if}

	<nav class="flex flex-1 flex-col gap-1">
		{#each items as item (item.href)}
			{@const Icon = item.icon}
			<a
				href={resolve(item.href)}
				title={collapsed ? item.label : undefined}
				aria-label={item.label}
				class={cn(
					'flex items-center gap-3 rounded-md py-2 text-sm font-medium transition-colors',
					collapsed ? 'justify-center px-0' : 'px-3',
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

	<div class="mt-3 flex flex-col gap-1 border-t border-[var(--border)] pt-3">
		{#if auth.user && !collapsed}
			<div class="px-3 py-1.5 text-xs">
				<div class="truncate font-medium text-[var(--foreground)]">
					{auth.user.display_name}
				</div>
				<div class="truncate text-[var(--muted-foreground)]">{auth.user.email}</div>
			</div>
		{/if}

		<button
			type="button"
			onclick={toggleMode}
			class={cn(
				'flex items-center gap-3 rounded-md py-2 text-sm font-medium text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]',
				collapsed ? 'justify-center px-0' : 'px-3'
			)}
			aria-label="Toggle theme"
			title={collapsed ? (mode.current === 'dark' ? 'Light mode' : 'Dark mode') : undefined}
		>
			{#if mode.current === 'dark'}
				<Sun class="h-4 w-4 shrink-0" />
				{#if !collapsed}Light mode{/if}
			{:else}
				<Moon class="h-4 w-4 shrink-0" />
				{#if !collapsed}Dark mode{/if}
			{/if}
		</button>

		<button
			type="button"
			onclick={() => auth.logout()}
			class={cn(
				'flex items-center gap-3 rounded-md py-2 text-sm font-medium text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]',
				collapsed ? 'justify-center px-0' : 'px-3'
			)}
			aria-label="Sign out"
			title={collapsed ? 'Sign out' : undefined}
		>
			<LogOut class="h-4 w-4 shrink-0" />
			{#if !collapsed}Sign out{/if}
		</button>
	</div>
</aside>
