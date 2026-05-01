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
		FolderKanban
	} from 'lucide-svelte';
	import { mode, toggleMode } from 'mode-watcher';

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
</script>

<aside
	class="flex h-screen w-56 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--card)] px-3 py-4"
>
	<div class="mb-4 px-2 text-lg font-semibold tracking-tight">Weld</div>

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

	<nav class="flex flex-1 flex-col gap-1">
		{#each items as item (item.href)}
			{@const Icon = item.icon}
			<a
				href={resolve(item.href)}
				class={cn(
					'flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors',
					isActive(item.href)
						? 'bg-[var(--accent)] text-[var(--accent-foreground)]'
						: 'text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]'
				)}
			>
				<Icon class="h-4 w-4" />
				{item.label}
			</a>
		{/each}
	</nav>

	<div class="mt-3 flex flex-col gap-1 border-t border-[var(--border)] pt-3">
		{#if auth.user}
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
			class="flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
			aria-label="Toggle theme"
		>
			{#if mode.current === 'dark'}
				<Sun class="h-4 w-4" />
				Light mode
			{:else}
				<Moon class="h-4 w-4" />
				Dark mode
			{/if}
		</button>

		<button
			type="button"
			onclick={() => auth.logout()}
			class="flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
		>
			<LogOut class="h-4 w-4" />
			Sign out
		</button>
	</div>
</aside>
