# Replication package

The study apparatus, in the form a reader of the paper would need it. The paper's
Data Availability statement promises exactly this set — task scripts, telemetry
schema, correctness rubric, and survey instrument — so it has to be tracked, and
until 2026-08-10 none of it was: every file here lived only in the gitignored
`research-paper/` directory on one laptop.

| File | What the paper uses it for |
|---|---|
| `WELD_AGENT_STUDY_DESIGN.md` | The design of record — RQs, hypotheses, conditions, measures. Section 3 of the paper is a condensation of it. |
| `WELD_AGENT_TASK_SCRIPTS.md` | All 12 task scripts verbatim, the 0--3 correctness rubric with per-task anchors, and the trust survey block. Section 3.4 and the correctness procedure in Section 4 both cite it. |
| `WELD_EXIT_SURVEY.md` | The exit instrument: NASA-TLX, SUS, integration items, trust block, demographics. Item ids here are the `item_id` values in the survey export. |
| `WELD_CONSENT_FORM.md` | Consent text of record. See the version note at the bottom of that file before running anyone. |
| `WELD_PARTICIPANT_FLOW.md` | Participant-facing copy: recruitment, landing, screening, end screens. |
| `pettracker_seed_README.md` | Seed design and its task mapping — which card, message, and PR each task depends on. |

## What is not here, and where it is

- **The seed fixture itself** is `crates/weld-server/study/pettracker_seed.json`.
  It is `include_str!`-embedded into the server, so that copy is authoritative and
  there is deliberately no second copy to drift from it. (There was one, in
  `research-paper/`; it was byte-identical and has been deleted.)
- **The telemetry schema** is the `ALLOWED_EVENT_TYPES` list in
  `crates/weld-server/src/services/study/telemetry.rs`. It is enforced at ingest,
  so the code is the schema; a prose copy would only ever be stale.
- **The analysis code** is `../analysis/`.
- **Operator material** — the runbook and the `CLAUDE_*` build specs — is next
  door in `../instrument/`. It describes how the software that took these
  measurements was built and how the deploy is run: needed to *re-run* the study,
  not to read it.

## Before archiving this for submission

Strip nothing, but check two things: `WELD_PARTICIPANT_FLOW.md` carries
recruitment copy with contact details, and `WELD_CONSENT_FORM.md` names the
research team. Both are fine to publish; neither should be edited to match the
paper if they disagree with it — a disagreement means the paper is wrong about
what participants were shown.
