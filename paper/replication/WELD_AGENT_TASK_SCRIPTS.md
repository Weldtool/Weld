# Weld Agent Study — Task Scripts, Correctness Rubric & Trust Survey (v2.0)

Concrete scripts for the 12-task session: 6 manual baseline tasks (C0, everyone) + 6 agent tasks (in the participant's assigned condition C1/C2/C3). All tasks run on the PetTracker seed (`pettracker_seed.json`). Task text is loaded from config (`apps/web/src/lib/study-tasks.ts`) so wording is editable without code changes.

**Design logic recap.** Baseline tasks establish each person's manual speed/load. The 6 agent tasks span three use-case types — comprehension/advice, single-surface action, cross-surface action — about two each. The prediction (H2) is that conditions look similar on comprehension and single-surface tasks but **diverge on cross-surface tasks**, where C3's GitHub+board context should pull ahead and C1 (chat-only) should struggle. The single-surface and comprehension tasks are controls that show context breadth doesn't *universally* help — which is what makes the cross-surface finding credible.

**Seed additions needed.** A few tasks need content not in the base seed; these are listed in §4 "Seed Deltas." All Seed Deltas are already implemented; see §4 for confirmation.

---

## Phase 0 — Manual Baseline (C0, everyone, no agent)

Agent UI is hidden. Participant works manually. Six tasks (expanded from two). They mirror the agent tasks' structure but use *different content* to avoid teaching the participant the agent-task answers. The expanded battery does double duty: (a) it measures **RQ1 — the value of the all-in-one workspace itself** — directly and without the agent confound (cross-reference usage, card→PR traversal into the code surface, chat-consulted-before-acting, view-switching, all done by hand), and (b) more baseline observations give a **stabler per-person normalizer** for the agent tasks. All baseline content is disjoint from the agent tasks (it reuses only the unused seed cards: `card_vet_directory`, `card_db_decision`, `card_ci_setup`, `card_feeding_schedule_ui`, `card_pet_profile`, and PR #22), so it cannot leak an agent-task answer.

### B1 — Baseline comprehension + single action
> "Read the recent messages in #general. Priya mentioned one card she's blocked on. Find that card on the board and move it to the 'In Progress' column."

- **Type:** `single_surface` (chat → board); surfaces: `chat`, `board`.
- **Anchors:** the #general message from Priya naming the blocked card (`card_feeding_schedule_ui`, Seed Delta D1) + the referenced card.
- **Captures:** manual time + actions for a simple read-then-act task. Normalizer baseline.

### B2 — Baseline cross-surface
> "A teammate asked in #general for the status of the 'Pet profile creation' work. Check the board to find that card's current column, then post a one-sentence status reply in #general."

- **Type:** `cross_surface`; surfaces: `chat`, `board`.
- **Anchors:** `card_pet_profile` (in Done) + a #general status request (Seed Delta D2).
- **Captures:** manual time/effort for a multi-surface task — the harder baseline, used to normalize the cross-surface agent tasks specifically.

### B3 — Baseline cross-reference (chat link → board)
> "In #pettracker, Alex flagged the 'Nearby vet directory' card and linked it in his message. Open that card directly from the link in his message, then set its priority to High."

- **Type:** `cross_surface`; surfaces: `chat`, `board`.
- **Anchors:** a #pettracker message from Alex containing `[[card:card_vet_directory]]` (Seed Delta D4) + `card_vet_directory` (Backlog, priority currently Low).
- **Captures:** the **headline integration metric manually** — does the participant open the card via the in-chat link (`card_referenced_from_chat`) rather than hunting for it on the board? Then `card_priority_set`.

### B4 — Baseline GitHub integration (card → PR → review → chat)
> "On the board, open the 'Feeding schedule editor UI' card and open the pull request attached to it. Review its changes, then post its PR number in #general along with a one-line assessment."

- **Type:** `cross_surface`; surfaces: `github`, `chat`.
- **Anchors:** seeded PR #22 "Feeding schedule editor UI" (pre-linked to `card_feeding_schedule_ui` via `github_pr: 22`) + #general.
- **Captures:** card→PR traversal (`pr_referenced_from_card`), in-app review (`pr_review_opened` / `pr_review_submitted`, plus `pr_merged` if they take the merge step), and view-switching for a task that spans the code surface. Correct answer is the PR number (22) plus a defensible one-line assessment of the diff.
- **Note on explicitness:** this instruction names the route, unlike the agent tasks. That is deliberate and safe here — baselines are a per-person normalizer, identical for every condition, so explicitness biases nothing differentially. Agent tasks must state goals only (see T1–T6).

### B5 — Baseline chat-thread → board edit (decision capture)
> "Earlier in #pettracker the team discussed which local cache database to use and reached a decision. Find the 'Decide local cache database' card on the board and update its description to record what they decided."

- **Type:** `cross_surface`; surfaces: `chat`, `board`.
- **Anchors:** the Postgres-vs-SQLite decision thread in #pettracker (`msg_030`–`msg_033`) + `card_db_decision` (Backlog).
- **Captures:** chat-consulted-before-acting — the participant must read the thread, extract the decision (SQLite), and write it onto the card (`card_edited`).

### B6 — Baseline board navigation → chat
> "Using the board, find the card about setting up the CI pipeline and open it. Then post a one-sentence reply in #general confirming what it does (for example, that tests run on every pull request)."

- **Type:** `comprehension`; surfaces: `board`, `chat`.
- **Anchors:** `card_ci_setup` (Done; description: GitHub Actions runs tests + lint on every PR) + #general.
- **Captures:** board navigation/search to locate a card (`card_opened`), then a cross-surface reply — a lighter comprehension baseline matching the agent Type-A tasks.

---

## Phase 1 — Agent Tasks (assigned condition, 6 tasks)

The participant has the agent available, scoped to their condition. **The same task text is shown to all conditions** — only what the agent can see/do differs. That's the manipulation. For each task below: the instruction is verbatim; the "expected good path" notes how the agent *could* help if it has the context; the "context dependency" states which condition should have the advantage.

> **Condition-neutral wording (important).** Each instruction states the *goal* ("a bug needs to be moved to In Progress") rather than commanding "ask the agent to move it." This is deliberate: an action-imperative phrasing is impossible for a C1 (advise-only) agent, which leaves C1 participants stuck thinking the task is broken — confounding the very delegation friction we want to measure. Goal-first wording works for every condition: a C1 participant does the manual step themselves (the measured friction), a C3 participant has the agent do it. The wording is identical across conditions; only the agent's capability differs.

> **Two further wording rules, both load-bearing.**
>
> 1. **Availability, not instruction.** Instructions end "The assistant is available if you want it," never "use the assistant to help." RQ2 measures how much participants *choose* to delegate; telling them to delegate measures compliance instead. The cost is exposure — some participants may barely use the agent — which is handled analytically (intention-to-treat primary, as-used secondary; see the analysis plan's limitations).
> 2. **State the goal, not the route.** Never name the surfaces and steps ("open this card, then attach the PR, then post in #pettracker"). A scripted route lets the participant navigate without the agent's context, which is the variable under test, collapsing condition differences into UI mechanics. Baseline tasks *may* be explicit — they are a normalizer, equally explicit for every condition — but agent tasks must not be. Enforced alongside the config in `apps/web/src/lib/study-tasks.ts`.

### Type A — Comprehension / Advice (2 tasks)

**T1 — What needs doing?**
> "Figure out what the team most needs done next, based on the recent discussion, then make that change on the board (for example, set a priority or move a card). The assistant is available if you want it."

> *Wording note:* names a concrete board change as the completion point (so the participant has a clear "done", reducing time-variance) without dictating *which* change — preserving condition-neutrality.

- **Type:** `comprehension`; surfaces: `chat`, `board`.
- **Context dependency:** LOW. Chat alone contains the triage prompt (`msg_050`/`msg_051`) and decision context. C1 should be competitive here.
- **Expected good path:** agent reads chat, identifies the three new-issue cards / the prioritization need, advises the user. User acts.
- **Cross-surface bonus:** C2/C3 can *also act* (set priorities) rather than just advise — note whether broader conditions finish faster because the agent does the action.

**T2 — Catch me up.**
> "You're returning after being away. Get caught up on what's happened recently and what's waiting on you, then post a short 2–3 sentence summary in #general of where things stand. The assistant is available if you want it."

> *Wording note:* the posted summary is a concrete, scorable artifact and a clear stopping point — previously T2 had no observable completion, making its time and correctness noisy.

- **Type:** `comprehension`; surfaces: `chat`, `board`, `github`.
- **Context dependency:** LOW–MEDIUM. Chat carries most of the story; board/GitHub add detail (open PRs, card states).
- **Expected good path:** C1 summarizes from chat; C2 adds board state (what's in progress, assigned to whom); C3 adds open PRs/issues. Watch whether richer summaries (C2/C3) reduce the user's follow-up actions.

### Type B — Single-Surface Action (2 tasks)

**T3 — Draft a status message** (chat-surface)
> "A short status update on where the vaccination reminders work stands needs to be posted in #pettracker. The assistant is available if you want it."

- **Type:** `single_surface`; surfaces: `chat`.
- **Context dependency:** LOW (lives in chat). Extra board/GitHub context shouldn't help much — this is a control.
- **Expected good path:** all conditions can read the handoff thread (`msg_020`/`msg_021`) and draft. C2/C3 can post it directly (or propose a draft for the participant to confirm).

**T4 — Reorganize the board** (board-surface)
> "Three new-issue cards in the Backlog need sensible priorities set. The assistant is available if you want it."

- **Type:** `single_surface`; surfaces: `board`.
- **Context dependency:** MEDIUM. Requires board access to *act*; C1 (chat-only, advise-only) can only tell the user what to do — the user must do it all manually. C2/C3 can set priorities directly.
- **This is a key contrast for C1 vs C2:** same task, but C1 can't touch the board, so the user does the clicking. Measures the cost of advise-only vs. act-capable on a board task.

### Type C — Cross-Surface Action (2 tasks) — where the ladder should diverge

**T5 — Take the bug from chat to the board.**
> "A bug Sarah reported in #pettracker needs to be moved to In Progress. The assistant is available if you want it."

- **Type:** `cross_surface`; surfaces: `chat`, `board`.
- **Context dependency:** HIGH for C1 vs others. The bug is in chat (`msg_010`) and links to `card_login_crash`. C1 can read the chat and *tell* the user, but can't open/move the card — user does it manually. C2/C3 can resolve the chat→card reference and move it themselves.
- **Prediction:** C2/C3 markedly faster/lower-effort than C1. Core H1/H2 evidence.

**T6 — Close the loop with GitHub.** (the C3-differentiating task)
> "The vaccination reminders work isn't connected to its code yet, and it hasn't been reviewed. Sort that out and let the team know in #pettracker. The assistant is available if you want it."

- **Type:** `cross_surface`; surfaces: `chat`, `board`, `github`.
- **Context dependency:** HIGHEST — spans chat + board + GitHub. The relevant PR is **#34 "Add advance vaccination reminders (7-day lead)"**, deliberately left unlinked in the seed; the card is `card_vaccination_reminders` (In Progress). PRs #22 and #27 ship pre-linked to other cards, so #34 is the only unattached one and linking it is the task. The note goes in #pettracker.
- **Completion:** card ↔ PR #34 linked (`pr_linked_to_card`), a review submitted on #34 (`pr_review_submitted`, any verdict), and a #pettracker message recording the link. Three parts; the rubric scores them separately. Merging is *optional* — see the review-outcome note below — so a card that ends in Done is a legitimate finish, not an extra requirement.
- **Prediction:** C3 (has GitHub) can do this end-to-end via `github_link`. C2 (no GitHub) cannot resolve the PR — agent must punt to the user or fail. C1 even more limited. **This task is what justifies the top rung of the ladder** — if C3 doesn't beat C2 here, the GitHub context isn't pulling its weight, and you'd see that in the data.
- **Note:** make sure the GitHub fixtures are rich enough that finding the right PR requires actually having GitHub context (not guessable from chat alone). See Seed Delta D3.
- **Surface note:** there is no separate GitHub panel — issues/PRs live on the card they belong to (chip on the card face, link + in-app review inside the card). The unlinked PR #34 is reachable through the card's "Link a PR" picker. This keeps the workspace to two surfaces, and means a participant who never opens a card never encounters the code surface at all.

### Review outcomes drive the board (study mode)

The in-app review view mirrors what GitHub's webhooks do to a linked card, because study mode has no webhooks and a review that changed nothing would teach a workflow that doesn't exist:

| Action in the review view | Linked card |
|---|---|
| **Approve** | *nothing* — approval is a signal, not a ship |
| **Merge pull request** (offered only after an approval) | → Done |
| **Request changes** (comment required) | → In Progress, i.e. handed back to the author |
| **Comment** | unchanged |

This is the two-step GitHub model on purpose: "I approved it, why isn't it shipped?" is a real thing juniors hit, and the separation is worth teaching. Provisioning wires `in_progress_column_id` / `done_column_id` to the seeded columns (`provision.rs`), without which the merge would record but move nothing. Card movements arrive as ordinary `card_moved` events, so who-did-it attribution is unchanged.

### Task order

Present in a fixed order within each condition: T1, T2 (comprehension) → T3, T4 (single-surface) → T5, T6 (cross-surface), so difficulty ramps and the cross-surface divergence comes when the participant is fluent with the agent. Log per-task timing regardless of order. (If you prefer to counterbalance order to control for fatigue/learning, randomize within-type pairs but keep type blocks together; record the order per participant.)

---

## 2. Correctness Rubric

Speed is meaningless without correctness — always score both. Each task scored 0–3 by two independent coders. Score the *final state* the participant produced, regardless of whether the agent or the user did it.

**Scoring procedure (pre-specified — see `../WELD_ANALYSIS_PLAN.tex` §Correctness).** Correctness is the only primary measure produced by human judgement, so the procedure is fixed before data collection: coders work from a final-state export with `agent_condition`, the agent transcript, and all timing **stripped** (blinding is mandatory — knowing a participant was in C3 while judging their output is a direct route to confirming H1); **all** tasks are double-coded, not a 20% sample, since 12 × N is small enough to cover completely; reliability is reported as **quadratic-weighted** Cohen's κ (the scale is ordinal — a 3-vs-2 disagreement is not a 3-vs-0 disagreement) plus exact agreement, taken **pre**-adjudication; disagreements are then resolved to consensus, still blind. The rubric is piloted on pilot data and frozen before any real session is scored.

| Score | Meaning |
|---|---|
| 0 | Not attempted / no meaningful progress |
| 1 | Attempted, wrong outcome (e.g., moved the wrong card, posted irrelevant text) |
| 2 | Partially correct (right idea, incomplete — e.g., set 2 of 3 priorities; linked PR but didn't post the note) |
| 3 | Fully correct |

**Per-task correctness anchors (what a 3 requires):**
- **B1:** the specific card Priya named (`card_feeding_schedule_ui`) is in In Progress.
- **B2:** status reply posted in #general correctly states the card's actual column.
- **B3:** `card_vet_directory`'s priority is set to High (bonus signal: opened via the in-chat link, not by board search).
- **B4:** the correct PR number (22) is posted in #general **and** accompanied by a one-line assessment that is defensible against the actual diff (a review was submitted in-app is a supporting signal, not required for a 3; the posted assessment is what is scored).
- **B5:** `card_db_decision`'s description records the actual decision (SQLite for the local cache).
- **B6:** a #general reply correctly states what CI does (tests/lint run on every PR).
- **T1:** user acted on a genuinely top-priority item consistent with chat context.
- **T2:** (advice task) score the *usefulness/accuracy* of the summary the user received + whether they acted appropriately; a 3 = accurate summary, no major omissions.
- **T3:** a coherent, accurate status message about vaccination reminders posted to #pettracker.
- **T4:** all three new-issue cards have priorities set, ordering defensible vs. their descriptions (data-loss/sync > timezone > typo).
- **T5:** `card_login_crash` is in In Progress.
- **T6:** three parts — PR #34 linked to `card_vaccination_reminders`, a review submitted on #34 (any verdict), and a note in #pettracker recording the link. 3 = all three; 2 = two of three; 1 = one of three or the wrong PR/card; 0 = nothing.

**Offline scoring from logs.** Each task emits `task_final_state` immediately before `task_completed`. `task_final_state` contains a full snapshot of the board (all card columns, priorities, labels, GitHub links) and all loaded chat channels (message content included — this is participant-authored task output, not the keystroke/screen capture §5 forbids). Coders can score correctness from this snapshot alone without accessing the live database.

**Also record, separately from correctness:**
- **Who did it:** agent-performed vs. user-performed vs. mixed (from telemetry — drives RQ1 delegation analysis).
- **Agent error type** if the agent failed: wrong target, hallucinated content, refused/couldn't (lacked context), or tool error. Especially log when C1/C2 *couldn't* do a cross-surface step due to scoping — that's signal, not noise.

---

## 3. Trust-in-Agent Survey Block

Implemented in `apps/web/src/lib/survey-config.ts` as the "The AI assistant" section. 7-point Likert (1 = strongly disagree … 7 = strongly agree) unless noted. Adapted from standard trust-in-automation constructs (reliability, predictability, reliance willingness).

| item_id | Statement |
|---|---|
| trust_reliable | The AI agent was reliable. |
| trust_predictable | I could predict what the AI agent would do. |
| trust_competent | The AI agent was capable of doing what I needed. |
| trust_rely | I would be comfortable relying on the AI agent without double-checking its work. |
| trust_context | The AI agent had enough context to actually help me. |
| trust_again | I would want to use this AI agent for my own real work. |

**Behavioral-vs-perceived check (custom, tied to RQ3/H5 perception–behavior + RQ2/H4 reliance):**

| item_id | Statement | Scale |
|---|---|---|
| trust_verify | I felt I had to check or redo what the agent did. | 1 = never … 7 = constantly |
| trust_speed_perceived | Using the agent made me faster. | 1 = much slower … 7 = much faster |

> `trust_context` is the key item for the context manipulation — expect it to rise C1 → C2 → C3 if the ladder works. `trust_speed_perceived` vs. the *actual* baseline-normalized time is your perception–behavior gap for the agent (H5): do users in act-capable conditions *feel* faster even when correctness/time says otherwise? `trust_rely` vs. `trust_verify` captures the over-/under-reliance tension (H4) the uploaded literature flags.

**Full survey structure (as implemented):**
1. Task load — NASA-TLX (6 items, 0–100 sliders)
2. Usability — SUS (10 items, 5-point Likert)
3. Working across the app — integration items (5 items, 7-point Likert): `int_findacross`, `int_noticed`, `int_vs_separate`, `int_reuse`, `int_switch_perceived` (reverse-coded: "I felt like I was constantly switching between separate tools")
4. The AI assistant — trust block above (8 items, 7-point Likert)
5. In your own words — 4 open-text items
6. About you — demographics (age, student status, experience, languages, tools, GitHub frequency, prior all-in-one tool use)
7. Follow-up (optional) — willingness + contact email (stored in `study_followup_contacts`, excluded from analysis export by design)

Keep this block under ~90 seconds. Watch total survey length in piloting; if fatigue shows, SUS can be trimmed before the trust block (trust is now central to the RQs).

---

## 4. Seed Deltas (add to `pettracker_seed.json` before launch)

The base seed covers most anchors. Add:

- **D1 (for B1):** a `#general` message from Priya saying she's blocked on a specific card, e.g. *"I'm blocked on the feeding schedule editor until the API refactor lands — going to switch to something else."* referencing `card_feeding_schedule_ui`. (B1 asks the user to find and move that card.)
- **D2 (for B2):** a `#general` message from a teammate asking *"Quick — what's the status on pet profile creation? Did that ship?"* (B2 asks the user to check `card_pet_profile`'s column and reply.)
- **D3 (for T6):** ensure the GitHub fixtures make the *right* PR identifiable only with GitHub context — e.g., chat refers vaguely to "the code for the vaccination/feeding work" while PR #27's title/body in the GitHub fixtures is what actually disambiguates it. This guarantees C3's GitHub access is genuinely required, so the C2-vs-C3 contrast is real.
- **D4 (for B3):** a `#pettracker` message from Alex linking the vet-directory card — *"Side note — can someone take a look at the nearby vet directory card? `[[card:card_vet_directory]]` … Let's bump its priority so it doesn't get lost."* — so B3 has a real in-chat card reference to open. (Implemented as `msg_b3` in the seed.)
- Keep all new messages authored by existing teammates with relative timestamps, consistent with the seed's conventions.

> **All Seed Deltas D1, D2, D4 are already in the runtime fixture** (`crates/weld-server/study/pettracker_seed.json`, the `include_str!`-embedded source of truth). D3 (GitHub disambiguation) is satisfied by the existing PR/issue fixtures.

> Note: B1/B2 reference different content than the agent tasks (feeding-editor + pet-profile) vs. the agent tasks' login-crash/vaccination/triage content — intentional, to avoid the baseline teaching the agent-task answers.

---

## 5. Telemetry these tasks require (cross-check with agent build spec §5)

Each task must produce, per participant: per-task start/end timestamps, agent invocations with scoped-context proof, agent vs. user action attribution, accept/reject/undo of agent output, and final-state capture for rubric scoring. The `agent_condition` and `current_task_id` tag every event. If any of T1–T6 can't be reconstructed for correctness + who-did-it + timing from the logs, the telemetry is incomplete — fix before launch.

### Key events (implemented)

| Event | When emitted | Key payload fields |
|---|---|---|
| `session_start` | Tab load, study active | browser metadata, viewport size |
| `consent_given` | Participant completes intro | `consent_version` |
| `task_shown` | Task panel advances to a new task | `task_id`, `task_type`, `phase`, `surfaces` |
| `task_final_state` | Just before `task_completed` / `task_skipped` | full board snapshot + loaded chat messages |
| `task_completed` | Participant clicks "Done" | `task_id`, `task_type`, `task_time_ms` |
| `task_skipped` | Participant clicks "Skip" | `task_id`, `task_type`, `task_time_ms` |
| `task_rating` | Participant submits per-task rating | `task_id`, `task_type`, `phase`, `effort` (1–7), `agent_help` (1–7 or null for baseline) |
| `agent_invoked` | Agent turn starts | `task_id`, `condition` |
| `agent_context_sent` | Gate assembles scoped context | `condition`, `sources_included`, full chat/board/github snapshot |
| `agent_response` | Model replies | `reply`, `actions`, `entities`, token counts |
| `agent_tool_call` | Executor runs a tool | `tool`, `args`, `result` |
| `agent_tool_rejected` | Tool blocked by gate | `tool`, `condition`, reason |
| `agent_draft_sent` | Participant sends agent-drafted chat | `channel_id`, `content` |
| `agent_draft_edited` | Participant edits before sending | `channel_id`, diff |
| `card_referenced_from_chat` | Participant clicks `[[card:...]]` link | `card_id`, `channel_id` |
| `view_changed` | Participant switches between board/chat/github | `from`, `to` |
| `survey_completed` | Exit survey submitted | item count |
| `session_end` | Clean exit (survey done) | — |
| `session_abandoned` | Tab closed mid-study (pagehide / beforeunload) | `stage`, `task_index` |

**`task_final_state` note.** The snapshot is emitted by `apps/web/src/lib/study-snapshot.ts` and includes: all board columns + all task fields (id, title, column, priority, assignee, labels, github links) + all server-confirmed messages in loaded channels (content included, optimistic pending messages excluded). Coders score correctness from this event; the live DB is a backup.

**`session_abandoned`** is emitted on-unload via a keepalive `fetch` (synchronous-style). It does not indicate data loss — events buffered before unload were flushed in the same call. It distinguishes true dropouts (no `survey_completed` after) from refreshes (participant resumes via access code).

### Per-task-type analysis support (H2)

So the condition × task-type interaction (H2 — does context breadth help more on cross-surface than comprehension tasks?) is first-class rather than hand-mapped:

- **`task_type`** (`comprehension` | `single_surface` | `cross_surface`) and the **`surfaces`** the task spans are defined on every task in `study-tasks.ts` and stamped onto `task_shown` / `task_completed` / `task_skipped` / `task_rating`. Slicing any per-task behavioral measure by task type needs no manual ID→type mapping.
- **`task_rating`** (logged after each task, before advancing — so it never inflates the measured task time): a single **effort** item (1–7) on every task, plus an **agent-help** item (1–7, "how much did the assistant help with this task?") on agent tasks only (`agent_help: null` for baseline). This is the per-task short-form of the cognitive-load measure (per the Measures section's "NASA-TLX per-task short form") and gives the subjective half of H2/H3 — e.g. *cross-surface tasks felt less demanding and the agent felt more helpful under C3 than C1*. Behavioral half (time, delegation, cross-references) is already per-task via `current_task_id`.

---

## 6. Agent Implementation Details

Relevant for interpreting what the agent can and cannot do in each condition, and for troubleshooting telemetry.

### Context Gate (`crates/weld-server/src/services/study/agent/gate.rs`)

The chokepoint that assembles scoped context. Default-deny: disallowed sources are never queried.

| Condition | Chat | Board | GitHub | Tools |
|---|---|---|---|---|
| C1 | ✓ | — | — | none (advise-only) |
| C2 | ✓ | ✓ | — | `board_write`, `chat_post` |
| C3 | ✓ | ✓ | ✓ | `board_write`, `chat_post`, `github_link` |

Scoping is proven by unit tests in `gate.rs` (not a runtime check — it's a compile-time-readable enum match) and by the `agent_context_sent` event, which records exactly which sources were assembled.

### Model and limits

- **Model:** `gpt-5.4-2026-03-05` (the `OPENAI_MODEL` default in `config.rs`, and the snapshot named in the paper's methods; overridable via the env var — but changing it mid-study is a confound, so don't). Earlier drafts of this doc named `gpt-4o-mini`; the pin moved and the code is authoritative.
- **Temperature:** configurable via `AgentConfig`, pinned for the session.
- **Max output tokens:** 800 per completion.
- **Max tool round-trips per turn:** 5 (guards against loops).
- **Max agent messages per participant (whole session):** 50 (cost guardrail; a participant who hits this gets a clear error, recorded as an `agent_error` event with `error: "quota_exceeded"`. Deliberately *not* an `agent_invoked` event — that one is the denominator of the invocation counts and of the quota itself).
- **Retry:** 1 retry on transport error per completion call.

### Agent output: drafts vs. direct actions

- **C2/C3 board writes** (`move_card`, `set_priority`, `link_github`): executed directly against the service layer + published via the realtime hub. The board updates immediately.
- **C2/C3 chat posts**: the agent proposes a draft (`AgentDraft`). The participant sees it in the AgentPanel and sends/edits it before it posts. This preserves authorship clarity and prevents the agent from posting on the participant's behalf without their review — which matters for `trust_verify` scoring. The client emits `agent_draft_sent` or `agent_draft_edited` depending on whether the content was changed.
- **C1**: no tools. The reply is text-only advice; every action is the participant's.

### Session resume

If a participant re-enters their access code on a new device (or clears localStorage), `POST /api/v1/study/provision` detects the already-spent code, resumes the existing participant account, and returns `resumed: true` with `resume_stage` and `resume_task_index`. The study store restores their saved position in the stage machine rather than starting over. Progress is persisted server-side via `POST /api/v1/study/progress` after each task transition.

---

## 7. Data Export & Researcher Dashboard

### Dashboard

`/study-dashboard` (web route, gated by `STUDY_MODE`) shows live aggregate statistics: participant count per condition, completion/dropout rates, per-task completion counts.

### Export API

`GET /api/v1/study/export?dataset=<name>` — requires `Authorization: Bearer <STUDY_EXPORT_TOKEN>`. When `STUDY_EXPORT_TOKEN` is unset, the endpoint returns 404 (exports are off by default).

| Dataset | Contents |
|---|---|
| `participants` | One row per participant: code, condition, timestamps, stage, completed flag |
| `events` | One row per behavioral telemetry event (bulk) |
| `survey` | Long format — one row per (participant, survey item) |
| `access_codes` | Code roster: which codes exist, whether spent, by whom |

The follow-up contact table (`study_followup_contacts`) is **deliberately excluded** — it is access-restricted PII and must not appear in the analysis dataset.

### Access codes

Codes are pre-minted by the researcher via `POST /api/v1/study/access-codes` (body: `{ count, note }`). Format: `WELD-XXXX-XXXX` using a no-ambiguity alphabet (no 0/O/1/I/L). Each code is single-use: provisioning validates and spends it in the same transaction, so a code can never provision twice under concurrent requests. Set `STUDY_REQUIRE_ACCESS_CODE=true` to enforce code-gated participation.
