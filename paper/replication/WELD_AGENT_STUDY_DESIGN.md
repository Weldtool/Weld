# Weld Study — AI Agent Scope Study (Design v1.1)

**New focus.** This study examines how the *scope* of an in-app AI agent — how much of the workspace it can see and act on — affects the agent's effectiveness and the user's speed, effort, and trust. It supersedes the earlier human-vs-separate-tools framing operationally, but reuses the existing infrastructure: the PetTracker seed, the privacy model, the telemetry spine, and the consent/survey/recruitment flow (with additions noted here).

---

## Research Questions

The study manipulates one variable — how much of the workspace the in-app agent can see and act on (the C1→C2→C3 ladder) — measured against each participant's own no-agent baseline (C0). The questions stay inside what that supports: the agent's **scope** over the workspace and how users work with it — **not** whether an all-in-one workspace beats separate tools (no fragmented-tools arm), and **not** context breadth with capability held fixed (the two rise together by construction; see the design note below).

- **RQ1 (primary) — Agent scope → effectiveness.** Does widening the agent's scope over the workspace (chat → +board → +GitHub) make the user more effective, and on which tasks?
- **RQ2 — Reliance.** How much do users delegate to vs. verify/override the agent, and does that shift with its scope?
- **RQ3 — Trust vs. reality.** Does users' trust in the agent track its measured effectiveness, or diverge from it?

## Hypotheses

- **H1 (RQ1).** Wider scope yields shorter baseline-adjusted agent-task time **across conditions**. Deliberately *not* stated as an absolute improvement over C0: phase order is fixed (baseline always first), so an agent−baseline delta also contains ordinary practice with the app. Between-condition comparisons are unaffected — every arm gets the identical order — but "the agent made people faster than working manually" is a sentence this design cannot support.
- **H2 (RQ1).** The gain is **task-dependent**: minimal on comprehension and single-surface tasks, substantial on cross-surface tasks spanning chat → board → GitHub.
- **H3 (RQ1).** Wider scope lowers self-reported cognitive load relative to baseline more than narrow scope.
- **H4 (RQ2).** Delegation to the agent rises with scope, but reliance is imperfectly calibrated — users do not verify or override precisely where the agent is actually wrong.
- **H5 (RQ3, perception–behavior).** Trust and perceived speedup need not track the agent's measured effectiveness — users may trust an agent that *does more* even when it isn't faster or is occasionally wrong. (The most publishable angle.)

## Design

**Hybrid: within-subjects baseline + between-subjects agent condition.**

1. Every participant first completes **6 manual baseline tasks (B1–B6, C0)** — no agent. Establishes each person's own manual speed and load on each task type. A post-baseline NASA-TLX is taken here, before any agent exposure.
2. Each participant is then **randomly assigned to ONE agent condition** and completes **6 agent tasks (T1–T6)** in that fixed condition (access never changes within a participant).

**Agent context conditions (between-subjects):**

| Condition | Agent can SEE | Agent can DO |
|---|---|---|
| **C1 — Chat-only** | Chat only | Advise only ("here's what needs doing"); user executes |
| **C2 — Chat + Board** | Chat + board | Advise + act on board/chat |
| **C3 — Full context** | Chat + board + GitHub | Advise + act across chat, board, GitHub |

> **Capability scales with context — by design.** An agent can't act on a board it can't see, so capability and visibility are intentionally coupled. This bundling *is* the phenomenon under study ("does giving the agent more access help the user"). Stated explicitly as a design decision, not a confound.
>
> **Consequence: call the variable *agent scope*, not *context breadth*.** "Context breadth" implies capability was held fixed while visibility varied; it wasn't, and a reviewer will say so. Use *agent scope* in the title, RQs, and pre-registration. The specific interpretive limit: **C3-vs-C1 confounds seeing with doing** — C1 can only advise, C3 acts across three surfaces, so a gap there is most simply explained by "the agent did the clicking," not by context. Treat that contrast as evidence the manipulation works. The contrast that isolates context is **C2-vs-C3** (both act; differ only in GitHub scope), which this N can estimate but not test — report it with its interval. And note that H5 is untouched by any of this, since it compares each condition's trust against that same condition's own measured effectiveness. See `../WELD_ANALYSIS_PLAN.tex` §"The spine of the argument".

