<script lang="ts">
	import { study as studyApi, userMessage } from '$lib/api';
	import { study } from '$lib/study.svelte';
	import { SURVEY, type SurveyItem } from '$lib/survey-config';
	import { cn } from '$lib/utils';

	// Single response per item. Multi-selects keep an array, joined on submit.
	let responses = $state<Record<string, string>>({});
	let multi = $state<Record<string, string[]>>({});
	let submitting = $state(false);
	let error = $state<string | null>(null);

	$effect(() => {
		study.noteSurveyStarted();
	});

	function setMulti(id: string, value: string, checked: boolean) {
		const current = multi[id] ?? [];
		multi[id] = checked ? [...current, value] : current.filter((v) => v !== value);
	}

	const requiredIds = SURVEY.flatMap((s) => s.items.filter((i) => i.required).map((i) => i.id));
	const missing = $derived(requiredIds.filter((id) => !responses[id]));

	async function submit() {
		if (missing.length > 0) {
			error = 'Please answer the required questions before submitting.';
			document
				.getElementById(`item-${missing[0]}`)
				?.scrollIntoView({ behavior: 'smooth', block: 'center' });
			return;
		}
		submitting = true;
		error = null;

		// Assemble the payload: every item's value, multis joined; the follow-up
		// email is split out so the backend stores it in its separate table.
		const out: Record<string, string> = { ...responses };
		for (const [id, vals] of Object.entries(multi)) {
			if (vals.length > 0) out[id] = vals.join(', ');
		}
		const followupEmail = out['followup_contact']?.trim();
		delete out['followup_contact'];

		try {
			await studyApi.submitSurvey({ responses: out, followup_email: followupEmail || undefined });
			study.finishSurvey();
		} catch (e) {
			error = userMessage(e, 'Could not submit the survey. Please retry.');
			submitting = false;
		}
	}

	function scaleTicks(max: number): number[] {
		return Array.from({ length: max }, (_, i) => i + 1);
	}
</script>

<div class="mx-auto max-w-2xl px-6 py-10">
	<h1 class="text-2xl font-semibold">One last thing — a short survey</h1>
	<p class="mt-2 text-sm text-[var(--muted-foreground)]">
		Your answers help us understand your experience. It takes about 5 minutes.
	</p>

	{#each SURVEY as section (section.title)}
		<section class="mt-10">
			<h2 class="text-lg font-medium">{section.title}</h2>
			{#if section.instruction}
				<p class="mt-1 text-sm text-[var(--muted-foreground)]">{section.instruction}</p>
			{/if}

			<div class="mt-5 space-y-7">
				{#each section.items as item (item.id)}
					{@render field(item)}
				{/each}
			</div>
		</section>
	{/each}

	{#if error}
		<p class="mt-6 text-sm text-red-500">{error}</p>
	{/if}

	<div class="mt-8 mb-16">
		<button
			type="button"
			disabled={submitting}
			class="rounded-md bg-[var(--primary)] px-6 py-2.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
			onclick={submit}
		>
			{submitting ? 'Submitting…' : 'Submit and finish'}
		</button>
	</div>
</div>

{#snippet field(item: SurveyItem)}
	<div id="item-{item.id}">
		<p class="text-sm font-medium">
			{item.label}
			{#if item.required}<span class="text-red-500">*</span>{/if}
		</p>

		{#if item.type === 'slider100'}
			{@const touched = responses[item.id] !== undefined && responses[item.id] !== ''}
			<div class="mt-3">
				<input
					type="range"
					min="1"
					max="100"
					step="1"
					name={item.id}
					value={touched ? responses[item.id] : '50'}
					oninput={(e) => (responses[item.id] = e.currentTarget.value)}
					class="w-full accent-[var(--primary)]"
					aria-label={item.label}
				/>
				<div
					class="mt-1 flex items-center justify-between text-[11px] text-[var(--muted-foreground)]"
				>
					<span>{item.lowAnchor ?? ''}</span>
					<span class="text-sm font-medium text-[var(--foreground)]">
						{touched ? responses[item.id] : 'Drag to set'}
					</span>
					<span>{item.highAnchor ?? ''}</span>
				</div>
			</div>
		{:else if item.type === 'likert5' || item.type === 'likert7'}
			{@const max = item.type === 'likert5' ? 5 : 7}
			<div class="mt-2.5">
				<div class="flex gap-1.5">
					{#each scaleTicks(max) as n (n)}
						{@const selected = responses[item.id] === String(n)}
						<label
							class={cn(
								'flex h-9 flex-1 cursor-pointer items-center justify-center rounded-md border text-sm transition-colors',
								selected
									? 'border-[var(--primary)] bg-[var(--primary)] font-medium text-[var(--primary-foreground)]'
									: 'border-[var(--border)] text-[var(--muted-foreground)] hover:border-[var(--primary)] hover:text-[var(--foreground)]'
							)}
						>
							<input
								type="radio"
								class="sr-only"
								name={item.id}
								value={String(n)}
								checked={selected}
								onchange={() => (responses[item.id] = String(n))}
							/>
							{n}
						</label>
					{/each}
				</div>
				{#if item.lowAnchor || item.highAnchor}
					<div class="mt-1.5 flex justify-between text-[11px] text-[var(--muted-foreground)]">
						<span>{item.lowAnchor ?? ''}</span>
						<span>{item.highAnchor ?? ''}</span>
					</div>
				{/if}
			</div>
		{:else if item.type === 'text'}
			<textarea
				rows="3"
				class="mt-2 w-full rounded-md border border-[var(--border)] bg-[var(--background)] p-2 text-sm"
				value={responses[item.id] ?? ''}
				oninput={(e) => (responses[item.id] = e.currentTarget.value)}
			></textarea>
		{:else if item.type === 'single'}
			<div class="mt-2 flex flex-col gap-1.5">
				{#each item.options ?? [] as opt (opt.value)}
					<label class="flex items-center gap-2 text-sm">
						<input
							type="radio"
							name={item.id}
							value={opt.value}
							checked={responses[item.id] === opt.value}
							onchange={() => (responses[item.id] = opt.value)}
						/>
						{opt.label}
					</label>
				{/each}
			</div>
		{:else if item.type === 'multi'}
			<div class="mt-2 flex flex-wrap gap-x-4 gap-y-1.5">
				{#each item.options ?? [] as opt (opt.value)}
					<label class="flex items-center gap-2 text-sm">
						<input
							type="checkbox"
							checked={(multi[item.id] ?? []).includes(opt.value)}
							onchange={(e) => setMulti(item.id, opt.value, e.currentTarget.checked)}
						/>
						{opt.label}
					</label>
				{/each}
			</div>
		{/if}
	</div>
{/snippet}
