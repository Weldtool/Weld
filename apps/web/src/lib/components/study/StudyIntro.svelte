<script lang="ts">
	import { study, CONSENT_VERSION } from '$lib/study.svelte';
	import { appConfig } from '$lib/config.svelte';

	// Pre-provisioning steps. Copy adapted from WELD_PARTICIPANT_FLOW.md.
	type Step = 'welcome' | 'consent' | 'screening';
	let step = $state<Step>('welcome');

	let consentAffirmed = $state(false);

	// Single-use access code (only when the backend requires one).
	let accessCode = $state('');
	const codeRequired = $derived(appConfig.requireAccessCode);

	// Identity fields (collected for result linkage).
	let participantEmail = $state('');
	let institution = $state('');

	// Screening answers (eligibility gate; not stored — demographics come later).
	let experience = $state<string | null>(null);
	let familiarity = $state<string | null>(null);

	const emailValid = $derived(participantEmail.trim().includes('@'));

	const screeningComplete = $derived(
		emailValid &&
			institution.trim().length > 0 &&
			experience !== null &&
			familiarity !== null &&
			(!codeRequired || accessCode.trim().length > 0)
	);

	function submitScreening() {
		const ineligible = experience === 'lt1' || familiarity === 'none';
		if (ineligible) {
			study.markIneligible();
		} else {
			void study.begin({
				accessCode: codeRequired ? accessCode : undefined,
				participantEmail: participantEmail.trim(),
				institution: institution.trim()
			});
		}
	}
</script>

