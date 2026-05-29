<script lang="ts">
	import { tasks as tasksApi, github as githubApi } from '$lib/api';
	import { boardStore } from '$lib/board.svelte';
	import { projects } from '$lib/projects.svelte';
	import { createPrDialog } from '$lib/create-pr-dialog.svelte';
	import { X, GitPullRequest } from 'lucide-svelte';

	const task = $derived(createPrDialog.task);
	const project = $derived(projects.current);

	let head = $state('');
	let base = $state('');
	let title = $state('');
	let body = $state('');
	let draft = $state(false);

	let branches = $state<string[]>([]);
	let loadingMeta = $state(false);
	let busy = $state(false);
	let error = $state<string | null>(null);

	// When a task is requested, reset the form and load branches + PR template.
	$effect(() => {
		const t = task;
		const p = project;
		if (!t || !p) return;

		title = t.title;
		head = '';
		base = '';
		draft = false;
		error = null;
		branches = [];
		body = t.description ?? '';

		loadingMeta = true;
		void Promise.all([
			githubApi.listBranches(p.id).catch(() => null),
			githubApi.getPRTemplate(p.id).catch(() => null)
		])
			.then(([branchData, templateData]) => {
				if (createPrDialog.task?.id !== t.id) return; // dialog changed while loading
				if (branchData) {
					branches = branchData.branches;
					base = branchData.default_branch;
				}
				if (templateData?.template) {
					body = templateData.template;
				}
			})
			.finally(() => {
				loadingMeta = false;
			});
	});

	function close() {
		createPrDialog.close();
	}

	async function submit(e: Event) {
		e.preventDefault();
		const t = task;
		if (!t) return;
		if (!head.trim() || !base.trim()) {
			error = 'Pick both a head and base branch.';
			return;
		}
		busy = true;
		error = null;
		try {
			const updated = await tasksApi.createPR(t.id, {
				head: head.trim(),
				base: base.trim(),
				title: title.trim(),
				body,
				draft
			});
			boardStore.applyTaskUpdate(updated);
			createPrDialog.confirm();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to create pull request.';
		} finally {
			busy = false;
		}
	}
</script>

{#if task && project}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
		role="dialog"
		aria-modal="true"
		aria-labelledby="create-pr-title"
	>
		<div
			class="flex h-[85vh] w-full max-w-3xl flex-col rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-xl"
		>
			<div class="flex items-center justify-between border-b border-[var(--border)] px-5 py-3">
				<h2 id="create-pr-title" class="flex items-center gap-2 text-base font-semibold">
					<GitPullRequest class="h-4 w-4" />
					Create pull request
				</h2>
				<button
					type="button"
					class="rounded p-1 hover:bg-[var(--accent)]"
					onclick={close}
					aria-label="Close"
				>
					<X class="h-4 w-4" />
				</button>
			</div>

			<form onsubmit={submit} class="flex min-h-0 flex-1 flex-col gap-3 p-5">
				<p class="text-xs text-[var(--muted-foreground)]">
					{project.github_repo}
					{#if task.github_issue_id}
						· links issue #{task.github_issue_id}
					{/if}
				</p>

				<div class="flex gap-3">
					<label class="block flex-1 text-sm">
						<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Head (compare)</span>
						<select
							bind:value={head}
							required
							disabled={loadingMeta}
							class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
						>
							<option value="" disabled>{loadingMeta ? 'Loading…' : 'Select branch'}</option>
							{#each branches as branch (branch)}
								<option value={branch}>{branch}</option>
							{/each}
						</select>
					</label>

					<label class="block flex-1 text-sm">
						<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Base (into)</span>
						<select
							bind:value={base}
							required
							disabled={loadingMeta}
							class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
						>
							<option value="" disabled>{loadingMeta ? 'Loading…' : 'Select branch'}</option>
							{#each branches as branch (branch)}
								<option value={branch}>{branch}</option>
							{/each}
						</select>
					</label>
				</div>

				<label class="block text-sm">
					<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Title</span>
					<input
						type="text"
						bind:value={title}
						required
						class="block w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					/>
				</label>

				<label class="flex min-h-0 flex-1 flex-col text-sm">
					<span class="mb-1 block text-xs text-[var(--muted-foreground)]">Description</span>
					<textarea
						bind:value={body}
						class="block min-h-0 w-full flex-1 resize-none overflow-y-auto rounded-md border border-[var(--input)] bg-[var(--background)] px-2 py-1.5 font-mono text-xs outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
					></textarea>
				</label>

				<label class="flex items-center gap-2 text-sm">
					<input type="checkbox" bind:checked={draft} />
					<span>Create as draft</span>
				</label>

				{#if error}
					<p class="text-xs text-red-500">{error}</p>
				{/if}

				<div class="flex justify-end gap-2 pt-1">
					<button
						type="button"
						onclick={close}
						class="rounded-md border border-[var(--border)] px-3 py-1.5 text-sm hover:bg-[var(--accent)]"
					>
						Cancel
					</button>
					<button
						type="submit"
						disabled={busy}
						class="rounded-md bg-[var(--primary)] px-3 py-1.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
					>
						{busy ? 'Creating…' : 'Create pull request'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}
