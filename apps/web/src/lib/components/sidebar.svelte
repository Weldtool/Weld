<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { cn } from '$lib/utils';
	import {
		Kanban,
		MessageSquare,
		Clock,
		KeyRound,
		Settings,
		LayoutDashboard,
		Moon,
		Sun
	} from 'lucide-svelte';
	import { mode, toggleMode } from 'mode-watcher';

	const items = [
		{ href: '/', label: 'Dashboard', icon: LayoutDashboard },
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
</script>

<aside
	class="flex h-screen w-56 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--card)] px-3 py-4"
>
	<div class="mb-6 px-2 text-lg font-semibold tracking-tight">Weld</div>

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

	<button
		type="button"
		onclick={toggleMode}
		class="mt-3 flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
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
</aside>
