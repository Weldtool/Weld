<script lang="ts">
	import { onMount } from 'svelte';
	import { env } from '$env/dynamic/public';

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
	}

	interface AccessCodeStats {
		total: number;
		used: number;
		unused: number;
	}

	interface StudySummary {
		total_participants: number;
		by_condition: ConditionCount[];
		by_status: StatusCount[];
		top_event_types: EventTypeCount[];
		access_codes: AccessCodeStats;
		participants: ParticipantRow[];
	}

	// ── State ────────────────────────────────────────────────────────────────

	let token = $state('');
	let savedToken = $state('');
	let summary = $state<StudySummary | null>(null);
	let error = $state('');
	let loading = $state(false);
	let downloadingDataset = $state('');

	const SESSION_KEY = 'study_dashboard_token';

	onMount(() => {
		const stored = sessionStorage.getItem(SESSION_KEY);
		if (stored) {
			savedToken = stored;
			token = stored;
			fetchSummary(stored);
		}
	});

	// ── API helpers ──────────────────────────────────────────────────────────

	async function apiGet<T>(path: string, tok: string): Promise<T> {
		const res = await fetch(`${API_BASE}${path}`, {
			headers: { Authorization: `Bearer ${tok}` }
		});
		if (!res.ok) {
			const text = await res.text().catch(() => res.statusText);
			throw new Error(`${res.status}: ${text}`);
		}
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
			error = e instanceof Error ? e.message : String(e);
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
			if (!res.ok) throw new Error(`${res.status}`);
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
			alert(`Download failed: ${e instanceof Error ? e.message : String(e)}`);
		} finally {
			downloadingDataset = '';
		}
	}

	// ── Derived helpers ──────────────────────────────────────────────────────

	function completionPct(c: ConditionCount) {
		return c.total === 0 ? 0 : Math.round((c.completed / c.total) * 100);
	}

	function avgCompletionTime(participants: ParticipantRow[]): string {
		const times = participants
			.filter((p) => p.duration_seconds !== null)
			.map((p) => p.duration_seconds as number);
		if (times.length === 0) return '—';
		const avg = Math.round(times.reduce((a, b) => a + b, 0) / times.length);
		return formatDuration(avg);
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
					<p class="text-xs text-slate-400">Live data · refreshes on page load</p>
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
					value: summary.by_status.find((s) => s.status === 'in_progress')?.count ?? 0,
					sub: 'currently active'
				})}
				{@render StatCard({
					label: 'Avg. completion time',
					value: avgCompletionTime(summary.participants),
					sub: 'completed sessions only'
				})}
			</div>

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

				<!-- Status breakdown -->
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
									</td>
									<td class="py-2 pr-4 text-slate-400">{formatDate(p.provisioned_at)}</td>
									<td class="py-2 pr-4 text-slate-400">{formatDate(p.completed_at)}</td>
									<td class="py-2 pr-4 tabular-nums">{formatDuration(p.duration_seconds)}</td>
									<td class="py-2 pr-4 tabular-nums">{p.event_count}</td>
									<td class="py-2 tabular-nums">{p.survey_items_answered}</td>
								</tr>
							{/each}
							{#if summary.participants.length === 0}
								<tr>
									<td colspan="8" class="py-4 text-center text-slate-500">No participants yet.</td>
								</tr>
							{/if}
						</tbody>
					</table>
				</div>
			</section>

			<!-- ── Data exports ────────────────────────────────────────────────── -->
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
		</main>
	</div>
{/if}

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
