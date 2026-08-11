# Weld Agent Study — Research Documentation & Paper-Drafting Reference

**For:** Dr. Jie Wu and Andrew Sadler
**Status:** Working reference, v1.0 — consolidates the study design, the complete data inventory with rationale, the task set, the analysis plan, and scaffolding for a paper draft.
**One-line summary:** A controlled study of how beneficial it is to consolidate task, communication, and code context in one workspace and make it accessible to an AI agent — operationalized as a context ladder (chat-only → +board → +GitHub) and measured by agent effectiveness and by user speed/effort/trust against each participant's own manual baseline, inside an integrated developer workspace called Weld.

---

## 1. Motivation & Research Questions

Software development is fragmented across many tools (issue trackers, chat, code hosts), and AI coding assistants are increasingly embedded in that workflow. A recurring open question in the literature is how much *context* an assistant needs to be useful: prior work shows repository-level context consistently improves an agent's ability to infer developer intent, and that developers' *perceptions* of AI assistance often diverge from their measured behavior. Weld — a workspace integrating task management, team chat, and (mocked) GitHub in one application — lets us manipulate exactly how much of a shared workspace an AI agent can see and act on, and measure the downstream effect on the user.

**RQ1 (primary) — Agent scope → effectiveness.** Does widening the agent's scope over the workspace (chat → +board → +GitHub) make the user more effective, and on which tasks?

**RQ2 — Reliance.** How much do users delegate to vs. verify and override the agent, and does that shift with its scope?

**RQ3 — Trust vs. reality.** Does users' trust in the agent track its measured effectiveness, or diverge from it?

> Scope discipline: the only thing manipulated is the agent's **scope** over the workspace (the C1→C2→C3 ladder), measured against each participant's own no-agent baseline (C0). Two claims are deliberately out of reach and are never made. (a) *All-in-one beats separate tools* — there is no fragmented-tools arm. (b) *Context breadth alone drives the effect* — visibility and capability rise together by construction (an agent cannot act on a board it cannot see), so C3-vs-C1 confounds seeing with doing; the contrast that isolates context is C2-vs-C3, which this N can estimate but not test. We therefore say *agent scope*, not *context breadth*, throughout. Consolidation is the motivation; the agent's scope ladder is what we measure.

**Hypotheses.**

- **H1 (RQ1).** Wider scope yields shorter baseline-adjusted agent-task times *across conditions*. Not stated as an absolute gain over C0: phase order is fixed, so an agent−baseline delta also contains ordinary practice (§6.6).
- **H2 (RQ1).** The gain is *task-dependent* — minimal on comprehension and single-surface tasks, substantial on cross-surface tasks spanning chat → board → GitHub.
- **H3 (RQ1).** Wider scope reduces self-reported cognitive load relative to baseline more than narrow scope.
- **H4 (RQ2).** Delegation to the agent rises with scope, but reliance is imperfectly calibrated — users do not verify or override precisely where the agent is actually wrong.
- **H5 (RQ3, perception–behavior).** Trust and perceived speedup may not track the agent's measured effectiveness — users may trust an agent that *does more* even when it isn't faster or is occasionally wrong.

## 2. Study Design

**Hybrid: within-subjects baseline + between-subjects agent condition.**

- Every participant first completes **6 manual baseline tasks (B1–B6, C0)** with no agent — establishing their own manual speed and cognitive load on each task type (B1 single-surface; B2–B5 cross-surface; B6 comprehension). A post-baseline NASA-TLX is collected here, before any agent exposure.
- Each participant is then **randomly assigned to one agent condition** and completes **6 agent tasks (T1–T6)** in that fixed condition.

**The three agent context conditions (the independent variable):**

| Condition | Agent can SEE | Agent can DO |
|---|---|---|
| **C1 — Chat-only** | Chat only | Advise only; user executes |
| **C2 — Chat + Board** | Chat + board | Advise + act on board/chat |
| **C3 — Full context** | Chat + board + GitHub | Act across chat, board, and (mocked) GitHub |

