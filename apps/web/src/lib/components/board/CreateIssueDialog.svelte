<script lang="ts">
	import { tasks as tasksApi, github as githubApi } from '$lib/api';
	import type { IssueTemplate, Task } from '$lib/api/types';
	import { telemetry } from '$lib/telemetry.svelte';
	import { X, CircleDot } from '@lucide/svelte';

	interface Props {
		task: Task;
		projectId: string;
		repo: string;
		onClose: () => void;
		onCreated: (task: Task) => void;
	}

	const { task, projectId, repo, onClose, onCreated }: Props = $props();

	let templates = $state<IssueTemplate[]>([]);
	let selectedIndex = $state<number | null>(null);
	let loadingTemplates = $state(true);

	let title = $state(task.title);
	let body = $state(task.description ?? '');
	let busy = $state(false);
	let error = $state<string | null>(null);

	$effect(() => {
		loadingTemplates = true;
		githubApi
			.listIssueTemplates(projectId)
			.then((t) => {
				templates = t;
				// Auto-apply when the repo defines exactly one template.
				if (t.length === 1) applyTemplate(0);
			})
			.catch(() => {})
			.finally(() => {
				loadingTemplates = false;
			});
	});

	function applyTemplate(index: number) {
		selectedIndex = index;
		const tpl = templates[index];
		if (tpl) body = tpl.body;
	}

	async function submit(e: Event) {
		e.preventDefault();
		if (!title.trim()) {
			error = 'Title is required.';
			return;
		}
		busy = true;
		error = null;
		try {
			const updated = await tasksApi.pushToGithub(task.id, { title: title.trim(), body });
			telemetry.log('issue_opened', {
				task_id: task.id,
				issue_number: updated.github_issue_id
			});
			onCreated(updated);
			onClose();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to create issue.';
		} finally {
			busy = false;
		}
	}
</script>

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
	role="dialog"
	aria-modal="true"
	aria-labelledby="create-issue-title"
>
	<div
		class="flex max-h-[90vh] w-full max-w-lg flex-col rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-xl"
	>
		<div class="flex items-center justify-between border-b border-[var(--border)] px-5 py-3">
			<h2 id="create-issue-title" class="flex items-center gap-2 text-base font-semibold">
				<CircleDot class="h-4 w-4" />
				Create GitHub issue
			</h2>
			<button
				type="button"
				class="rounded p-1 hover:bg-[var(--accent)]"
				onclick={onClose}
				aria-label="Close"
			>
				<X class="h-4 w-4" />
			</button>
		</div>

		<form onsubmit={submit} class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-5">
			<p class="text-xs text-[var(--muted-foreground)]">{repo}</p>

			{#if templates.length > 1}
				<div class="space-y-1">
					<span class="block text-xs text-[var(--muted-foreground)]">Template</span>
					<div class="flex flex-wrap gap-2">
						{#each templates as tpl, i (tpl.name)}
							<button
								type="button"
								onclick={() => applyTemplate(i)}
								class="rounded-md border px-2.5 py-1 text-left text-xs transition-colors {selectedIndex ===
								i
									? 'border-[var(--ring)] bg-[var(--accent)]'
									: 'border-[var(--border)] hover:bg-[var(--accent)]'}"
								title={tpl.about ?? undefined}
							>
								{tpl.name}
							</button>
						{/each}
					</div>
				</div>
			{/if}

			<label class="block text-sm">
				<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Title</span>
				<input
					type="text"
					bind:value={title}
					required
					class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
			</label>

			<label class="block text-sm">
				<span class="mb-1 block text-xs text-[var(--muted-foreground)]">
					Description
					{#if loadingTemplates}
						<span class="opacity-70">· loading templates…</span>
					{/if}
				</span>
				<textarea
					bind:value={body}
					rows="10"
					class="block w-full resize-y rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 font-mono text-xs outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				></textarea>
			</label>

			{#if error}
				<p class="text-xs text-red-500">{error}</p>
			{/if}

			<div class="flex justify-end gap-2 pt-1">
				<button
					type="button"
					onclick={onClose}
					class="rounded-md border border-[var(--border)] px-3 py-1.5 text-sm hover:bg-[var(--accent)]"
				>
					Cancel
				</button>
				<button
					type="submit"
					disabled={busy}
					class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
				>
					{busy ? 'Creating…' : 'Create issue'}
				</button>
			</div>
		</form>
	</div>
</div>
