<script lang="ts">
	import { study } from '$lib/study.svelte';
	const ineligible = $derived(study.stage === 'ineligible');
</script>

<div
	class="mx-auto flex min-h-screen max-w-xl flex-col items-center justify-center px-6 text-center"
>
	{#if ineligible}
		<h1 class="text-2xl font-semibold">Thanks for your interest!</h1>
		<p class="mt-4 text-[var(--muted-foreground)]">
			Based on your answers, you’re not eligible for this particular study, but we really appreciate
			you stopping by. No data has been collected.
		</p>
	{:else}
		<h1 class="text-2xl font-semibold">All done — thank you! 🎉</h1>
		<p class="mt-4 text-[var(--muted-foreground)]">
			You’ve completed the study and your responses are recorded.
			{#if study.participantCode}
				<br />Your participant code is <span class="font-mono">{study.participantCode}</span>.
			{/if}
		</p>
		<p class="mt-3 text-sm text-[var(--muted-foreground)]">
			If you opted in to future research related to this study, we’ll reach out via the email you
			provided. You can close this tab now.
		</p>
	{/if}
</div>
