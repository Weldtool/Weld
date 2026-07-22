# Study analysis figures

Generates every figure and table in `research-paper/WELD_ANALYSIS_PLAN.tex` from
the study export, so you don't hand-build plots. Standalone — not wired into the app.

## Setup

```bash
cd scripts/study-analysis
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
```

## Run

Two ways to feed it data (both use the CSVs the backend already exports):

```bash
# A) from downloaded CSVs — participants.csv, events.csv, survey.csv in a dir
python analyze.py --data ./exports

# B) fetch straight from a running server
python analyze.py --url https://your-host --token "$STUDY_EXPORT_TOKEN"
```

Get the CSVs for mode (A) from the export endpoint (needs `STUDY_EXPORT_TOKEN`
set on the server):

```bash
for d in participants events survey; do
  curl -s "https://your-host/api/v1/study/export?dataset=$d&token=$TOKEN" > exports/$d.csv
done
```

## Outputs (in `figures/`)

Each figure is written as vector **PDF** (drop into the `\includegraphics`
placeholders in the plan) plus a **PNG** preview.

| File | Plan | What |
|---|---|---|
| `fig1_time` | Fig. 1 | Normalized time by condition × task type (headline, H2) |
| `fig2_correctness` | Fig. 2 | Correctness by condition × task type — **needs hand-coding, see below** |
| `fig3_perception` | Fig. 3 | Perceived vs. measured speedup, per participant (H5) |
| `fig4_delegation` | Fig. 4 | Executed agent actions per participant, by condition (RQ2/H4) |
| `fig_h3_tlx` | H3 | TLX delta (agent − baseline) by condition |
| `table_manipulation_check.csv` | Fig. 5 | Scoping held: %% C1 turns with no board/GitHub, %% C2 no GitHub |
| `table_consort_flow.csv` | §flow | Started / finished / dropped counts |
| `stats.txt` | — | Group means±SD, H5 Spearman |

Small-N by design: every condition summary overlays individual participant dots
on the mean, never bare bars.

## Fig. 2 correctness (the one manual step)

Correctness isn't in telemetry — it's hand-scored **0–3** from each task's final
state (`0` none · `1` wrong · `2` partial · `3` fully correct), by two independent
coders with Cohen's kappa on a 20% sample (per `WELD_AGENT_TASK_SCRIPTS.md`).

Every run produces two files to make this fast and objective:

- **`correctness_template.csv`** — a `participant × agent-task` grid with the
  task's success anchor and a blank `score` column. Fill `score` (0–3), re-run,
  and Fig. 2 appears. Never overwritten once created.
- **`coding_sheet.md`** — for each participant × task: the anchor ("what a 3
  requires") plus the **final-state evidence** pulled from the `task_final_state`
  snapshot — the resulting board (card columns, priorities, PR links) and the
  messages the participant posted. Coders score straight from this; no need to
  open the live app. Regenerated each run (derived, not hand-edited).

## Notes

- **NASA-TLX Performance is reverse-scored** (`101 − raw`) before compositing,
  because that item is anchored Failure→Perfect (high = good), opposite the other
  five. This matches `apps/web/src/lib/survey-config.ts` and `WELD_EXIT_SURVEY.md`.
- Any figure whose inputs aren't present yet is skipped with a one-line note
  rather than crashing, so it's safe to run mid-collection.
- `python analyze.py --selftest` checks the normalization and TLX-composite math.