**Design rationale (for the methods section):**

- *Why hybrid.* The per-person C0 baseline lets each participant's agent-task performance be analyzed as a *delta from their own manual speed*, absorbing individual skill differences that would otherwise sit between the between-subjects conditions and sap power. Six baseline tasks (one per agent task type, with four cross-surface) give a type-matched, stable per-person anchor rather than one generic warm-up.
- *Why between-subjects on condition.* Holding the agent's access fixed per person avoids within-person contamination (once you've seen the full-context agent solve a task, you can't un-see how it should go) and keeps the session within a self-serve ~55–65-minute budget.
- *Why capability scales with context.* An agent cannot act on a board it cannot see, so visibility and capability are intentionally coupled. This bundling *is* the phenomenon under study ("does giving the agent more access help the user"); it is stated as a design decision, not treated as a confound.
- *Apparatus.* All participants use the same fictional seed project ("PetTracker"); only the agent's context differs. GitHub is mocked/seeded inside the app (no live API, no repo, write controls hidden), so participation is fully self-serve and nothing a participant does corrupts another's environment. The agent runs on a pinned, dated OpenAI model snapshot at fixed temperature (0.2) with versioned prompts, so the model is a controlled, reproducible variable; the exact snapshot string is recorded here once the pilot settles it and held constant for the entire study. Model capability is a design parameter, not an ops detail: the C3 arm can only convert its wider scope into performance if the model is strong enough to use it, so a snapshot too weak would floor the very condition under study and yield an uninterpretable null. The pilot verifies C2/C3 tool-calling before the choice is fixed.

## 3. Participants

- **Target N:** ~20–30 adults, 6 agent-task observations per participant plus baselines provide additional power.
- **Population:** adults 18+, ≥1 year programming experience, basic familiarity with issue trackers, team chat, and GitHub. Student-focused recruitment, not student-limited.
- **Recruitment:** self-serve link via course lists, departmental channels, developer communities.

## 4. The Task Set

Twelve tasks — six manual baselines (B1–B6) then six agent tasks (T1–T6) — ~4–5 minutes each (~55–65 min + survey). All on the PetTracker seed. The agent tasks span three use-case types so we can locate *where* agent scope matters; comprehension and single-surface tasks serve as controls demonstrating that wider scope does not *universally* help — which is what makes the cross-surface result credible. Each baseline is built to match an agent task type, giving a like-for-like per-person normalizer (below).

