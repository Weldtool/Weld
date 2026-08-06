# Analysis pipeline — Weld agent-scope study

Exports in, tables and figures out. Everything here implements what
`../WELD_ANALYSIS_PLAN.tex` fixed in advance; if you change a rule, you are
deviating from the plan and it must be reported in the paper's Results section.

## Setup

```bash
cd paper/analysis
python3 -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt
```

## Run it right now, before any data exists

The pipeline is testable today on synthetic data with the exact shape of the real
exports. This is the point: nothing about the analysis should still be unknown on
the day real data lands.

```bash
python synthesize.py --out data_synthetic --n 24
python run_all.py --data data_synthetic --out out
```

You get `out/report.txt`, `out/tables/*.csv`, and `out/figures/*.png`.

> The synthetic numbers are **fake** — arbitrary effects chosen so every code path
> executes. Never let one near the paper.

## Run it on real data

Pull the three exports (needs `STUDY_EXPORT_TOKEN` set on the deploy; the
endpoint is 404 otherwise):

```bash
BASE=https://your-deploy; TOKEN=…
for ds in participants events survey; do
  curl -sH "Authorization: Bearer $TOKEN" \
    "$BASE/api/v1/study/export?dataset=$ds" -o data/study_$ds.csv
done
```

Then add `data/correctness.csv` from the coders (below) and:

```bash
python run_all.py --data data --out out
```

## The one input that is not automatic

`correctness.csv` comes from the two human coders, not the server:

| column | meaning |
|---|---|
| `participant_code` | e.g. `P0007` |
| `task_id` | `B1`…`B6`, `T1`…`T6` |
| `score` | consensus score, 0–3 |
| `coder_1`, `coder_2` | pre-adjudication scores, for quadratic-weighted κ |

The rubric and anchors are in `research-paper/WELD_AGENT_TASK_SCRIPTS.md` §2.
Coders work from the `task_final_state` snapshot with condition, agent
transcript, and timing stripped — blinding is mandatory.

If the file is absent the pipeline still runs; correctness outputs are empty.

## Files

| File | Does |
|---|---|
| `load.py` | Reads the three CSVs, parses the JSON payload column. The only place that knows the server's export shape. |
| `derive.py` | Task-level and participant-level tables: baseline normalisation, idle rule, TLX/SUS/trust scales, delegation. |
| `analyze.py` | Manipulation check, primary timing model, focused contrasts, correctness, TLX deltas, agent funnel, RQ3 correlations. |
| `figures.py` | Figures 2–5. |
| `synthesize.py` | Fake exports for testing the pipeline. |
| `run_all.py` | Orchestrates everything and writes the report. |
| `../sensitivity.py` | Minimum-detectable-effect calculation quoted in §5.3. |

## Decisions worth knowing about

**Baseline anchors.** Comprehension → B6; single-surface → B1; cross-surface →
mean of B2–B5. The baseline enters models as a covariate, not as a pre-divided
ratio; `normalised_time` exists for plotting and per-participant summaries.

**Idle rule.** ≥180 s between consecutive events inside a task counts as idle.
Because we have only the away *total* and not a full visibility timeline, the
adjustment subtracts only the excess over recorded away time — deliberately
conservative. The primary model is fitted both ways.

**Correctness model.** The plan specifies a cumulative-link *mixed* model.
statsmodels has no CLMM and R is not a dependency here, so `analyze.py` fits an
ordinal model without the random effect plus Kruskal–Wallis, flags itself as
approximate, and writes `out/correctness_model.R` with the exact `ordinal::clmm`
call. **Run that in R for the model of record.**

**Trust composite** is three items — `trust_reliable`, `trust_predictable`,
`trust_competent`. The other `trust_*` items are reported individually and are
deliberately excluded; `trust_context` especially, since it is a perception of
the manipulation and would build the IV into the outcome. Cronbach's α is
reported, and the report shouts if it falls below the pre-committed 0.70.

**TLX** is raw (unweighted) 0–100. `tlx_performance` is authored anchored
Perfect→Failure, i.e. already in the workload direction, so nothing is
reverse-scored. Verify that anchoring before changing it.

**Figures** use Okabe-Ito colours validated for colour-vision deficiency (worst
adjacent pair ΔE 11.4 protan). Marker shape duplicates colour so the figures
survive greyscale printing. Every participant point is drawn — at 7–10 per cell a
bar of means hides what a reader needs to judge.

## Sanity checks that should hold

- Manipulation check: C1 shows 0% board and 0% GitHub; C2 shows 0% GitHub;
  `violations` is 0 everywhere. Anything else is a leak — report it, never drop it.
- C1 delegation ratio is exactly 0.00 (C1 has no tools by construction).
- Every participant has 12 completed tasks; fewer means skips or abandonment,
  which belong in the participant-flow figure.
