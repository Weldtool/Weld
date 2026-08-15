# The instrument — how the study software was built and is operated

The study's measuring device is the Weld deployment itself: the `STUDY_MODE`
flag, the per-participant seed clone, the Context Gate that enforces each
condition, and the telemetry spine that records what happened. These four
documents specify that device and explain how to run it.

They were untracked until 2026-08-10 — which meant the references to them from
`crates/weld-server/src/services/study/` and `apps/web/src/lib/` pointed at files
that did not exist in a clone.

| File | What it is |
|---|---|
| `WELD_STUDY_RUNBOOK.md` | Operating the deploy: env vars, minting access codes, what a participant experiences, pulling the CSV exports, and the pre-run verification checklist. Start here to actually run the study. |
| `CLAUDE_STUDY.md` | Build spec for the non-agent infrastructure: the `STUDY_MODE` flag, per-participant seed provisioning and isolation, the telemetry spine and its privacy exclusions (§5), the GitHub mock and write lockdown. |
| `CLAUDE_AGENT_BUILD.md` | Build spec for the context-scoped agent: the Context Gate, the scoping rules, the double-gated tool executor, agent telemetry, and the pinned-model contract. |
| `WELD_CLAUDE_CODE_HANDOFF.md` | Reading order and build order across all of the above. The entry point if you are picking this up cold. |

## These are specs, not the record

Where the code implements a spec, **the code is authoritative** — these describe
intent, and intent drifts. Two cases where it already has:

- The pinned model is whatever `config.rs` defaults `OPENAI_MODEL` to. Earlier
  drafts of these documents named a different model.
- The telemetry schema is the `ALLOWED_EVENT_TYPES` list in
  `crates/weld-server/src/services/study/telemetry.rs`, enforced at ingest. A
  prose copy of it would only ever be stale.

So if you are reconstructing what the study actually did, read the code. Read
these to understand why it is shaped the way it is.

## Why this sits under `paper/`

An operator needs it and a replicator needs it, and those were briefly two
directories on two branches. One tracked location is worth more than a tidy
split. `../replication/` holds the material a *reader* of the paper needs — the
design of record, task scripts, rubric, survey, consent; this directory holds
what someone *rebuilding or re-running* the study needs.