**Why hybrid.** The per-person C0 baseline lets you analyze each participant's agent-task performance as a *delta from their own manual speed*, absorbing individual skill differences that would otherwise sit between the between-subjects conditions. This buys back much of the statistical power a pure between-subjects design loses. Six baseline tasks — one per agent task type, with four cross-surface — give a type-matched, stable per-person anchor rather than one generic warm-up.

**Why between-subjects on the agent condition.** Holding access fixed per person avoids within-person contamination (once you've seen the full-context agent, you can't un-see how the task should go) and keeps the session within the self-serve ~55–65-min budget. The cost — comparing across people — is mitigated by baseline normalization.

## Task Structure

**12 tasks total, ~4–5 min each (~55–65 min + survey).** Tasks map to canonical use cases so you can locate *where* agent scope matters.

**Baseline (C0, manual, everyone) — 6 tasks (B1–B6):**
- B1 — single-surface (read #general, find a blocked card, move it to In Progress).
- B2 — cross-surface (chat status request → check a card's column → post a status reply).
- B3 — cross-surface (open a card from a chat link → set its priority).
- B4 — cross-surface (open a card → open its attached PR → review it → post its number and an assessment in chat).
- B5 — cross-surface (recover a decision from chat → record it in a card's description).
- B6 — comprehension (find the CI-pipeline card → confirm what it does in chat).
- One baseline matches each agent task type (B1↔single-surface, B6↔comprehension, B2–B5↔cross-surface) so normalization is apples-to-apples, but content is *disjoint* from the agent tasks to avoid learning transfer.

**Agent tasks (assigned condition) — 6 tasks, spanning 3 use-case types (~2 each):**

| Type | Example | Prediction |
|---|---|---|
| **Comprehension / advice** | "Ask the agent what the team needs done next based on recent discussion, then act on its answer." | All conditions do well; chat-only competitive. |
| **Single-surface action** | "Have the agent help you reorganize the board" (board-only) or "draft a status message" (chat-only). | Extra context adds little. |
| **Cross-surface action** | "Have the agent take the bug from chat, update the board, and surface the linked PR." | Full context pulls ahead; chat-only struggles. |

The cross-surface tasks are where H1/H2 live. The other types are controls that show wider scope *doesn't* universally help — which makes the cross-surface finding credible rather than a blanket "more is better."

**Matched content.** Each task needs equivalent difficulty across conditions; since condition is between-subjects, all participants see the *same* tasks — only their agent's access differs. This is clean: task content is held constant, agent scope is the only manipulation.

## Measures

**Primary (RQ1):**
- **End-to-end user task time** per task (the headline outcome).
- **Baseline-normalized time:** each agent task time relative to that participant's own *type-matched* C0 baseline (comprehension ÷ B6, single-surface ÷ B1, cross-surface ÷ mean B2–B5). The core comparison across conditions.
- **Cognitive load:** per-task effort (1–7, every task) plus NASA-TLX collected **twice** — post-baseline (`tlx_*_baseline`) and at exit (`tlx_*`) — so load is a within-person agent−baseline delta, not a single end-of-session snapshot.
- **Trust in the agent:** validated trust-in-automation items (see survey additions below), plus behavioral proxies (see RQ2).

**Quality control:**
- **Task correctness** (rubric-scored): essential — a fast-but-wrong agent isn't effective. Always pair time with correctness.
- **Agent-internal metrics:** agent response latency, number of agent turns/tool calls, whether the agent acted vs. only advised.

**Behavioral (RQ2 — reliance / division of labor):**
- **Delegation ratio:** share of task actions performed by the agent vs. the user.
- **Verification behavior:** does the user check/repair agent output? (re-reads, edits agent's changes, undoes them) — directly parallels Tang et al.'s validation-and-repair behaviors for LLM-generated code.
- **Acceptance/rejection:** how often users accept the agent's suggestion or action vs. override it.
- **View-switching, cross-reference usage** (carried over from prior telemetry spine).

## Analysis

- **Primary (RQ1):** compare baseline-normalized task times across C1/C2/C3 (mixed-effects model: task time ~ condition × task-type + (1|participant), with C0 as the per-person anchor). Report effect sizes and CIs.
- **H2 test:** the condition × task-type interaction — is the condition effect larger for cross-surface tasks? This is the key result.
- **Trust vs. effectiveness (H5):** correlate trust ratings against normalized time and correctness; look for divergence.
- **RQ2 (reliance):** delegation ratio and accept/verify/override patterns by condition (telemetry) + thematic analysis of open-ended responses.

## Sample Size

Between-subjects on 3 conditions makes N matter more than before. The per-person baseline and 6 tasks each (repeated measures) help. **Planned target: ~20–30 participants total (~7–10 per condition)** — a short *full* study, not a pilot. At this N the full condition×task_type interaction is underpowered, so the primary analysis is a **pre-registered focused contrast** (C3 vs. C1 on cross-surface tasks), with the omnibus interaction reported as supporting evidence and the trust/perception analyses as exploratory; the per-person baseline is carried as a covariate (ANCOVA-on-logs) rather than pre-divided. See `../WELD_ANALYSIS_PLAN.tex` §"Statistical models" and §"Limitations." For reference, ~45 (15/cell) would make the omnibus interaction itself reasonably powered and ~60–75 comfortable for a top-tier venue, but the focused-contrast design is what makes 20–30 a defensible full study rather than a pilot.

## Survey Additions (beyond the existing exit survey)

Add a **trust-in-automation** measure — the existing survey covers NASA-TLX, SUS, and integration items; for RQ3 you need agent trust. Recommended: a short validated scale (e.g., selected items from the Trust in Automation / Trust in Automated Systems literature), 5–6 Likert items such as agent reliability, predictability, confidence in its output, and willingness to rely on it without checking. Plus 1–2 custom items: "The agent had enough context to help me" and "I had to redo or fix what the agent did." Keep total survey additions under ~2 minutes.

## What Carries Over vs. Changes

**Carries over unchanged:** PetTracker seed (`pettracker_seed.json`), privacy model and exclusions, consent form, recruitment copy, the telemetry spine (`../instrument/CLAUDE_STUDY.md` §5/§5A), self-serve solo flow.

**Changes / additions:**
- Task list is replaced by the 12-task structure above (6 manual baseline B1--B6, then 6 agent T1--T6). NOTE: an earlier revision specified 8 tasks (2 C0 + 6 agent); the baseline was widened to six so every agent task type has a matched per-person anchor. The shipped set is in `apps/web/src/lib/study-tasks.ts` and is authoritative.
- Random assignment to C1/C2/C3 at provisioning, recorded as `agent_condition`.
- The agent itself must be built with **context-scoping** — the ability to restrict exactly what it sees/can-do per condition. This is the new critical-path engineering item (see the agent build spec).
- New telemetry: agent events (latency, turns, tool calls, advise-vs-act, accept/reject/override) and the `agent_condition` field on every event.
- Exit survey gains the trust-in-automation block.

## Critical-Path Risks (one month)

1. **The context-scoped agent is the riskiest build.** It must reliably show the agent *only* its condition's context and expose *only* the matching actions. Get this working and pilot-verified before anything else. If scoping leaks (a chat-only agent somehow sees the board), the study is invalid.
2. **Agent determinism / variability.** LLM agents are stochastic; the same task may go differently run-to-run. Decide: fixed model + temperature, logged prompts, and capture the agent's actual context window per task so you can verify scoping after the fact.
3. **Correctness rubric must exist before launch** — you cannot interpret speed without it. It also needs a scoring *procedure* fixed in advance: two coders, blind to condition (score from a final-state export with `agent_condition`, agent transcript, and timing stripped), all tasks double-coded, quadratic-weighted Cohen's κ reported pre-adjudication, disagreements resolved to consensus. Specified in `../WELD_ANALYSIS_PLAN.tex` §Correctness.
4. **IRB amendment.** If you already submitted the human-vs-tools version, this is a protocol change — talk to your IRB about an amendment rather than assuming the old approval covers it.
5. **Model capability is a design parameter, not an ops detail.** The pinned snapshot must be strong enough that C3 can convert its wider scope into performance. A model too weak floors the very condition the study is about and produces a null that says nothing about scope. Verify the snapshot is still served before the run, hold it constant throughout, and name it exactly in the methods.
6. **Task wording is load-bearing.** Agent tasks state a *goal*, never a route, and say the assistant *is available*, never "use the assistant." Naming the surfaces lets participants navigate without the agent — collapsing the manipulation into UI mechanics — and instructing use turns RQ2's delegation measure into a compliance measure. Enforced in `apps/web/src/lib/study-tasks.ts` (see the module docstring).

---

*Design v1.1 — supersedes the human-vs-tools operational plan. Next deliverables: the agent build spec (context-scoping + agent telemetry), the 12 concrete task scripts with matched content, the correctness rubric, and the trust-survey block. Reuses existing seed, consent, recruitment, and privacy docs.*