<div class="mx-auto flex min-h-screen max-w-2xl flex-col justify-center px-6 py-12">
	{#if step === 'welcome'}
		<h1 class="text-2xl font-semibold">Welcome 👋</h1>
		<p class="mt-4 text-[var(--muted-foreground)]">
			Thanks for your interest in this research study. Here's what to expect:
		</p>
		<ul class="mt-4 list-disc space-y-2 pl-6 text-sm text-[var(--muted-foreground)]">
			<li><strong>About an hour</strong>, done online at your own pace.</li>
			<li>
				You'll work through realistic tasks in <strong>Weld</strong>, a developer workspace, using a
				<strong>practice project</strong> — none of your real code or accounts are involved.
			</li>
			<li>Then a <strong>short survey</strong> about your experience.</li>
		</ul>
		<p class="mt-4 text-sm text-[var(--muted-foreground)]">
			You'll be asked for your email and institution so we can link your responses to you. Please
			read the consent information on the next page before you begin.
		</p>
		<div class="mt-8">
			<button
				type="button"
				class="rounded-md bg-[var(--primary)] px-5 py-2.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90"
				onclick={() => (step = 'consent')}
			>
				Continue
			</button>
		</div>
	{:else if step === 'consent'}
		<h1 class="text-2xl font-semibold">Consent to participate</h1>
		<div
			class="mt-4 max-h-72 overflow-y-auto rounded-md border border-[var(--border)] p-4 text-sm text-[var(--muted-foreground)]"
		>
			<p>
				You are invited to take part in a research study about how developers work in an integrated
				workspace assisted by an AI agent. Participation is voluntary and takes about an hour.
			</p>
			<p class="mt-3">
				You will complete a set of tasks in a practice project and answer a short survey. We log
				only your in-app actions (e.g. opening cards, sending messages, navigating) and your survey
				answers. We do <strong>not</strong> record keystrokes, mouse movement, your screen, or any data
				outside this study.
			</p>
			<p class="mt-3">
				Your email and institution are collected so your results can be linked to you. This
				information is stored securely, separately from your study responses, and used only for
				research purposes. You may stop at any time by closing the tab.
			</p>
			<p class="mt-3">
				The AI assistant is powered by a third-party language-model service (OpenAI). The messages
				you send to the assistant, and the practice-project content it is given to answer them, are
				transmitted to that service for processing under their API data terms (not used to train
				their models). No real personal data or real code of yours is involved.
			</p>
			<p class="mt-3">
				De-identified study data (containing no name, email, or institution) may be retained
				indefinitely and shared publicly to support reproducibility and future research. Data that
				has already been published or shared cannot be withdrawn.
			</p>
			<p class="mt-3">By continuing, you confirm you are 18+ and consent to take part.</p>
			<p class="mt-3 text-xs">Consent version {CONSENT_VERSION}</p>
		</div>
		<label class="mt-4 flex cursor-pointer items-center gap-2 text-sm">
			<input type="checkbox" bind:checked={consentAffirmed} class="rounded" />
			I have read the above and consent to participate.
		</label>
		<div class="mt-8 flex gap-3">
			<button
				type="button"
				class="rounded-md border border-[var(--border)] px-4 py-2 text-sm hover:bg-[var(--accent)]"
				onclick={() => (step = 'welcome')}
			>
				Back
			</button>
			<button
				type="button"
				disabled={!consentAffirmed}
				class="rounded-md bg-[var(--primary)] px-5 py-2.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
				onclick={() => (step = 'screening')}
			>
				I consent — continue
			</button>
		</div>
	{:else}
		<h1 class="text-2xl font-semibold">A few quick questions</h1>
		<p class="mt-2 text-sm text-[var(--muted-foreground)]">
			This helps us confirm you're eligible and link your results. (~1 minute.)
		</p>

		<div class="mt-6 space-y-6">
			<!-- Identity -->
			<div class="space-y-1">
				<label class="text-sm font-medium" for="participant-email">Email address</label>
				<input
					id="participant-email"
					type="email"
					bind:value={participantEmail}
					placeholder="you@example.com"
					class="mt-1 w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
			</div>

			<div class="space-y-1">
				<label class="text-sm font-medium" for="participant-institution">
					Institution (university or company)
				</label>
				<input
					id="participant-institution"
					type="text"
					bind:value={institution}
					placeholder="e.g. University of Example"
					class="mt-1 w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
			</div>

			<!-- Eligibility -->
			<fieldset>
				<legend class="text-sm font-medium"
					>How many years of programming experience do you have?</legend
				>
				<div class="mt-2 flex flex-wrap gap-4 text-sm">
					{#each [['lt1', '< 1'], ['1to2', '1–2'], ['3to5', '3–5'], ['6to10', '6–10'], ['11plus', '11+']] as [val, label] (val)}
						<label class="flex items-center gap-2"
							><input type="radio" bind:group={experience} value={val} /> {label}</label
						>
					{/each}
				</div>
			</fieldset>

			<fieldset>
				<legend class="text-sm font-medium"
					>Have you used an issue tracker, a team chat tool, AND Git/GitHub before?</legend
				>
				<div class="mt-2 flex flex-wrap gap-4 text-sm">
					<label class="flex items-center gap-2"
						><input type="radio" bind:group={familiarity} value="all" /> Yes, all three</label
					>
					<label class="flex items-center gap-2"
						><input type="radio" bind:group={familiarity} value="some" /> Some</label
					>
					<label class="flex items-center gap-2"
						><input type="radio" bind:group={familiarity} value="none" /> None</label
					>
				</div>
			</fieldset>
		</div>

		{#if codeRequired}
			<div class="mt-6">
				<label for="access-code" class="text-sm font-medium">Access code</label>
				<p class="mt-1 text-xs text-[var(--muted-foreground)]">
					Enter the code you were given to take part. Each code works once.
				</p>
				<input
					id="access-code"
					type="text"
					bind:value={accessCode}
					autocomplete="off"
					spellcheck="false"
					placeholder="WELD-XXXX-XXXX"
					class="mt-2 w-full max-w-xs rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 font-mono text-sm tracking-wide uppercase outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
				/>
			</div>
		{/if}

		{#if study.error}
			<p class="mt-4 text-sm text-red-500">{study.error}</p>
		{/if}

		<div class="mt-8 flex gap-3">
			<button
				type="button"
				class="rounded-md border border-[var(--border)] px-4 py-2 text-sm hover:bg-[var(--accent)]"
				onclick={() => (step = 'consent')}
			>
				Back
			</button>
			<button
				type="button"
				disabled={!screeningComplete || study.provisioning}
				class="rounded-md bg-[var(--primary)] px-5 py-2.5 text-sm font-medium text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50"
				onclick={submitScreening}
			>
				{study.provisioning ? 'Setting up…' : 'Start the study'}
			</button>
		</div>
	{/if}
</div>
