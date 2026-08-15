# PetTracker Seed Content — Design & Task Mapping

This document explains the seed fixture (`pettracker_seed.json`) that every study participant loads at provisioning. It's the frozen "team context" that makes Weld's integration story visible from the first screen. Review this with your professor before locking the content.

## Design principles

1. **Every task has its anchors.** Each piece of content a task depends on is tagged with `task_anchor` in the JSON. These must not be renamed or removed without updating the task config. They are the load-bearing content.
2. **The integration story is built into the seed.** Chat messages reference board cards via `[[card:...]]` links; cards link to GitHub issues/PRs. This is what lets cross-reference behavior happen at all — without it, there's nothing to cross-reference.
3. **Realistic but domain-free.** PetTracker is concrete enough to feel like a real project but needs zero special knowledge. No participant is advantaged by prior domain expertise.
4. **Recent and alive.** All timestamps are relative offsets (in minutes) from provisioning time, so the content always looks like it happened over the past few days, never stale.
5. **Decisions have a clear answer.** The one decision task (Postgres vs. SQLite) is written so the team visibly converges on SQLite — the participant is reconstructing a settled decision, not inventing one. This keeps the task deterministic and codeable.

## The fake team

| Name | Role | Purpose in the study |
|---|---|---|
| Sarah Chen | Member (backend) | Authors the bug report (cross-ref task) and the handoff status thread. Goes "on leave" to motivate the handoff task. |
| Alex Rivera | Admin (tech lead) | Starts the database decision thread; posts the triage prompt. |
| Priya Nair | Member (frontend) | Background activity, adds realism to chat. |
| Marcus Webb | Member (full-stack) | Authors the API refactor PR (GitHub task). Optionally voiced by the bot for trigger #2. |

The participant joins as a 5th member at provisioning. The four teammates are inactive during the session unless the optional bot voices one.

## Board: 12 cards

Distributed Backlog (6) / In Progress (3) / Done (3) to look like a project mid-sprint.

**Task-anchored cards (do not alter without updating tasks):**

- `card_vaccination_reminders` ("Add vaccination reminders") — used by **p1_orient** (the card participants find first) and **p2_handoff** (the card they take over from Sarah). In Progress, assigned to Sarah, links to issue #14.
- `card_login_crash` ("Login screen crashes on empty email") — used by **p1_crossref**. This is the card Sarah's chat message links to; the participant opens it *from chat* and moves it to In Progress.
- The three `card_new_issue_*` cards (sync bug, timezone bug, typo) — used by **p2_triage**. All tagged `new-issue`, all with `priority: null` so the participant is the one who sets priorities. They're deliberately of different real severity (data-loss > timezone > cosmetic typo) so prioritization reveals reasoning.

**Supporting cards** (realism + things to link to): feeding schedule UI, vet directory, db decision placeholder, API refactor, plus three Done cards (onboarding, pet profile, CI setup).

## Chat: ~16 messages across 2 channels

`#general` has light standup chatter for ambient realism. `#pettracker` carries all the task-relevant content:

- **p1_crossref anchor** (`msg_010`): Sarah reports the login crash and links the card. The participant must find this message and click through to the card.
- **p2_handoff anchors** (`msg_020`, `msg_021`): Sarah's detailed status notes on the vaccination card + her announcement she's going on leave. This is the context the participant reads before taking over.
- **p2_decision anchors** (`msg_030`–`msg_033`): Alex opens the Postgres-vs-SQLite question; Marcus and Priya weigh in; Alex concludes with SQLite and asks someone to record the decision. The participant reconstructs this into a board card.
- **p2_triage anchor** (`msg_050`, `msg_051`): Alex points at the three new-issue cards and nudges on prioritization; Priya adds an opinion. Gives the participant chat context to consult before setting priorities.

The `[[card:card_id]]` syntax marks where a chat message links to a board card — render these as live cross-reference links. They're what make the cross-reference telemetry events fire.

## GitHub: mocked fixtures (no live API)

Rendered in Weld's GitHub view from seed data only. Write controls hidden under `STUDY_MODE`.

- **3 issues:** #14 (vaccination 7-day reminder, links to the handoff card), #31 (login crash), #33 (timezone bug).
- **2 PRs:** #22 (feeding UI) and #27 ("Refactor feeding schedule API") — **#27 is the p1_github anchor**, the PR participants are asked to locate.
- **5 commits:** realistic history so the repo view doesn't look empty.

## Task → content quick reference

| Task | Content it depends on |
|---|---|
| p1_orient | `card_vaccination_reminders` |
| p1_create | (participant creates new card; no seed dependency) |
| p1_crossref | `msg_010` → `card_login_crash` |
| p1_message | `#pettracker` channel, Sarah as @-mention target |
| p1_github | PR #27 in GitHub fixtures |
| p2_triage | 3 `card_new_issue_*` cards + `msg_050`/`msg_051` |
| p2_handoff | `card_vaccination_reminders` + `msg_020`/`msg_021` + issue #14 |
| p2_decision | `msg_030`–`msg_033` thread |
| p3_explore | (no dependency — free exploration) |

## Things to review with your professor

- **Severity ordering in the triage task.** The three new issues are designed so a thoughtful participant ranks data-loss (sync) highest and the typo lowest. If you want triage to be more ambiguous (harder to code "correct" prioritization), adjust the descriptions. Right now it's intentionally codeable.
- **The decision is pre-settled (SQLite).** This makes p2_decision a reconstruction task, not a judgment task. If you'd rather test the participant's own decision-making, leave the thread unresolved — but that makes scoring harder.
- **Content volume.** ~16 chat messages is enough to feel alive without overwhelming a 40-minute session. If pilots show participants ignoring chat, add 2–3 more messages that make the cross-references more enticing.
- **Names/avatars.** Swap any names if you'd prefer a different/more diverse set; just keep the IDs stable since tasks reference them.

## Implementation notes for Claude Code

- **The fixture:** `crates/weld-server/study/pettracker_seed.json`, embedded into the server with `include_str!`. That is the only copy and therefore the authoritative one — this document describes it, it does not duplicate it. (There was a second copy alongside these docs; it was byte-identical and has been deleted, because two copies of a fixture can only ever drift.) Seed Deltas D1 (`msg_d1`), D2 (`msg_d2`), and D4 are in it; D3 (GitHub disambiguation) is satisfied by the existing PR/issue fixtures.
- Load this fixture at participant provisioning (see `../instrument/CLAUDE_STUDY.md` §2.2, §6).
- Convert every `*_offset` (minutes) to an absolute timestamp at load time, relative to `now`.
- Render `[[card:card_id]]` tokens in message content as live links that emit `card_referenced_from_chat` when clicked.
- The fixture must reproduce identically per participant; no shared mutable state.
