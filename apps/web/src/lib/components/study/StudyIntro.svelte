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
	let priorWeld = $state<string | null>(null);

	const emailValid = $derived(participantEmail.trim().includes('@'));

	const screeningComplete = $derived(
		emailValid &&
			institution.trim().length > 0 &&
			experience !== null &&
			familiarity !== null &&
			priorWeld !== null &&
			(!codeRequired || accessCode.trim().length > 0)
	);

	function submitScreening() {
		// Eligibility criteria per WELD_CONSENT_FORM.md ("Who can take part?"):
		// 18+, ≥1 year programming, has used issue tracker + chat + Git/GitHub, and
		// has not used Weld extensively before (prior exposure confounds the
		// time-on-task measure the study turns on).
		const ineligible = experience === 'lt1' || familiarity === 'none' || priorWeld === 'lots';
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
				<strong>Study title:</strong> The effect of AI agent context scope on productivity and trust in
				an integrated developer workspace
			</p>
			<p class="mt-2">
				<strong>Researchers:</strong> Dr. Jie Wu (Principal Investigator), Andrew Sadler (Student
				Investigator), Computer Science, Michigan Technological University. Approved by the Michigan
				Technological University IRB, protocol <strong>IRB-2026-111</strong>.
			</p>

			<p class="mt-3 font-medium text-[var(--foreground)]">What is this study about?</p>
			<p class="mt-1">
				You are invited to take part in a research study about how developers work when their tools
				(task board, team chat, and code integration) are combined into a single application, and
				how access to an AI assistant within that workspace affects how quickly and comfortably
				people complete common development-coordination tasks.
			</p>

			<p class="mt-3 font-medium text-[var(--foreground)]">What will I do?</p>
			<p class="mt-1">
				A single online session of <strong>about an hour</strong>, on your own: a few background
				questions, a set of guided tasks in a fictional practice project called PetTracker (no real
				code or accounts of yours are involved), and a short survey about your experience.
			</p>
			<p class="mt-2">
				<strong>Note on the AI assistant.</strong> You will be
				<strong>randomly assigned to one of three conditions</strong>
				that differ in how much information the AI assistant can access (e.g. just team chat, or chat
				plus the task board, or chat plus board plus GitHub).
				<strong>You will not be told which condition you are in during the session.</strong> After data
				collection is complete, the researchers can provide more detail if you request it.
			</p>

			<p class="mt-3 font-medium text-[var(--foreground)]">What information is collected?</p>
			<p class="mt-1">
				Your in-app actions and when you took them (e.g. opening cards, sending messages, moving
				cards, navigating), basic technical info (browser, OS, screen size), the messages you post
				as part of the tasks, your interactions with the AI assistant, your survey answers, and
				background categories such as age range and years of experience. We do <strong>not</strong>
				record keystrokes, mouse movement, your screen, anything from your other apps or tabs, or any
				of your real projects.
			</p>
			<p class="mt-2">
				Your email and institution are collected so your results can be linked to you. They are
				stored securely, separately from your study responses, and are not included in exported
				analysis datasets. Your session is labelled with an anonymised participant code (e.g.
				<span class="font-mono">P0012</span>), shown to you at the end.
			</p>
			<p class="mt-2">
				The AI assistant is powered by a third-party language-model service (OpenAI). The messages
				you send to the assistant, and the practice-project content it is given to answer them, are
				transmitted to that service for processing under their API data terms (not used to train
				their models). No real personal data or real code of yours is involved.
			</p>
			<p class="mt-2">
				De-identified study data (containing no name, email, or institution) may be retained
				indefinitely and shared publicly — for example in a public research repository such as OSF
				or Zenodo — to support reproducibility and future research.
			</p>

			<p class="mt-3 font-medium text-[var(--foreground)]">Risks, benefits, and payment</p>
			<p class="mt-1">
				Risks are minimal — no more than ordinary computer use; you may feel mild fatigue or
				frustration. There is no direct benefit to you and <strong>no compensation</strong> for taking
				part. Your participation helps researchers design better development tools.
			</p>

			<p class="mt-3 font-medium text-[var(--foreground)]">Is participation voluntary?</p>
			<p class="mt-1">
				Yes. You may decline to answer any question and may stop at any time by closing the browser
				tab, without penalty. To withdraw your data afterwards, email
				<span class="font-mono">awsadler@mtu.edu</span> with your participant code. Data that has already
				been published, aggregated, or deposited in a public repository cannot be withdrawn.
			</p>

			<p class="mt-3 font-medium text-[var(--foreground)]">Who can I contact?</p>
			<p class="mt-1">
				About the study: <span class="font-mono">awsadler@mtu.edu</span> or
				<span class="font-mono">jwu2@mtu.edu</span>. About your rights as a research participant:
				the MTU Human Research Protection Program, <span class="font-mono">irb@mtu.edu</span> or (906)
				487-1799.
			</p>

			<p class="mt-3">
				By checking the box below and continuing, you confirm that you are 18 years of age or older,
				have read and understood this information, understand that you will be randomly assigned to
				one of three AI assistant conditions and will not be told which during the session, and
				voluntarily agree to take part.
			</p>
			<p class="mt-3 text-xs">
				Consent version {CONSENT_VERSION}. A record that you consented (the version identifier and
				timestamp) is stored with your session data. You may save or print this page for your
				records.
			</p>
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

			<fieldset>
				<legend class="text-sm font-medium">Have you used Weld before this study?</legend>
				<div class="mt-2 flex flex-wrap gap-4 text-sm">
					<label class="flex items-center gap-2"
						><input type="radio" bind:group={priorWeld} value="never" /> Never</label
					>
					<label class="flex items-center gap-2"
						><input type="radio" bind:group={priorWeld} value="a_little" /> A little</label
					>
					<label class="flex items-center gap-2"
						><input type="radio" bind:group={priorWeld} value="lots" /> Extensively</label
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
