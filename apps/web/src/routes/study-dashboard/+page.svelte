<script lang="ts">
	import { onMount } from 'svelte';
	import { env } from '$env/dynamic/public';
	import { errorFromResponse, userMessage } from '$lib/api/client';

	const API_BASE = (env.PUBLIC_API_BASE_URL ?? '').replace(/\/$/, '');

	// ── Types ────────────────────────────────────────────────────────────────

	interface ConditionCount {
		condition: string;
		total: number;
		completed: number;
	}

	interface StatusCount {
		status: string;
		count: number;
	}

	interface EventTypeCount {
		event_type: string;
		count: number;
	}

	interface ParticipantRow {
		participant_code: string;
		agent_condition: string;
		status: string;
		provisioned_at: string;
		completed_at: string | null;
		duration_seconds: number | null;
		event_count: number;
		survey_items_answered: number;
		last_event_at: string | null;
		active_task_ms: number | null;
	}

	interface AccessCodeStats {
		total: number;
		used: number;
		unused: number;
	}

	interface TimeCell {
		condition: string;
		task_type: string;
		mean_active_ms: number;
		n: number;
	}

	interface DelegationRow {
		condition: string;
		agent_actions: number;
		user_actions: number;
	}

	interface ManipulationRow {
		condition: string;
		turns: number;
		clean_no_board_github: number;
		clean_no_github: number;
	}

	interface StudyCharts {
		time_by_cell: TimeCell[];
		delegation: DelegationRow[];
		manipulation: ManipulationRow[];
	}

	interface StudySummary {
		total_participants: number;
		by_condition: ConditionCount[];
		by_status: StatusCount[];
		top_event_types: EventTypeCount[];
		access_codes: AccessCodeStats;
		charts: StudyCharts;
		participants: ParticipantRow[];
	}

	// ── State ────────────────────────────────────────────────────────────────

	let token = $state('');
	let savedToken = $state('');
	let summary = $state<StudySummary | null>(null);
	let error = $state('');
	let loading = $state(false);
	let downloadingDataset = $state('');
	// Default to the paper-figure view; raw tables are a toggle away.
	let view = $state<'charts' | 'raw'>('charts');
	// Mint-codes panel state.
	let mintCount = $state(10);
	let minting = $state(false);
	let mintedCodes = $state('');
	let mintError = $state('');

	const SESSION_KEY = 'study_dashboard_token';
	const REFRESH_INTERVAL_MS = 60_000;

	onMount(() => {
		const stored = sessionStorage.getItem(SESSION_KEY);
		if (stored) {
			savedToken = stored;
			token = stored;
			fetchSummary(stored);
		}
		// Poll while authenticated so a left-open dashboard stays live during a run.
		// Skipped when signed out (no token) or when a fetch is already in flight.
		const timer = setInterval(() => {
			if (savedToken && !loading) fetchSummary(savedToken);
		}, REFRESH_INTERVAL_MS);
		return () => clearInterval(timer);
	});

	// ── API helpers ──────────────────────────────────────────────────────────

	async function apiGet<T>(path: string, tok: string): Promise<T> {
		const res = await fetch(`${API_BASE}${path}`, {
			headers: { Authorization: `Bearer ${tok}` }
		});
		if (!res.ok) throw errorFromResponse(res.status, await res.text().catch(() => ''));
		return res.json();
	}

	async function fetchSummary(tok: string) {
		loading = true;
		error = '';
		try {
			summary = await apiGet<StudySummary>('/api/v1/study/summary', tok);
			savedToken = tok;
			sessionStorage.setItem(SESSION_KEY, tok);
		} catch (e: unknown) {
			error = userMessage(e, "Couldn't load the study summary. Please try again.");
			summary = null;
		} finally {
			loading = false;
		}
	}

	function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (token.trim()) fetchSummary(token.trim());
	}

	function signOut() {
		sessionStorage.removeItem(SESSION_KEY);
		savedToken = '';
		token = '';
		summary = null;
		error = '';
	}

	async function downloadDataset(dataset: string) {
		downloadingDataset = dataset;
		try {
			const res = await fetch(`${API_BASE}/api/v1/study/export?dataset=${dataset}`, {
				headers: { Authorization: `Bearer ${savedToken}` }
			});
			if (!res.ok) throw errorFromResponse(res.status, await res.text().catch(() => ''));
			const blob = await res.blob();
			const filename =
				res.headers.get('Content-Disposition')?.match(/filename="([^"]+)"/)?.[1] ??
				`study_${dataset}.csv`;
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = filename;
			a.click();
			URL.revokeObjectURL(url);
		} catch (e: unknown) {
			alert(`Download failed. ${userMessage(e, 'Please try again.')}`);
		} finally {
			downloadingDataset = '';
		}
	}

	// Mint single-use participation codes (researcher-only; same token as export).
	// The endpoint returns text/plain, one code per line, ready to hand out.
	async function mintCodes() {
		minting = true;
		mintError = '';
		mintedCodes = '';
		try {
			const res = await fetch(`${API_BASE}/api/v1/study/generate-codes`, {
				method: 'POST',
				headers: {
					Authorization: `Bearer ${savedToken}`,
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({ count: mintCount, note: null })
			});
			if (!res.ok) throw errorFromResponse(res.status, await res.text().catch(() => ''));
			mintedCodes = await res.text();
			fetchSummary(savedToken); // refresh the unused/used counts
		} catch (e: unknown) {
			mintError = userMessage(e, "Couldn't generate codes. Please try again.");
		} finally {
			minting = false;
		}
	}

	function downloadMintedCodes() {
		const blob = new Blob([mintedCodes], { type: 'text/plain' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `study_access_codes_${new Date().toISOString().slice(0, 10)}.txt`;
		a.click();
		URL.revokeObjectURL(url);
	}

	// ── Derived helpers ──────────────────────────────────────────────────────

	function completionPct(c: ConditionCount) {
		return c.total === 0 ? 0 : Math.round((c.completed / c.total) * 100);
	}

	// Clock fix: the meaningful "time on task" is summed active (engaged) time
	// from the per-task clock, not wall-clock provision→survey (which includes
	// breaks and cross-device resumes).
	function avgActiveTaskTime(participants: ParticipantRow[]): string {
		const times = participants
			.map((p) => p.active_task_ms)
			.filter((ms): ms is number => ms !== null && ms > 0);
		if (times.length === 0) return '—';
		const avgSecs = Math.round(times.reduce((a, b) => a + b, 0) / times.length / 1000);
		return formatDuration(avgSecs);
	}

	// ── Chart helpers ─────────────────────────────────────────────────────────

	const TASK_TYPE_ORDER = ['comprehension', 'single_surface', 'cross_surface'];
	const taskTypeLabel: Record<string, string> = {
		comprehension: 'Comprehension',
		single_surface: 'Single-surface',
		cross_surface: 'Cross-surface'
	};
	const conditionBar: Record<string, string> = {
		C1: 'bg-sky-500',
		C2: 'bg-indigo-500',
		C3: 'bg-violet-500'
	};

	function cell(charts: StudyCharts, condition: string, taskType: string): TimeCell | undefined {
		return charts.time_by_cell.find((c) => c.condition === condition && c.task_type === taskType);
	}

	function maxCellMs(charts: StudyCharts): number {
		return charts.time_by_cell.reduce((m, c) => Math.max(m, c.mean_active_ms), 1);
	}

	function fmtMs(ms: number): string {
		return formatDuration(Math.round(ms / 1000));
	}

	function delegationPct(d: DelegationRow): number {
		const total = d.agent_actions + d.user_actions;
		return total === 0 ? 0 : Math.round((d.agent_actions / total) * 100);
	}

	// Scoping target % for the manipulation check: C1 wants 0 board/GitHub turns,
	// C2 wants 0 GitHub turns. C3 has no target (all sources allowed by design).
	function scopingPct(m: ManipulationRow): number | null {
		if (m.turns === 0) return null;
		if (m.condition === 'C1') return Math.round((m.clean_no_board_github / m.turns) * 100);
		if (m.condition === 'C2') return Math.round((m.clean_no_github / m.turns) * 100);
		return null;
	}

	function maxEventCount(rows: EventTypeCount[]) {
		return rows.reduce((m, r) => Math.max(m, r.count), 1);
	}

	function formatDuration(secs: number | null): string {
		if (secs === null) return '—';
		const m = Math.floor(secs / 60);
		const s = secs % 60;
		return m > 0 ? `${m}m ${s}s` : `${s}s`;
	}

	function formatDate(iso: string | null): string {
		if (!iso) return '—';
		return new Date(iso).toLocaleString();
	}

	// Nothing sets status='abandoned' (it's only a telemetry event), so an
	// in_progress participant whose last event is old is a likely dropout. Flag
	// them so the live dashboard distinguishes "active now" from "quit days ago".
	const STALE_AFTER_MS = 60 * 60 * 1000; // 1h with no events = likely dropped

	function lastSeenAgo(iso: string | null): string {
		if (!iso) return 'never';
		const ms = Date.now() - new Date(iso).getTime();
		const mins = Math.floor(ms / 60000);
		if (mins < 1) return 'just now';
		if (mins < 60) return `${mins}m ago`;
		const hrs = Math.floor(mins / 60);
		if (hrs < 24) return `${hrs}h ago`;
		return `${Math.floor(hrs / 24)}d ago`;
	}

	function likelyDropped(p: ParticipantRow): boolean {
		return (
			p.status === 'in_progress' &&
			(p.last_event_at === null ||
				Date.now() - new Date(p.last_event_at).getTime() > STALE_AFTER_MS)
		);
	}

	function activeNow(participants: ParticipantRow[]): number {
		return participants.filter((p) => p.status === 'in_progress' && !likelyDropped(p)).length;
	}

	function likelyDroppedCount(participants: ParticipantRow[]): number {
		return participants.filter(likelyDropped).length;
	}

	const statusColor: Record<string, string> = {
		completed: 'bg-green-500',
		in_progress: 'bg-blue-500',
		consented: 'bg-yellow-500',
		provisioned: 'bg-slate-400',
		abandoned: 'bg-red-400'
	};

	const conditionLabel: Record<string, string> = {
		C1: 'C1 — Chat only',
		C2: 'C2 — Chat + Board',
		C3: 'C3 — Chat + Board + GitHub'
	};
</script>

<svelte:head>
	<title>Study Dashboard</title>
</svelte:head>

{#if !summary}
	<!-- ── Gate: code entry ─────────────────────────────────────────────────── -->
	<div class="flex min-h-screen items-center justify-center bg-slate-950">
		<div class="w-full max-w-sm space-y-6 p-8">
			<div class="space-y-1 text-center">
				<h1 class="text-xl font-semibold text-white">Research Dashboard</h1>
				<p class="text-sm text-slate-400">Enter the researcher access token to continue.</p>
			</div>

			<form onsubmit={handleSubmit} class="space-y-4">
				<input
					type="password"
					bind:value={token}
					placeholder="Access token"
					autocomplete="off"
					class="w-full rounded-md border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-slate-500 focus:outline-none"
				/>

				{#if error}
					<p class="text-xs text-red-400">{error}</p>
				{/if}

				<button
					type="submit"
					disabled={loading || !token.trim()}
					class="w-full rounded-md bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-500 disabled:opacity-50"
				>
					{loading ? 'Loading…' : 'Access Dashboard'}
				</button>
			</form>
		</div>
	</div>
{:else}
	<!-- ── Dashboard ──────────────────────────────────────────────────────────── -->
	<div class="min-h-screen bg-slate-950 text-white">
		<!-- Header -->
		<header class="border-b border-slate-800 px-6 py-4">
			<div class="mx-auto flex max-w-7xl items-center justify-between">
				<div>
					<h1 class="text-lg font-semibold">Research Study Dashboard</h1>
					<p class="text-xs text-slate-400">Live data · auto-refreshes every 60s</p>
				</div>
				<div class="flex items-center gap-3">
					<button
						onclick={() => fetchSummary(savedToken)}
						class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-300 hover:border-slate-500"
					>
						Refresh
					</button>
					<button
						onclick={signOut}
						class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-300 hover:border-slate-500"
					>
						Sign out
					</button>
				</div>
			</div>
		</header>

		<main class="mx-auto max-w-7xl space-y-8 px-6 py-8">
			<!-- ── Top stat cards ──────────────────────────────────────────────── -->
			<div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
				{@render StatCard({ label: 'Total participants', value: summary.total_participants })}
				{@render StatCard({
					label: 'Completed',
					value: summary.by_status.find((s) => s.status === 'completed')?.count ?? 0,
					sub:
						summary.total_participants > 0
							? `${Math.round(((summary.by_status.find((s) => s.status === 'completed')?.count ?? 0) / summary.total_participants) * 100)}%`
							: '0%'
				})}
				{@render StatCard({
					label: 'In progress',
					value: activeNow(summary.participants),
					sub:
						likelyDroppedCount(summary.participants) > 0
							? `${likelyDroppedCount(summary.participants)} likely dropped`
							: 'active in the last hour'
				})}
				{@render StatCard({
					label: 'Avg. time on task',
					value: avgActiveTaskTime(summary.participants),
					sub: 'active (engaged) time'
				})}
			</div>

			<!-- ── View toggle ─────────────────────────────────────────────────── -->
			<div
				class="flex items-center gap-1 rounded-lg border border-slate-800 bg-slate-900 p-1 text-xs sm:w-fit"
			>
				<button
					onclick={() => (view = 'charts')}
					class="rounded-md px-4 py-1.5 font-medium transition-colors {view === 'charts'
						? 'bg-indigo-600 text-white'
						: 'text-slate-400 hover:text-slate-200'}"
				>
					Figures
				</button>
				<button
					onclick={() => (view = 'raw')}
					class="rounded-md px-4 py-1.5 font-medium transition-colors {view === 'raw'
						? 'bg-indigo-600 text-white'
						: 'text-slate-400 hover:text-slate-200'}"
				>
					Raw data
				</button>
			</div>

			{#if view === 'charts'}
				{@render ChartsView(summary.charts)}
			{:else}
				{@render RawView()}
			{/if}

			<!-- ── Data exports + code minting (always visible) ────────────────── -->
			{@render ToolsSection()}
		</main>
	</div>
{/if}

<!-- ════════════════════════════════════════════════════════════════════════ -->
<!--  Figures view (default) — live previews of the paper figures             -->
<!-- ════════════════════════════════════════════════════════════════════════ -->
{#snippet ChartsView(charts: StudyCharts)}
	<div class="space-y-6">
		<!-- Fig 1 — active time by condition × task type -->
		<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
			<h2 class="text-sm font-medium text-slate-300">Time on task by condition × task type</h2>
			<p class="mb-4 text-xs text-slate-500">
				Mean active time per agent task (Fig. 1 shape). Raw active time — the paper normalizes to
				each participant's baseline. Expect cells to look similar on comprehension / single-surface
				and fan apart on cross-surface, with C3 lowest.
			</p>
			{#if charts.time_by_cell.length === 0}
				<p class="text-xs text-slate-500">No completed agent tasks yet.</p>
			{:else}
				{@const maxMs = maxCellMs(charts)}
				<div class="space-y-5">
					{#each TASK_TYPE_ORDER as tt (tt)}
						<div>
							<p class="mb-1.5 text-xs font-medium text-slate-400">{taskTypeLabel[tt]}</p>
							<div class="space-y-1">
								{#each ['C1', 'C2', 'C3'] as cond (cond)}
									{@const c = cell(charts, cond, tt)}
									<div class="flex items-center gap-2">
										<span class="w-6 text-xs text-slate-500">{cond}</span>
										<div class="h-4 flex-1 overflow-hidden rounded bg-slate-800">
											{#if c}
												<div
													class="h-4 rounded {conditionBar[cond]}"
													style="width: {Math.max(2, (c.mean_active_ms / maxMs) * 100)}%"
												></div>
											{/if}
										</div>
										<span class="w-28 text-right text-xs text-slate-400 tabular-nums">
											{c ? `${fmtMs(c.mean_active_ms)} (n=${c.n})` : '—'}
										</span>
									</div>
								{/each}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</section>

		<div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
			<!-- Fig 4 — delegation by condition -->
			<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
				<h2 class="text-sm font-medium text-slate-300">Division of labor by condition</h2>
				<p class="mb-4 text-xs text-slate-500">
					Share of agent-phase actions done by the agent vs. the user (Fig. 4 shape).
				</p>
				{#if charts.delegation.length === 0}
					<p class="text-xs text-slate-500">No agent-phase actions yet.</p>
				{:else}
					<div class="space-y-3">
						{#each charts.delegation as d (d.condition)}
							{@const pct = delegationPct(d)}
							<div>
								<div class="flex justify-between text-xs">
									<span class="font-medium">{d.condition}</span>
									<span class="text-slate-400"
										>{pct}% agent · {d.agent_actions + d.user_actions} actions</span
									>
								</div>
								<div class="mt-1 flex h-3 overflow-hidden rounded-full bg-slate-800">
									<div class="bg-violet-500" style="width: {pct}%"></div>
									<div class="bg-slate-600" style="width: {100 - pct}%"></div>
								</div>
							</div>
						{/each}
					</div>
					<p class="mt-3 text-xs text-slate-600">Violet = agent · grey = user</p>
				{/if}
			</section>

			<!-- Fig 5 — live scoping manipulation check -->
			<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
				<h2 class="text-sm font-medium text-slate-300">Scoping check (manipulation)</h2>
				<p class="mb-4 text-xs text-slate-500">
					From <code class="text-slate-400">agent_context_sent</code>. C1 turns must be 100%
					board+GitHub-free, C2 turns 100% GitHub-free. Anything below 100% is a scoping leak.
				</p>
				{#if charts.manipulation.length === 0}
					<p class="text-xs text-slate-500">No agent turns yet.</p>
				{:else}
					<div class="space-y-2">
						{#each charts.manipulation as m (m.condition)}
							{@const pct = scopingPct(m)}
							<div class="flex items-center gap-3 text-sm">
								<span class="w-6 font-medium">{m.condition}</span>
								<span class="flex-1 text-xs text-slate-400">
									{#if m.condition === 'C1'}
										no board/GitHub: {m.clean_no_board_github}/{m.turns}
									{:else if m.condition === 'C2'}
										no GitHub: {m.clean_no_github}/{m.turns}
									{:else}
										all sources (by design) · {m.turns} turns
									{/if}
								</span>
								{#if pct !== null}
									<span
										class="rounded px-2 py-0.5 text-xs font-medium tabular-nums {pct === 100
											? 'bg-green-900 text-green-300'
											: 'bg-red-900 text-red-300'}">{pct}%</span
									>
								{:else}
									<span class="text-xs text-slate-500">—</span>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</section>
		</div>

		<p class="text-xs text-slate-600">
			Correctness (Fig. 2) and the trust/perception figure (Fig. 3) aren't shown here: correctness
			is hand-coded offline and perception needs the analyzed survey, so neither is derivable from
			the live event log.
		</p>
	</div>
{/snippet}

<!-- ════════════════════════════════════════════════════════════════════════ -->
<!--  Raw view — the original tables/breakdowns                               -->
<!-- ════════════════════════════════════════════════════════════════════════ -->
{#snippet RawView()}
	{#if summary}
		<div class="space-y-8">
			<!-- ── Condition breakdown + Status breakdown ──────────────────────── -->
			<div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
				<!-- Condition breakdown -->
				<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
					<h2 class="mb-4 text-sm font-medium text-slate-300">Participants by condition</h2>
					<div class="space-y-4">
						{#each summary.by_condition as c (c.condition)}
							{@const pct = completionPct(c)}
							<div class="space-y-1">
								<div class="flex items-center justify-between text-xs">
									<span class="font-medium">{conditionLabel[c.condition] ?? c.condition}</span>
									<span class="text-slate-400">{c.completed}/{c.total} completed</span>
								</div>
								<div class="h-2 overflow-hidden rounded-full bg-slate-800">
									<div
										class="h-2 rounded-full bg-indigo-500 transition-all"
										style="width: {pct}%"
									></div>
								</div>
								<p class="text-right text-xs text-slate-500">{pct}%</p>
							</div>
						{/each}
						{#if summary.by_condition.length === 0}
							<p class="text-xs text-slate-500">No participants yet.</p>
						{/if}
					</div>
				</section>

				<!-- Status breakdown + access codes -->
				<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
					<h2 class="mb-4 text-sm font-medium text-slate-300">Status breakdown</h2>
					<div class="space-y-2">
						{#each summary.by_status as s (s.status)}
							<div class="flex items-center gap-3">
								<span
									class="inline-block h-2.5 w-2.5 rounded-full {statusColor[s.status] ??
										'bg-slate-500'}"
								></span>
								<span class="flex-1 text-sm capitalize">{s.status.replace('_', ' ')}</span>
								<span class="text-sm font-medium tabular-nums">{s.count}</span>
							</div>
						{/each}
						{#if summary.by_status.length === 0}
							<p class="text-xs text-slate-500">No data yet.</p>
						{/if}
					</div>

					{#if summary.access_codes.total > 0}
						<div class="mt-5 border-t border-slate-800 pt-4">
							<h3 class="mb-2 text-xs font-medium text-slate-400">Access codes</h3>
							<div class="flex gap-6 text-sm">
								<span>
									<span class="font-semibold tabular-nums">{summary.access_codes.unused}</span>
									<span class="text-slate-500"> unused</span>
								</span>
								<span>
									<span class="font-semibold tabular-nums">{summary.access_codes.used}</span>
									<span class="text-slate-500"> used</span>
								</span>
								<span>
									<span class="font-semibold tabular-nums">{summary.access_codes.total}</span>
									<span class="text-slate-500"> total</span>
								</span>
							</div>
						</div>
					{/if}
				</section>
			</div>

			<!-- ── Top event types ─────────────────────────────────────────────── -->
			<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
				<h2 class="mb-4 text-sm font-medium text-slate-300">Top telemetry events</h2>
				{#if summary.top_event_types.length > 0}
					{@const maxN = maxEventCount(summary.top_event_types)}
					<div class="space-y-1.5">
						{#each summary.top_event_types as ev (ev.event_type)}
							{@const pct = Math.round((ev.count / maxN) * 100)}
							<div class="flex items-center gap-3">
								<span class="w-44 truncate text-right text-xs text-slate-400">{ev.event_type}</span>
								<div class="flex-1 overflow-hidden rounded-full bg-slate-800">
									<div class="h-2 rounded-full bg-violet-500" style="width: {pct}%"></div>
								</div>
								<span class="w-10 text-right text-xs text-slate-300 tabular-nums"
									>{ev.count.toLocaleString()}</span
								>
							</div>
						{/each}
					</div>
				{:else}
					<p class="text-xs text-slate-500">No events recorded yet.</p>
				{/if}
			</section>

			<!-- ── Participant table ───────────────────────────────────────────── -->
			<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
				<h2 class="mb-4 text-sm font-medium text-slate-300">
					Participants ({summary.participants.length})
				</h2>
				<div class="overflow-x-auto">
					<table class="w-full text-xs">
						<thead>
							<tr class="border-b border-slate-800 text-left text-slate-500">
								<th class="pr-4 pb-2 font-medium">Code</th>
								<th class="pr-4 pb-2 font-medium">Condition</th>
								<th class="pr-4 pb-2 font-medium">Status</th>
								<th class="pr-4 pb-2 font-medium">Started</th>
								<th class="pr-4 pb-2 font-medium">Last seen</th>
								<th class="pr-4 pb-2 font-medium">Completed</th>
								<th class="pr-4 pb-2 font-medium">Duration</th>
								<th class="pr-4 pb-2 font-medium">Events</th>
								<th class="pb-2 font-medium">Survey items</th>
							</tr>
						</thead>
						<tbody>
							{#each summary.participants as p (p.participant_code)}
								<tr class="border-b border-slate-800/50 hover:bg-slate-800/30">
									<td class="py-2 pr-4 font-mono">{p.participant_code}</td>
									<td class="py-2 pr-4">{p.agent_condition}</td>
									<td class="py-2 pr-4">
										<span
											class="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 capitalize"
											class:bg-green-900={p.status === 'completed'}
											class:text-green-300={p.status === 'completed'}
											class:bg-blue-900={p.status === 'in_progress'}
											class:text-blue-300={p.status === 'in_progress'}
											class:bg-slate-800={p.status === 'provisioned' || p.status === 'consented'}
											class:text-slate-300={p.status === 'provisioned' || p.status === 'consented'}
											class:bg-red-900={p.status === 'abandoned'}
											class:text-red-300={p.status === 'abandoned'}
										>
											{p.status.replace('_', ' ')}
										</span>
										{#if likelyDropped(p)}
											<span
												class="ml-1.5 rounded-full bg-amber-900/60 px-2 py-0.5 text-amber-300"
												title="in progress but no activity for over an hour">likely dropped</span
											>
										{/if}
									</td>
									<td class="py-2 pr-4 text-slate-400">{formatDate(p.provisioned_at)}</td>
									<td class="py-2 pr-4 text-slate-400" class:text-amber-400={likelyDropped(p)}
										>{lastSeenAgo(p.last_event_at)}</td
									>
									<td class="py-2 pr-4 text-slate-400">{formatDate(p.completed_at)}</td>
									<td class="py-2 pr-4 tabular-nums">{formatDuration(p.duration_seconds)}</td>
									<td class="py-2 pr-4 tabular-nums">{p.event_count}</td>
									<td class="py-2 tabular-nums">{p.survey_items_answered}</td>
								</tr>
							{/each}
							{#if summary.participants.length === 0}
								<tr>
									<td colspan="9" class="py-4 text-center text-slate-500">No participants yet.</td>
								</tr>
							{/if}
						</tbody>
					</table>
				</div>
			</section>
		</div>
	{/if}
{/snippet}

<!-- ════════════════════════════════════════════════════════════════════════ -->
<!--  Tools — exports + code minting (shared across both views)               -->
<!-- ════════════════════════════════════════════════════════════════════════ -->
{#snippet ToolsSection()}
	<div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
		<!-- Exports -->
		<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
			<h2 class="mb-1 text-sm font-medium text-slate-300">Export datasets</h2>
			<p class="mb-4 text-xs text-slate-500">
				CSV files ready for R / pandas / Excel. Follow-up contact PII is never exported.
			</p>
			<div class="flex flex-wrap gap-3">
				{#each ['participants', 'events', 'survey'] as dataset (dataset)}
					<button
						onclick={() => downloadDataset(dataset)}
						disabled={!!downloadingDataset}
						class="flex items-center gap-2 rounded-md border border-slate-700 bg-slate-800 px-4 py-2 text-xs font-medium text-slate-200 hover:border-slate-500 hover:bg-slate-700 disabled:opacity-50"
					>
						{#if downloadingDataset === dataset}
							<span
								class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-slate-500 border-t-white"
							></span>
						{:else}
							↓
						{/if}
						study_{dataset}.csv
					</button>
				{/each}
			</div>
		</section>

		<!-- Mint access codes -->
		<section class="rounded-lg border border-slate-800 bg-slate-900 p-5">
			<h2 class="mb-1 text-sm font-medium text-slate-300">Mint access codes</h2>
			<p class="mb-4 text-xs text-slate-500">
				Single-use participation codes, one per line — hand them out to recruit participants.
			</p>
			<div class="flex flex-wrap items-center gap-3">
				<input
					type="number"
					min="1"
					max="500"
					bind:value={mintCount}
					class="w-20 rounded-md border border-slate-700 bg-slate-800 px-2 py-2 text-xs text-white focus:border-slate-500 focus:outline-none"
				/>
				<button
					onclick={mintCodes}
					disabled={minting || mintCount < 1 || mintCount > 500}
					class="flex items-center gap-2 rounded-md border border-slate-700 bg-slate-800 px-4 py-2 text-xs font-medium text-slate-200 hover:border-slate-500 hover:bg-slate-700 disabled:opacity-50"
				>
					{#if minting}
						<span
							class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-slate-500 border-t-white"
						></span>
					{/if}
					Mint codes
				</button>
				{#if mintedCodes}
					<button
						onclick={downloadMintedCodes}
						class="rounded-md border border-slate-700 px-3 py-2 text-xs text-slate-300 hover:border-slate-500"
					>
						↓ download .txt
					</button>
				{/if}
			</div>
			{#if mintError}
				<p class="mt-2 text-xs text-red-400">{mintError}</p>
			{/if}
			{#if mintedCodes}
				<pre
					class="mt-3 max-h-40 overflow-auto rounded-md border border-slate-800 bg-slate-950 p-3 text-xs text-slate-300">{mintedCodes}</pre>
			{/if}
		</section>
	</div>
{/snippet}

<!-- ── Reusable stat card snippet ─────────────────────────────────────────── -->
{#snippet StatCard(props: { label: string; value: number | string; sub?: string })}
	<div class="rounded-lg border border-slate-800 bg-slate-900 p-4">
		<p class="text-xs text-slate-400">{props.label}</p>
		<p class="mt-1 text-2xl font-semibold tabular-nums">
			{typeof props.value === 'number' ? props.value.toLocaleString() : props.value}
		</p>
		{#if props.sub}
			<p class="mt-0.5 text-xs text-slate-500">{props.sub}</p>
		{/if}
	</div>
{/snippet}