**Manual baseline (C0, everyone, no agent — disjoint content from the agent tasks so it can't teach their answers):**

- **B1** — single-surface (read #general, find a blocked card, move it to In Progress).
- **B2** — cross-surface (chat status request → check a card's column → post a status reply).
- **B3** — cross-surface (open a card from a chat link → set its priority).
- **B4** — cross-surface (open a card → open its attached PR → review it → post its number and an assessment in chat).
- **B5** — cross-surface (recover a decision from chat → record it in a card's description).
- **B6** — comprehension (find the CI-pipeline card on the board → confirm what it does in chat).

**Agent tasks (assigned condition):**

| Task | Type | Context dependency | What it tests |
|---|---|---|---|
| **T1** — "What needs doing?" | Comprehension/advice | Low | Chat-only competitive; do C2/C3 finish faster by also acting? |
| **T2** — "Catch me up" | Comprehension/advice | Low–Med | Do richer summaries (board/GitHub) reduce follow-up work? |
| **T3** — Draft a status message | Single-surface (chat) | Low (control) | Extra context shouldn't help — validates the design |
| **T4** — Set priorities on the board | Single-surface (board) | Medium | C1 (advise-only) vs. C2/C3 (act): cost of advising vs. doing |
| **T5** — Take a bug from chat to the board | Cross-surface | High | C2/C3 resolve chat→card and act; C1 can't — core H1/H2 |
| **T6** — Link the PR to the card + note it in chat | Cross-surface | Highest | Only C3 has GitHub to do it end-to-end — justifies the top rung |

**Key design point for the paper:** T6 is constructed so the correct pull request is identifiable *only* with GitHub context (chat refers to it vaguely; the PR's title/body disambiguates). If C3 does not beat C2 on T6, the GitHub context is not pulling its weight — and the data will show that. This is the cleanest single test of the top of the context ladder.

## 5. Complete Data Inventory (what we collect and why)

Everything joins to one anonymized `participant_id`. Four categories.

### Category A — Behavioral telemetry (auto-logged during the session)

| Data | Why we collect it |
|---|---|
| Session metadata: start/end, `agent_condition`, phase transitions, abandonment point | Frame each session; enable per-condition comparison; handle dropouts |
| Per-task timing: shown/completed/skipped timestamps, **active engaged time** (excludes away/idle, survives reload), `away_ms`/`pause_count`/`was_interrupted`, instruction re-opens | Core productivity measure; active-time keeps a closed-and-reopened task from counting away-time (§6.6); re-opens proxy confusion |
| Board events: create/open/edit/move/assign/label/priority/search | Board actions by user vs. agent; RQ2 supporting evidence (division of labor) |
| Chat events: message sent (length, has-mention, has-link), edits, channel switches, thread opens, mention/link clicks | Communication & navigation; RQ2 supporting evidence |
| GitHub-view events: view opened, issue/PR opened, card↔issue/PR links | Whether/how GitHub context is used (esp. C3) |
| Navigation: view changes, searches | View-switch rate (context-switching measure) |
| **Cross-reference events** (card-from-chat, chat-from-card, PR-from-card, card-from-PR) | **Headline integration-usage metric**; how context is traversed |
| **Agent events:** invocation, `agent_context_sent` (snapshot of exactly what the agent received), response (latency, type, # tool calls, model, temp, prompt/completion/total tokens), each tool call (name, allowed?, executed?), tool-rejected anomalies | Agent effectiveness (RQ1); **`agent_context_sent` is the proof that scoping held** — without it the context manipulation rests on trust, not evidence |
| User-to-agent reactions: suggestion accepted/rejected, action undone, output edited | How broader context shifts work from user to agent; RQ2 supporting evidence + over/under-reliance |

### Category B — Survey responses (self-report, once at the end)

| Instrument | Why |
|---|---|
| NASA-TLX (6 subscales) | Cognitive load (H3); validated, comparable to literature |
| System Usability Scale (10 items) | Overall usability; comparable to published norms; `sus_05` ("well integrated") is research-relevant |
| Integration items (5) | Perceived ease of working across surfaces |
| **Trust-in-agent block (6 + 2 items)** | Agent trust, reliability, reliance willingness; `trust_context` should rise C1→C2→C3; `trust_speed_perceived` vs. actual time = perception–behavior gap (H5) |
| Open-ended (4 prompts) | Qualitative depth; most-useful/most-frustrating moments; thematic analysis |
| Demographics (8 bucketed fields) | Sample description; covariates (experience, prior tool use) |
| Optional follow-up consent | Recruit interview subset; stored separately |

### Category C — Derived metrics (computed from A at analysis time)

| Metric | From | Serves |
|---|---|---|
| Time-to-first-action; per-task completion time | A timing | Productivity |
| Task completion rate | A + rubric (D) | Productivity |
| **View-switch rate** | A navigation | Context-switching |
| **Cross-reference rate** | A cross-ref vs. card-open | Integration usage |
| **Baseline-normalized task time** (agent task ÷ own C0) | A + baseline | **Core RQ1 comparison across C1/C2/C3** |
| Agent delegation ratio (agent- vs. user-performed actions) | A agent + action events | RQ2 (how much work the user offloads to the agent) |
| Agent latency, tool-call counts | A agent events | Agent efficiency/cost |
| **Perception–behavior gap** (perceived speed/trust vs. normalized time) | A + B | **H5 — likely the most novel finding** |

### Category D — Manual-coded data (researcher-scored post-hoc)

| Coded item | Why |
|---|---|
| Per-task correctness (0–3 rubric) | Speed is meaningless without correctness — a fast-but-wrong agent isn't effective |
| Who-did-it (agent / user / mixed) | RQ2 (division of labor as context unifies) |
| Agent error type (wrong target / hallucination / couldn't-due-to-scoping / tool error) | Diagnose *why* a condition underperforms; "couldn't due to scoping" is signal, not noise |
| Thematic coding of open-ended responses | Qualitative findings; two coders, report Cohen's kappa on a sample |

### What we deliberately do NOT collect (privacy + IRB)

No keystroke logging (final submitted text only), no mouse-movement/hover tracking, no screen or camera recording, no data from other browser tabs, no data about any project other than the participant's PetTracker instance. This exclusion list is both an ethical commitment and what keeps the study in exempt/expedited IRB territory.

### Data handling

Encrypted institutional storage; access limited to the research team; pseudonymous IDs throughout; follow-up contact emails (if any) stored separately, linked by token, deleted after the follow-up window; open-ended text screened for identifying content before any release.

## 6. Analysis Plan

This plan is written against the fields actually collected (telemetry event types + payload keys, survey `item_id`s). Each construct names its source so the analysis is reproducible from the event log + survey export.

### 6.1 Measures (operationalized)

| Construct | Source field(s) | Notes |
|---|---|---|
| **Task completion time** (primary outcome) | `task_completed.task_time_ms` | **Active** engaged time, not wall-clock — see §6.6. Clock stops *before* the per-task rating, so rating time can't inflate it. |
| Wall / away time, interruptions | `task_completed.{wall_ms, away_ms, pause_count, was_interrupted}` | Audit + exclusion flags; `was_interrupted` gates sensitivity analyses. |
| Time-to-first-action | first surface event after `task_shown` − `task_shown` ts | Derived; no extra instrumentation. |
| Completion vs. abandonment | `task_completed` / `task_skipped`; `session_abandoned` | Per-task and per-session. |
| **Correctness (0–3)** | manual coding of `task_final_state` (board + message content) + `study_agent_turns` + DB | Two coders, Cohen's κ on a shared subset. |
| Per-task effort (within-person load) | `task_rating.effort` (1–7, **every** task incl. baselines) | The baseline-anchored H3 measure. |
| Perceived agent help | `task_rating.agent_help` (1–7, agent tasks) | Per-task perception. |
| Global cognitive load (per phase) | 6 NASA-TLX subscales collected **twice** — `tlx_*_baseline` (post-baseline interstitial) and `tlx_*` (exit) | The within-person agent−baseline delta is the H3 measure. |
| Usability | `sus_01`…`sus_10` | Standard SUS scoring. |
| Integration perception | `int_findacross, int_noticed, int_vs_separate, int_reuse, int_switch_perceived` | `int_switch_perceived` pairs with the behavioral view-switch rate. |
| Trust / reliance | `trust_reliable, trust_predictable, trust_competent, trust_rely, trust_context, trust_again, trust_verify, trust_speed_perceived` | H5; `trust_verify` reverse-valenced. |
| Agent effectiveness | `agent_response.{latency_ms, response_type, num_tool_calls, prompt_tokens, completion_tokens, total_tokens}`, `agent_tool_call.{allowed, executed, error}` | Effort/cost + success. |
| Delegation / division of labor | `agent_tool_call` (executed) vs. user board/chat action events; `agent_suggestion_accepted/_rejected`, `agent_output_edited`, `agent_action_undone` | RQ2. |
| Cross-reference (integration usage) | `card_referenced_from_chat`, `pr_referenced_from_card`, `card_referenced_from_pr`, `issue_referenced_from_card` | `chat_referenced_from_card` is N/A (no affordance) — not missing data. |
| View-switch rate | count of `view_changed` ÷ active time | Context-switching. |
| **Scoping proof** | `agent_context_sent` snapshot per invocation | Manipulation check (§6.5). |

Derived for the headline comparison: **baseline-normalized time** = agent-task active time ÷ that participant's own *type-matched* C0 baseline — comprehension (T1, T2) ÷ B6; single-surface (T3, T4) ÷ B1; cross-surface (T5, T6) ÷ mean(B2, B3, B4, B5). Every agent task type has a like-for-like anchor; the cross-surface anchor averages four baselines and is the most stable.

### 6.2 Primary model (RQ1 / H1 / H2)

Linear mixed model on log-transformed normalized time:
`log(norm_time) ~ condition * task_type + (1 | participant)`. **H2 is the `condition × task_type` interaction** — does the condition effect concentrate on cross-surface tasks (T5, T6)? Report βs with 95% CIs and cell-level Cohen's d; lead with effect sizes, not p-values. T6 is the single cleanest top-of-ladder test (only C3 has the GitHub context to disambiguate the correct PR); if C3 doesn't beat C2 on T6, the data say GitHub context isn't pulling its weight.

### 6.3 Correctness

Ordinal cumulative-link mixed model: `correctness ~ condition * task_type + (1 | participant)`, always reported beside time (faster-but-wrong ≠ better). Compare correctness distributions per condition × task.

### 6.4 Cognitive load (H3) — two measures, two analyses

NASA-TLX is collected **twice** — a post-baseline interstitial (`tlx_*_baseline`, manual phase) and at exit (`tlx_*`, agent phase) — so it yields a within-person, phase-to-phase delta. Test H3 two ways: (a) **global TLX delta** (primary) ΔTLX = TLX_agent − TLX_baseline on the composite (and per subscale), compared *between* conditions (Welch ANOVA / Kruskal–Wallis given small N); the paired baseline removes the between-person level differences a single end-of-study TLX confounds; (b) **per-task effort** `effort ~ condition * task_type + (1 | participant)` with each participant's baseline-task effort as the anchor (finer-grained, localizes by task type). (a) is the headline load result.

### 6.5 Perception–behavior gap (H5) and manipulation check

H4: per participant, Spearman-correlate `trust_speed_perceived` and a trust composite against measured normalized time and correctness; characterize divergence (e.g., agents that *do more* trusted even when not faster/occasionally wrong). Cross `trust_rely` × `trust_verify` for reliance calibration; pair `int_switch_perceived` (perceived) with behavioral view-switch rate.

**Manipulation check (report first, before any inferential result):** from `agent_context_sent`, verify 100% of C1 snapshots contain no board/GitHub context and 100% of C2 contain no GitHub. This converts the IV from an assumption into evidence.

### 6.6 Timing validity (interruptions and away-time)

Task time is **active engaged time**: the client clock runs only while the tab is visible, pausing on background/close and resuming on return, and persists across reloads/re-entry so away-time is excluded by construction rather than after the fact (`study-task-clock.ts`). Each task additionally logs `away_ms`, `pause_count`, and `was_interrupted`. Pre-specified handling: analyze `task_time_ms` (active) as primary; run a sensitivity analysis excluding `was_interrupted` tasks; report the interruption rate. Cross-device/cleared-storage resumes mark the in-progress task `was_interrupted` (its pre-resume time is unrecoverable) and are excluded from timing models.

**Visible-idle rule (pre-specified).** The clock catches stepping away *from the tab*, not stepping away from a tab left in front. Since sessions are unproctored and cells hold ~8 people, one walk-away can carry a condition. Pre-specified before data collection: within a task, ≥180 s of visible time containing zero telemetry events is treated as idle and subtracted. The threshold sits well above a natural pause for reading a channel or a diff (tasks run ~4–5 min and emit events throughout) and well below a genuine walk-away. Report how many tasks trigger it; fit the primary model with and without the correction, and report if the conclusion depends on which.

**Practice effects bound the baseline claim.** Phase order is fixed — six manual baselines always precede six agent tasks, since agent exposure cannot be undone — so an agent−baseline delta contains ordinary learning as well as agent help. Between-condition inferences are unaffected (identical ordering in every arm), but we never claim the agent beat manual work; H1 is a between-condition difference in baseline-adjusted time.

### 6.7 RQ2 (reliance / division of labor) — descriptives

How do users actually work with the agent? Alongside the primary outcome models, the behavioral telemetry gives a division-of-labor picture — does the user delegate to the agent or take over, and do they check its output? Supporting telemetry descriptives: delegation ratio by condition; the agent funnel (`agent_invoked` → tool attempted → `executed` → `accepted`/`edited`/`undone`); tool error/rejection rates; cross-reference traversal counts; CONSORT-style participant flow from `study_participants.status` + `session_abandoned`. Thematic analysis of the four open-ended items (two coders, report κ on a sample).

Correctness scoring is fixed in advance because it is the one primary measure produced by judgement rather than instrumentation: two coders score **all** tasks from a final-state export with `agent_condition`, the agent transcript, and timing stripped (blinding is mandatory — knowing someone was in C3 while judging their output is a direct route to confirming H1); reliability is quadratic-weighted Cohen's κ on the 0–3 scale reported **pre**-adjudication, with disagreements then resolved to consensus, still blind. The rubric is piloted on pilot data and frozen before any real session is scored.

### 6.8 Small-N strategy and figures

Three between-subjects cells at N≈20–30 (~7–10/cell) is a modest, focused sample (a short full study, not a large trial); the within-person baseline — carried as a covariate in the primary ANCOVA, not pre-divided — and 6 agent observations/participant are what make it viable. Mitigations: condition assignment balanced by adaptive minimization — each participant assigned to the least-filled condition, ties broken at random (`provision.rs::pick_agent_condition`); pre-register on OSF (confirmatory focused contrast C3-vs-C1 on cross-surface + exploratory remainder); report effect sizes + CIs (and consider permutation or Bayesian estimation) over null-hypothesis tests.

**Name the variable *agent scope*, not *context breadth*.** Visibility and capability rise together by construction (an agent can't act on a board it can't see), so C3-vs-C1 — the pre-registered confirmatory contrast — confounds seeing with doing: C1 only advises, C3 acts across three surfaces, and the simplest reading of any gap is that the agent did the clicking. Report it as evidence the manipulation works, not as evidence about context. The contrast that isolates context is C2-vs-C3 (both act, differ only in GitHub scope), estimable but underpowered here, sharpened by T6 where the correct PR is identifiable only with GitHub context. H5 is untouched by the confound, comparing each condition's trust against its own measured effectiveness — which is part of why it carries the paper.

**Two further bounds to state plainly.** (a) *The top rung is narrow*: study-mode GitHub is read-only fixtures with one-to-two-file diffs, so C3's capability beyond C2 is reading small diffs and linking a PR. That caps the detectable C2→C3 effect and limits generalization to real repositories, where review is the expensive part. (b) *One model, one moment*: a single pinned snapshot at fixed temperature buys internal validity at the cost of external validity, and the snapshot must be capable enough that the top rung is floored by the design rather than by the model. **Headline figures:** (1) normalized time, condition × task_type (the H2 interaction plot); (2) correctness, same axes; (3) perception–behavior scatter (trust vs. measured speed); (4) delegation ratio by condition; (5) the scoping manipulation-check table.

## 7. Open Items for the Two of You to Decide

- Pre-registration on OSF (recommended; strengthens the COI story).
- IRB: Do we have to make an IRB application or not?
- Whether to run the optional follow-up interviews (script already drafted).
- Pilot size before launch (recommend ≥2 full runs per condition to verify scoping and timing).
- Do we want to have code entering for the whole project, easy to add and makes it so we don't have the same person doing it multiple times.
