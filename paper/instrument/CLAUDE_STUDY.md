# CLAUDE.md — Weld Study Build

> **Purpose of this file.** This is an engineering spec for the *research study build* of Weld, not the product. It tells Claude Code what must be implemented so that a single participant can complete a ~40-minute solo, self-serve user study inside Weld and have their behavior logged. The study measures how an integrated developer workspace (board + chat + GitHub in one app) affects productivity, cognitive load, and subjective experience versus fragmented multi-tool workflows.
>
> The product architecture lives in the main `CLAUDE.md`. This file only covers what's *new or changed* for the study. When in doubt, prefer the smallest change that makes the study runnable — this is a time-boxed (~1 month) research effort, not a product feature.

---

## 0. Guiding Constraints (read first)

1. **Solo + self-serve.** No live teammates, no scheduling. A participant clicks a link, consents, and completes everything alone. Anything requiring real-time multi-user interaction is out of scope.
2. **Deterministic, identical seed.** Every participant must start from a byte-identical project state ("PetTracker"). Resetting/cloning that state per participant must be reliable and automatic.
3. **No live GitHub API during the study.** All GitHub content is seeded fixture data rendered inside Weld. Write actions are hidden. Nothing a participant does can corrupt shared state. (Rationale: the live integration supports writes, so a shared repo would be corrupted by participant #1.)
4. **The bot is optional.** Build it only after everything else works. The study must be fully valid and completable with the bot disabled.
5. **Privacy by design.** Log only what §5 lists. No keystrokes, no mouse movement, no screen/camera capture, no other-tab data. This scope is what keeps the study in exempt/expedited IRB territory.
6. **Feature-flagged.** All study behavior lives behind a `STUDY_MODE` flag so the product build is unaffected.

---

## 1. What "Done" Looks Like

The study build is complete when a fresh participant can, via a single link, do all of the following without researcher intervention:

- [ ] Land on a study entry page, give consent, complete a screening/demographics form.
- [ ] Be provisioned an isolated, identical copy of the PetTracker seed project.
- [ ] Be guided through Phase 1 (5 onboarding tasks) and Phase 2 (3 workflow tasks) via an in-app task panel, then a 5-minute free exploration (Phase 3).
- [ ] Have every relevant action logged with timestamps (§5), tagged with their anonymized participant ID and the current task.
- [ ] Complete an exit survey (§7).
- [ ] Reach a clean "thank you / done" state, with all their data persisted and retrievable for analysis.
- [ ] Do all of the above with GitHub writes impossible and the seed uncorrupted for the next participant.

---

## 2. Components to Build

### 2.1 `STUDY_MODE` feature flag
A global flag (env var + runtime config) that gates all study behavior. When off, Weld behaves as the normal product. When on:
- Write controls for GitHub are hidden (see §4).
- The task panel (§3) is shown.
- Telemetry logging (§5) is active.
- New-participant provisioning (§2.2) is enabled.
- Navigation is constrained to the PetTracker project only.

### 2.2 Participant provisioning & seed isolation
The single most important infrastructure piece. Each participant needs their own isolated copy of the seed so their actions never affect anyone else's view.

Requirements:
- On study entry (after consent), generate an anonymized `participant_id` (e.g., `P0001`) and create a fresh, isolated instance of the PetTracker project seeded from a canonical fixture (§6).
- Isolation can be per-participant project rows scoped by `participant_id`, or a cloned workspace — whatever is simplest given the existing data model. The hard requirement: **participant A's edits are invisible to participant B.**
- Provisioning must be idempotent and fast (sub-second ideally) so the self-serve flow doesn't stall.
- Provide a way to wipe/regenerate a participant's seed (for re-runs and pilots).

### 2.3 In-app task panel
A persistent, non-intrusive panel (sidebar or pinned card) that drives the participant through the scripted tasks.

Requirements:
- Displays the current task text (verbatim from §3 of the study tasks doc; load from a config/JSON so wording can change without code edits).
- An **"I'm done with this task"** button that timestamps task completion and advances to the next task.
- Tracks and logs: task shown timestamp, task completed timestamp, skip events, and how many times the participant re-opened/expanded the instruction (a confusion proxy).
- Shows phase progress (e.g., "Task 3 of 8").
- Phase transitions are logged.
- Must not block the rest of the UI — participants work in the real board/chat/GitHub views while the panel sits alongside.

### 2.4 Consent + screening + survey flow
Web pages (can be in-app routes or a lightweight separate flow) for:
- **Consent screen** — displays the IRB-approved consent text; participant must affirm before provisioning. Log consent timestamp + version.
- **Screening/demographics** — short form (§7.5). Gate eligibility (e.g., ≥1 year programming experience).
- **Exit survey** — NASA-TLX, SUS, custom integration items, open-ended (§7). Can be in-app or an embedded external form (e.g., a hosted survey) — but if external, the participant_id must be passed through so responses can be joined to telemetry.
- **Done screen** — confirms completion, optionally shows compensation instructions.

### 2.5 Telemetry logging layer
Structured event logging for every item in §5. See §5 for the event schema and the non-negotiable exclusions.

### 2.6 Mocked GitHub content + write lockdown
See §4. Render seeded GitHub fixtures in the existing GitHub view; hide all write affordances under `STUDY_MODE`.

### 2.7 (Optional) Scripted bot
See §8. Build last, only if §2.1–2.6 are solid.

---

## 3. The Tasks (load from config, do not hardcode)

Store task definitions in a JSON/config file so wording is editable without redeploying. Structure each task as: `{ id, phase, order, title, instruction_text, completion_type }`.

**Phase 1 — Onboarding (5 tasks):**
1. `p1_orient` — Find and open the card "Add vaccination reminders," read it, close it.
2. `p1_create` — Create a Backlog card "Set up unit tests for feeding tracker," label `testing`, self-assign.
3. `p1_crossref` — From Sarah's #pettracker chat message, open the linked card and move it to In Progress. *(critical cross-reference task)*
4. `p1_message` — Post in #pettracker that they've picked up the bug, @-mention Sarah. *(optional bot trigger #1)*
5. `p1_github` — Open the GitHub view, locate the PR "Refactor feeding schedule API." (view only)

**Phase 2 — Workflow (3 tasks):**
1. `p2_triage` — Read 3 `new-issue` Backlog cards + recent chat, set priorities on all three, post a prioritization rationale in #pettracker. *(highest-value task)*
2. `p2_handoff` — Take over Sarah's card "Add vaccination reminders," read linked chat thread, update the card description with current status. *(optional bot trigger #2)*
3. `p2_decision` — Read the Postgres-vs-SQLite thread, create a card capturing the decision + rationale, link to the discussion if the feature exists.

**Phase 3 — Open exploration (1 timed block):**
1. `p3_explore` — 5-minute free exploration, no task. Auto-advances after 5 min or on a "done exploring" click.

---

## 4. GitHub Handling (critical-path item)

**First, verify:** can the existing GitHub view render from local/seeded data, or is it hardwired to call the live GitHub API?

- **If it can render from local data:** seed the fixtures (§6) into Weld's DB and point the view at them under `STUDY_MODE`.
- **If it's hardwired to the live API:** either (a) add a data-source switch so `STUDY_MODE` reads from seed fixtures, or (b) fall back to a real repo connected with a **read-only token**, plus graceful handling of any attempted write. Option (a) is strongly preferred for determinism; (b) is an acceptable fallback if time is short.

**Write lockdown (required either way):** Under `STUDY_MODE`, hide/disable all GitHub write controls — create issue, create PR, comment, close/merge, edit. Participants (including in free exploration) must have no write affordances and must never see a write-failure error. Linking a Weld card to an *existing* seeded issue/PR is allowed (it writes to Weld's data, not GitHub).

**Acceptance check:** with `STUDY_MODE` on, there is no UI path that mutates GitHub state, and the GitHub view shows identical content for every participant on every load.

---

## 5. Telemetry Schema

Every event: `{ event_type, participant_id, current_task_id, timestamp, payload }`. Persist to a store that's easy to export as CSV/JSON for analysis.

### Session metadata
`session_start`, `session_end`, `phase_started` (payload: phase), `phase_ended`, `session_abandoned` (payload: last_task_id), plus browser/OS/screen-resolution captured once at start.

### Per-task timing
`task_shown`, `task_completed`, `task_skipped`, `instruction_reopened` — all carry `current_task_id`.

### Board events
`card_created`, `card_opened`, `card_edited`, `card_moved` (payload: from_column, to_column), `card_assigned` (payload: assignee), `card_labeled`, `card_priority_set`, `card_deleted`, `card_searched` (payload: query), `board_filtered` (payload: filter_type).

### Chat events
`message_sent` (payload: channel, length, contains_mention, contains_link), `message_edited`, `message_deleted`, `channel_switched`, `thread_opened`, `mention_clicked`, `link_clicked` (payload: target_type ∈ {card, pr, issue, external}).

### Cross-reference events (MOST IMPORTANT — instrument explicitly)
`card_referenced_from_chat`, `chat_referenced_from_card`, `pr_referenced_from_card`, `card_referenced_from_pr`. These power the headline integration metric (cross-reference rate). If they don't already exist, building them is a priority.

### GitHub events
`github_view_opened`, `pr_opened`, `issue_opened`, `pr_linked_to_card`, `issue_linked_to_card`, `pr_review_opened`, `pr_review_submitted`, `pr_merged`.

### Navigation events
`view_changed` (payload: from_view, to_view), `search_used` (payload: query, results_clicked_index).

### Bot events (if built)
`bot_message_sent` (payload: trigger_id, voiced_teammate) — flagged so bot messages are excluded from participant behavioral analysis.

### MUST NOT LOG (hard exclusions)
No keystroke-level capture (store only final submitted message text). No mouse-movement/hover events. No screen or camera recording. No data from other browser tabs. No data about any project other than the participant's PetTracker instance.

---

## 5A. Complete Data Inventory (authoritative)

This is the single source of truth for *everything the study collects*. Four categories. Each row of analysis data must be joinable to one participant via `participant_id`. Nothing outside this inventory should be collected.

### Category A — Behavioral telemetry (auto-logged events)
Captured live as the participant works. Full event list in §5. Grouped:
- **Session & timing:** session_start/end, phase transitions, abandonment point, per-task shown/completed/skipped timestamps, instruction re-opens, browser/OS/screen-resolution (once).
- **Feature actions:** all board, chat, GitHub, and navigation events in §5.
- **Cross-reference events:** the four `*_referenced_from_*` events — highest priority, they power the headline metric.
- **Bot events:** `bot_message_sent` (flagged, excluded from participant behavior), only if the bot is built.

### Category B — Survey responses (self-report)
Mostly collected once at the end (full spec in `../replication/WELD_EXIT_SURVEY.md`): NASA-TLX (6), SUS (10), integration custom items (5), open-ended (4), demographics (8), optional follow-up consent (2), plus `survey_started`/`survey_completed` timestamps. **Exception:** the 6 NASA-TLX items are *also* collected once mid-session at the baseline→agent boundary (item ids suffixed `_baseline`, via `POST /api/v1/study/interim-survey`), giving a within-person manual-vs-agent load delta. Both sets land in `study_survey_responses` keyed by `item_id`.

### Category C — Derived metrics (computed at analysis time, not logged directly)
These are *calculated from Category A* during analysis. Listed here so you can verify the event stream is rich enough to produce them. **If a derived metric below can't be computed from the logged events, the event schema is incomplete — fix it before launch.**

| Derived metric | Computed from | Answers |
|---|---|---|
| Time-to-first-action per phase | task_shown → first action event | Onboarding friction |
| Per-task completion time | task_shown → task_completed | Productivity (H: integrated faster) |
| Task completion rate | task_completed vs. task_skipped + rubric (Category D) | Productivity |
| **View-switch rate** (switches/min) | sequence of `view_changed` events | **Context-switching — core metric** |
| **Cross-reference rate** (% card opens via a link) | `card_opened` vs. `*_referenced_from_*` events | **Integration usage — headline metric** |
| Chat-consulted-before-acting (Tasks 2.1–2.3) | ordering of channel/message events vs. card edits | Whether integration informs decisions |
| Search reliance | `card_searched` / `search_used` frequency | Navigation strategy |
| Perception–behavior gap | view-switch rate (A) vs. `int_switch_perceived` (B) | **Most publishable finding** |

### Category D — Manual-coded data (researcher-scored post-hoc)
Not collected by the app; scored by two researchers from Category A + B after sessions. See §6 of the tasks doc / the rubric. Stored in a separate analysis spreadsheet keyed by `participant_id`:
- Per-task completion score (0–3 rubric).
- Context-reconstruction behavior in Tasks 2.2/2.3 (built-on-existing vs. recreated-manually).
- Coding of open-ended responses into themes (thematic analysis).
- Notable confusion moments flagged for qualitative writeup.

### How it all joins
- `participant_id` is the universal key across A, B, C, D.
- Identifiable contact info (follow-up email) lives **only** in a separate access-restricted store, linked by a token, never in the analysis dataset, deleted after the follow-up window.
- Final analysis export: one row per participant with all Category A summaries, B responses, and C/D derived/coded values; open-ended text and contact info handled per the privacy rules above.

### Data handling
- Store on encrypted institutional storage; access limited to the research team.
- Use `participant_id` (e.g., `P0001`) everywhere — no names in the analysis dataset.
- Strip any inadvertently identifying content from open-ended responses before publication.
- Define a minimum-completion threshold (recommend: finished through Phase 2) for inclusion in primary analysis; log enough to apply it.

---

## 6. Seed Content Fixtures

Canonical fixture data, version-controlled, loaded at provisioning. (Actual copy/text is a separate content deliverable; this spec defines the shape.)

- **Board:** ~12 cards across Backlog / In Progress / Done with titles, descriptions, labels, priorities. 3 cards link to seeded GitHub issues; 2 link to seeded PRs. Includes the specific cards the tasks reference ("Add vaccination reminders," the 3 `new-issue` cards, etc.).
- **Chat:** #general and #pettracker channels, ~30 timestamped messages authored by the 4 fake teammates, including: a message from Sarah referencing a bug card (for `p1_crossref`), a status thread on the handoff card (for `p2_handoff`), and the Postgres-vs-SQLite decision thread started by Alex (for `p2_decision`). Some messages contain card/PR links to exercise cross-references.
- **Teammates:** 4 fake users (Sarah, Alex, Priya, Marcus) with names + avatars. Authors of seed content; inactive unless the optional bot voices one.
- **GitHub fixtures:** 3 open issues, 2 open PRs (incl. "Refactor feeding schedule API"), realistic commit history, a README — as local data rendered in the GitHub view.

All fixtures must be defined once and reproduced identically per participant. Timestamps should be relative to provisioning time (e.g., "yesterday") so the content always feels recent.

---

## 7. Survey Instruments

Full item-level spec (exact wording, scales, item_ids, scoring) is in the companion file **`../replication/WELD_EXIT_SURVEY.md`** — implement against that. Summary of what the exit survey collects, all joined to telemetry by `participant_id`:

- **NASA-TLX** — 6 raw subscales (Mental, Physical, Temporal Demand; Performance; Effort; Frustration), 21-point scales. `item_id` prefix `tlx_`.
- **SUS** — standard 10 items, 5-point. `item_id` prefix `sus_`. Note `sus_05` ("well integrated") is research-relevant; report individually.
- **Integration custom items** — 5 Likert items (`int_findacross`, `int_noticed`, `int_vs_separate`, `int_reuse`, `int_switch_perceived`). `int_switch_perceived` is the perceived counterpart to the behavioral view-switch-rate metric — keep both for the perception–behavior gap analysis.
- **Open-ended** — 4 free-text prompts (`open_useful`, `open_frustrating`, `open_change`, `open_integration`). Store final submitted text only — no keystroke capture.
- **Demographics/background** — 8 bucketed fields (`dem_*`): age range, student status, experience, languages, prior PM tools, prior chat tools, GitHub frequency, prior integrated-workspace use. Collect only these.
- **Optional follow-up consent** — `followup_ok` + optional `followup_contact` email, stored in a **separate access-restricted table**, linked by token, deleted after the follow-up window.

Record `survey_started` and `survey_completed` as telemetry events. Persist partial responses on each page advance so an abandon mid-survey still captures completed pages.

---

## 8. Optional Bot Spec (build last)

Trigger-based, single canned response per trigger, disable-able via one config flag. Study must run identically with it off.

- **Trigger 1 — after `p1_message`:** when the participant @-mentions Sarah, after ~20s post a fixed message voicing Sarah: *"Thanks for grabbing that! I left some notes on the card."*
- **Trigger 2 — during `p2_handoff`:** when the participant updates the handoff card, after ~25s post a fixed message voicing Marcus: *"Appreciate you picking this up while Sarah's out 👍"*

Rules: one response per trigger, no branching, no NLP, no reacting to anything else. If a trigger never fires, the bot stays silent. All bot messages carry the `bot_message` flag. If this takes more than ~a day, ship without it.

---

## 9. Build Order (priority)

1. `STUDY_MODE` flag scaffolding.
2. Participant provisioning + seed isolation (§2.2) — nothing works without identical, isolated seeds.
3. Seed fixtures (§6) — needed to test everything else.
4. GitHub render-from-seed + write lockdown (§4) — verify the API-vs-local question *first*, it's critical path.
5. Telemetry layer (§5), with cross-reference events prioritized.
6. In-app task panel (§3).
7. Consent / screening / exit-survey / done flow (§2.4).
8. End-to-end pilot pass (one researcher completes the whole study; verify data lands cleanly).
9. (Optional) Bot (§8).

---

## 10. Out of Scope (do not build for the study)

- Real-time multi-user collaboration during a session.
- Live GitHub writes of any kind.
- Time-tracking, secrets management, file upload, mobile/desktop/TUI shells (unless a task explicitly needs them — none currently do).
- Any analytics beyond the §5 schema.
- Open public signup (study is invite-only via the study link; public signup can come after).

---

## 11. Acceptance Checklist (pre-launch)

- [ ] A fresh participant link runs end-to-end with no researcher intervention.
- [ ] Two participants in parallel never see each other's data; each seed is identical at start.
- [ ] `STUDY_MODE` off = normal product; on = full study behavior.
- [ ] No UI path mutates GitHub state; GitHub view identical on every load.
- [ ] Every task logs shown/completed timestamps; cross-reference events fire correctly.
- [ ] Every Category C derived metric (§5A) can actually be computed from the logged event stream — verify on pilot data before launch.
- [ ] All four data categories (A behavioral, B survey, C derived, D manual-coded) join cleanly on `participant_id`.
- [ ] Excluded data (keystrokes, mouse, screen, other tabs) is verifiably *not* collected.
- [ ] Exit survey responses join to telemetry by `participant_id`; partial responses persist per page.
- [ ] Follow-up contact info stored separately from analysis data, linked only by token.
- [ ] Data exports cleanly to CSV/JSON for analysis.
- [ ] Bot (if present) can be toggled off and the study is unaffected.
