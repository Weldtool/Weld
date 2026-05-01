<script lang="ts">
	const cards = [
		{ title: 'Active tasks', value: '–', hint: 'Coming with the board' },
		{ title: 'Unread messages', value: '–', hint: 'Coming with chat' },
		{ title: 'Tracked today', value: '–', hint: 'Coming with time tracking' }
	];
</script>

<header class="mb-8">
	<h1 class="text-2xl font-semibold tracking-tight">Dashboard</h1>
	<p class="text-sm text-[var(--muted-foreground)]">
		A quick view of what's happening across Weld.
	</p>
</header>

<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
	{#each cards as card (card.title)}
		<div
			class="rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 text-[var(--card-foreground)]"
		>
			<div class="text-sm font-medium text-[var(--muted-foreground)]">{card.title}</div>
			<div class="mt-2 text-3xl font-semibold">{card.value}</div>
			<div class="mt-1 text-xs text-[var(--muted-foreground)]">{card.hint}</div>
		</div>
	{/each}
</div>
