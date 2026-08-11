# `paper/` — the study's tracked material

Everything here is version controlled, so this directory is the source of truth.
If something exists both here and anywhere untracked, this copy wins.

## The submission

| File | What it is |
|---|---|
| `WELD_TOSEM_PAPER.tex` | The manuscript. Target: ACM TOSEM special section on Human-AI Collaboration in SE, deadline 2026-09-01. Sections 3--5 (design, measures, analysis plan) are near-final; the rest are structured skeletons with `\NUM{}` placeholders for every number that has to come from data. |
| `WELD_ANALYSIS_PLAN.tex` | The analysis plan, fixed before data collection. Not deposited in a public registry — this file, with its git history, is the record that what was decided in advance can be checked against what gets reported. |
| `weld-tosem.bib` | The live bibliography. Verified 2026-08-06 against publishers of record. |

```bash
latexmk -pdf -outdir=build WELD_TOSEM_PAPER.tex
latexmk -pdf -outdir=build WELD_ANALYSIS_PLAN.tex
```

Before submitting, grep the manuscript for `\todo` and `\NUM` — none may survive.

## Supporting directories

| Directory | What it is |
|---|---|
| `analysis/` | The analysis pipeline, plus `sensitivity.py` (the minimum-detectable-effect calculation quoted in the paper). Runs today on synthetic data with the exact shape of the real exports, so nothing about the analysis is still unknown when data lands. See its README. |
| `replication/` | The apparatus a reader needs: design of record, all 12 task scripts, correctness rubric, survey instrument, consent text, participant-facing copy, seed design. The paper's Data Availability statement promises this set by name. |
| `instrument/` | How the study software was built and is operated: the operator runbook, and the build specs for the study infrastructure and the context-scoped agent. Comments in `crates/weld-server/src/services/study/` and `apps/web/src/lib/` point here. |
| `archive/` | One superseded manuscript draft (`main.tex` + `sections/`), retired for the reasons in its README. Nothing current lives here. |

## Earlier-framing documents

These describe the study as it was **before** the redesign from *integrated
workspace vs. separate tools* to *agent scope*. They are kept at the top level
because the IRB submission (`IRB-2026-111`, June 2026) was made against this
design, so reconstructing what changed depends on them — but do not read them as
the current protocol.

| File | What it is |
|---|---|
| `WELD_STUDY_PROTOCOL.pdf` | Protocol draft v0.1 — the original workspace-vs-multi-tool study. |
| `WELD_STUDY_TASKS_AND_DATA_v3.pdf` | Task list and data plan v0.3: 35--45 minutes, no agent conditions. |
| `WELD_STUDY_REFERENCE.md` | Working reference v1.0 written for the research team: design, full data inventory with rationale, task set, and analysis plan in one document. Written against the current agent-scope framing, but the two `.tex` documents above supersede it wherever they disagree. Kept as Markdown only, so it renders in the browser and cannot drift from a PDF of itself. |

The current design of record is `replication/WELD_AGENT_STUDY_DESIGN.md`.

## What is deliberately not here

- **The seed fixture** — `crates/weld-server/study/pettracker_seed.json`, which is
  `include_str!`-embedded into the server and therefore authoritative. There is no
  second copy to drift from it.
- **The telemetry schema** — the `ALLOWED_EVENT_TYPES` list in
  `crates/weld-server/src/services/study/telemetry.rs`, enforced at ingest. The
  code is the schema.
- **IRB records** are the only thing left in the untracked `research-paper/`
  directory; they carry team contact details. See
  `research-paper/README_WHERE_THINGS_LIVE.md`.

## A branch caveat worth knowing

`paper/` exists only on `paper/tosm-journal-writing`. The study code lives on the
`Production` line, and its comments cite `paper/instrument/…`, so those citations
resolve here and not there. Merging this branch fixes that; until then, read
these documents from this branch.
