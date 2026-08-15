# Weld Agent Study — Claude Code Handoff Package

This is the entry point for building the study. Read the files in the order below, then build in the order in §3. The study compares three AI-agent **context conditions** (C1 chat-only, C2 chat+board, C3 chat+board+GitHub) against each participant's own manual baseline, to measure whether broader agent context makes the agent more effective and the user faster.

---

## 1. Read these first (in order)

1. **`../replication/WELD_AGENT_STUDY_DESIGN.md`** — the research design (RQs, conditions, hybrid baseline, hypotheses, measures). The "why." Read this first so the build decisions make sense.
2. **`CLAUDE_STUDY.md`** — the base study build: `STUDY_MODE` flag, per-participant seed provisioning & isolation, telemetry spine, privacy exclusions, GitHub mocked/locked-down, consent/survey/flow. **Most of the non-agent infrastructure is specified here and still applies.**
3. **`CLAUDE_AGENT_BUILD.md`** — the agent build: the Context Gate, context scoping (the critical-path, must-be-provable part), tools, agent telemetry, the OpenAI API integration (pinned model snapshot, cost controls, reproducibility), Railway deployment.
4. **`../replication/WELD_AGENT_TASK_SCRIPTS.md`** — the 12 concrete tasks (6 baseline + 6 agent), the correctness rubric, the trust-survey block, and the seed deltas to add.
5. **`crates/weld-server/study/pettracker_seed.json`** + **`../replication/pettracker_seed_README.md`** — the seed fixture (the only copy, `include_str!`-embedded) and its design/task-mapping. Load at provisioning.
6. **`../replication/WELD_EXIT_SURVEY.md`** — survey instrument; append the trust block from the task-scripts file.
7. **`../replication/WELD_PARTICIPANT_FLOW.md`** — participant-facing copy (landing, consent, screening, end screens).

> Supporting/optional (not needed to build the core): `../replication/WELD_CONSENT_FORM.md` (compliance, not engineering) and the pre-redesign `../WELD_STUDY_PROTOCOL.pdf` / `../WELD_STUDY_TASKS_AND_DATA_v3.pdf` (superseded by the agent design — historical context only, but the IRB submission was made against them).
>
> Earlier drafts of this list also named `WELD_BOT_SPEC.md`, `WELD_IRB_APPLICATION.md`, and `WELD_INTERVIEW_SCRIPT.md`. **None of those documents were ever written**, and the study as built needs none of them: there is no scripted bot, the IRB record is `research-paper/IRB-2026-111_approval.pdf`, and there is no interview component. They are listed here only so a reader who goes looking stops looking.

## 2. The non-negotiables (do not compromise these)

1. **Context scoping must be provable.** A C1 (chat-only) agent must NEVER receive board or GitHub data; C2 must never receive GitHub. Enforce at the context-assembly layer (the Context Gate), not via prompt instructions. Log `agent_context_sent` so scoping can be verified from logs after the fact. A silent leak invalidates the whole study. (`CLAUDE_AGENT_BUILD.md` §0–§3, §9.)
2. **Per-participant seed isolation.** Every participant gets an identical, isolated PetTracker copy; one participant's actions never affect another's. (`CLAUDE_STUDY.md` §2.2.)
3. **No live GitHub.** GitHub is mocked/seeded inside Weld; write controls hidden under `STUDY_MODE`; participants need no GitHub account or repo. C3's GitHub context is the *seeded fixtures*. (`CLAUDE_STUDY.md` §4.)
4. **Model pinned & reproducible.** OpenAI `gpt-5.4-2026-03-05` (the `config.rs` default and the string the paper names; earlier drafts said `gpt-4o-mini`), fixed low temperature, versioned prompt, full request/response logged. Hold constant for the whole study. (`CLAUDE_AGENT_BUILD.md` §6.)
5. **Privacy exclusions.** No keystrokes, mouse, screen/camera, other-tab data, or non-PetTracker data. (`CLAUDE_STUDY.md` §5.)
6. **Every task reconstructable from logs** for correctness + who-did-it + timing. (`../replication/WELD_AGENT_TASK_SCRIPTS.md` §5.)

## 3. Build order

1. `STUDY_MODE` flag scaffolding (`CLAUDE_STUDY.md` §2.1).
2. Participant provisioning + seed isolation, loading `pettracker_seed.json` (`CLAUDE_STUDY.md` §2.2, §6) + **add Seed Deltas D1–D3** (`../replication/WELD_AGENT_TASK_SCRIPTS.md` §4).
3. Random assignment to C0-then-{C1|C2|C3}; persist `agent_condition` per participant.
4. GitHub mocked view + write lockdown (`CLAUDE_STUDY.md` §4) — verify the view renders from seed, not live API.
5. Telemetry spine + cross-reference events (`CLAUDE_STUDY.md` §5/§5A).
6. **Context Gate + scoping rules** (`CLAUDE_AGENT_BUILD.md` §2–§3) — build and unit-test FIRST among agent pieces.
7. **Agent telemetry, esp. `agent_context_sent`** (`CLAUDE_AGENT_BUILD.md` §5).
8. OpenAI call wiring, C1 (advise-only) first (`CLAUDE_AGENT_BUILD.md` §6).
9. Tool executor + `board_write`, `chat_post` (C2), double-gated.
10. `github_link` (C3), reusing GitHub lockdown.
11. Agent UI (`CLAUDE_AGENT_BUILD.md` §7).
12. In-app task panel driving the 12 tasks (`CLAUDE_STUDY.md` §2.3 + `../replication/WELD_AGENT_TASK_SCRIPTS.md`); agent hidden during C0.
13. Consent / screening / exit-survey (incl. trust block) / done flow.
14. **Scoping verification pass + end-to-end pilot** before any real participant (`CLAUDE_AGENT_BUILD.md` §9).
15. (Optional) bot — only if all the above is solid.

## 4. Secrets & deploy
- `OPENAI_API_KEY` as a Railway secret; LLM calls server-side only.
- Set an OpenAI dashboard usage alert; log per-call token usage; enable prompt caching (stable content first).

## 5. Definition of done
All acceptance checks pass in `CLAUDE_AGENT_BUILD.md` §9 and `CLAUDE_STUDY.md` §11, plus: a fresh participant link runs C0 → assigned agent condition → survey → done with no researcher intervention, all 12 tasks reconstructable from logs, and scoping verified from `agent_context_sent` for C1 and C2.
