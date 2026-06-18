<script lang="ts">
	import type { GitHubMessageMetadata } from '$lib/api/types';
	import { GitPullRequest, CircleDot, Eye, GitMerge } from 'lucide-svelte';
	import { appConfig } from '$lib/config.svelte';

	interface Props {
		meta: GitHubMessageMetadata;
	}
	const { meta }: Props = $props();

	// In study mode the linked PR/issue is a seeded fixture with no real GitHub
	// page, so suppress the outbound link to avoid sending participants to a 404.
	const linksDisabled = $derived(appConfig.studyMode);
	const linkHref = $derived(linksDisabled ? undefined : meta.url);
	const linkTarget = $derived(linksDisabled ? undefined : '_blank');
	const linkClass = $derived(linksDisabled ? '' : 'hover:underline');

	const kindLabel: Record<string, string> = {
		'pr.opened': 'opened',
		'pr.merged': 'merged',
		'pr.closed': 'closed',
		'pr.reopened': 'reopened',
		'issue.opened': 'opened',
		'issue.closed': 'closed',
		'issue.reopened': 'reopened',
		'review.submitted': 'reviewed'
	};

	const isPR = $derived(meta.kind.startsWith('pr.'));
	const isIssue = $derived(meta.kind.startsWith('issue.'));
	const isReview = $derived(meta.kind === 'review.submitted');
	const isMerged = $derived(meta.kind === 'pr.merged');
	const isClosed = $derived(meta.kind === 'pr.closed' || meta.kind === 'issue.closed');
	const label = $derived(kindLabel[meta.kind] ?? meta.kind);

	const accentColor = $derived(
		isMerged
			? 'text-purple-500 border-purple-500/30 bg-purple-500/5'
			: isClosed
				? 'text-red-500 border-red-500/30 bg-red-500/5'
				: isReview && meta.verdict === 'approved'
					? 'text-green-500 border-green-500/30 bg-green-500/5'
					: 'text-[var(--primary)] border-[var(--primary)]/30 bg-[var(--primary)]/5'
	);
</script>

<div
	class="mt-0.5 inline-flex max-w-lg flex-col gap-1 rounded-lg border px-3 py-2 text-xs {accentColor}"
>
	<div class="flex items-center gap-1.5 font-medium">
		{#if isReview}
			<Eye class="h-3.5 w-3.5 shrink-0" />
		{:else if isMerged}
			<GitMerge class="h-3.5 w-3.5 shrink-0" />
		{:else if isPR}
			<GitPullRequest class="h-3.5 w-3.5 shrink-0" />
		{:else}
			<CircleDot class="h-3.5 w-3.5 shrink-0" />
		{/if}

		{#if isReview}
			<span>
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
				<a href={linkHref} target={linkTarget} rel="noopener noreferrer" class={linkClass}>
					@{meta.author}
				</a>
				{meta.verdict === 'approved'
					? 'approved'
					: meta.verdict === 'changes_requested'
						? 'requested changes on'
						: 'commented on'}
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
				<a href={linkHref} target={linkTarget} rel="noopener noreferrer" class={linkClass}>
					#{meta.number}
				</a>
			</span>
		{:else}
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
			<a href={linkHref} target={linkTarget} rel="noopener noreferrer" class={linkClass}>
				#{meta.number}
				{meta.title}
			</a>
		{/if}
	</div>

	<div class="flex flex-wrap items-center gap-2 text-[var(--muted-foreground)]">
		<span class="capitalize">{label} by @{meta.author}</span>

		{#if isPR && meta.additions !== undefined && meta.deletions !== undefined}
			<span class="text-green-500">+{meta.additions}</span>
			<span class="text-red-500">-{meta.deletions}</span>
		{/if}

		{#if isIssue && meta.labels?.length}
			<div class="flex flex-wrap gap-1">
				{#each meta.labels as lbl (lbl)}
					<span class="rounded-full bg-[var(--muted)] px-1.5 py-0.5 text-[10px]">{lbl}</span>
				{/each}
			</div>
		{/if}

		<span class="text-[var(--muted-foreground)]/60">{meta.repo}</span>
	</div>
</div>
