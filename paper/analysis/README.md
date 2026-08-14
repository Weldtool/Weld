# Analysis pipeline — Weld agent-scope study

Exports in, tables and figures out. Everything here implements what
`../WELD_ANALYSIS_PLAN.tex` fixed in advance; if you change a rule, you are
deviating from the plan and it must be reported in the paper's Results section.

Everything runs through `make`. `make help` lists every target; `make setup`
builds the venv, and the targets that need it depend on it, so you can go
straight to `make run-synthetic` from a clean checkout.

| Target | Does |
|---|---|
| `make setup` | Create `.venv`, install `requirements.txt`. |
| `make check` | Verify the environment before a real run: deps, R + `ordinal`, which exports are present. |
| `make synthetic` | Regenerate fake exports into `data_synthetic/`. |
| `make run-synthetic` | Regenerate fake data, then analyse it. |
| `make export BASE=… TOKEN=…` | Pull the three real exports into `data/`. |
| `make run` | The full analysis on `data/`. |
| `make packet` | Build the blinded coding packet. |
| `make merge` | Merge the coders' scores into `data/correctness.csv`. |
| `make test` | Regression tests (a few seconds). |
| `make clean` / `distclean` | Remove outputs / also venv and synthetic data. |

`DATA`, `OUT`, `N` and `SEED` are overridable: `make run DATA=data_pilot
OUT=out_pilot`.

## Try it right now, before any data exists

The pipeline is testable today on synthetic data with the exact shape of the real
exports. This is the point: nothing about the analysis should still be unknown on
the day real data lands.

```bash
cd paper/analysis
make run-synthetic
```

You get `out/report.txt`, `out/tables/*.csv`, `out/figures/*.png`, and
`out/correctness_model_R_output.txt`.

> The synthetic numbers are **fake** — arbitrary effects chosen so every code path
> executes. Never let one near the paper.

## Run day, start to finish

Roughly: pull the data, analyse it, get correctness coded, analyse it again.
Only step 4 involves other people, and it is the long pole — start it the day
the last participant finishes.

### 1. Check the environment

```bash
make check
```

Confirms the Python deps import, that `Rscript` and `ordinal` are present (the
correctness model of record needs them), and which export files you already
have. Do this *before* the run, not after.

### 2. Pull the exports

```bash
make export BASE=https://your-deploy TOKEN=$STUDY_EXPORT_TOKEN
```

Writes `data/study_{participants,events,survey}.csv`. The endpoint 404s unless
`STUDY_EXPORT_TOKEN` is set on the deploy — the target fails loudly and tells
you so rather than leaving you with three empty files.

### 3. First analysis pass

```bash
make run
```

Everything except correctness works from here. The report says
`Coded: 0/N agent-phase tasks` and flags the correctness section as empty —
that is expected at this stage, not a failure.

Use this pass to check the manipulation check reads 0 violations and the
participant counts match your records, before anyone spends time coding.

### 4. Correctness coding (the human step)

```bash
make packet          # builds coding/ and prints the blinding check
```

Send each coder `coding/CODING_PACKET.md` and **one** of
`coding/coder_1.csv` / `coding/coder_2.csv`. They score every row 0–3
independently, without conferring.

**Do not send anyone `study_events.csv`.** Blinding is mandatory and that file
carries `agent_condition` on every row, plus timestamps and the agent's whole
transcript — it unblinds a coder on line one. `make packet` verifies the packet
itself and **fails** if it finds a leak, printing the offending lines; a clean
build says so explicitly.

When both come back:

```bash
make merge           # -> data/correctness.csv
```

The merge leaves `score` blank wherever the two disagree and prints those rows.
The coders settle each one by discussion, still blind, and you fill in the blank
cells. **The file is done only when no `score` cell is empty** — blank rows are
dropped, which shrinks the model. Keep the `coder_1`/`coder_2` columns after
adjudicating: κ is computed from them, and the plan reports the
*pre*-adjudication figure.

### 5. Final analysis pass

```bash
make run
```

Now the report shows `Coded: N/N` and the correctness section is populated. If
it says anything less than 100%, adjudication is unfinished and the numbers are
fitted on a subset — the report shouts about this rather than letting it pass
quietly.

### 6. Read the output

- `out/report.txt` — everything, in the order the Results section presents it.
- `out/tables/*.csv` — one file per table, for pasting into the paper.
- `out/figures/*.png` — Figures 2–5.
- `out/correctness_model_R_output.txt` — the CLMM model of record, including
  the script's own `STATUS:` line saying whether the primary model held or the
  pre-specified fallback became the result.

Then work through "Sanity checks that should hold" at the bottom of this file.

## Tests

```bash
make test
```

`test_pipeline.py` encodes the invariants that used to be checked by eye:
scoping never leaks, C1 delegates nothing and cannot produce tool-derived
funnel events, the idle rule actually fires, every participant has 12 tasks
unless they withdrew, the reverse-scored TLX item stays reversed, κ is
undefined rather than 1.0 when coders never vary.

They assert **structure, not effect sizes**. Synthetic effects are arbitrary and
reseeding changes them; a test pinning a p-value would fail for no reason.

## What is in the coding packet

`correctness.csv` is the one input the server cannot produce — it comes from two
human coders. The workflow is step 4 above; this is what the packet actually
contains and why.

The packet is **final state only**: board (cards, columns, priorities, links)
and chat, with `[[card:…]]` refs resolved to titles so a coder can tell which
card a teammate meant, the participant's own messages marked, and any in-app PR
review verdict and comment. Condition, timing and the agent transcript are
absent. Items are shuffled with a fixed seed so nobody works through one
participant's session in order and carries an impression between tasks. Each
item shows the rubric anchor for its task, parsed live from
`../replication/WELD_AGENT_TASK_SCRIPTS.md` §2 rather than copied, so the packet
cannot drift from the rubric.

The blinding check runs automatically as part of building the packet, and
**exits non-zero if it finds anything**, so a leaking packet cannot be built
quietly and then sent. It scans the generated items only — the preamble is our
own fixed instructions, which legitimately mention conditions in the course of
telling the coder they are not being shown one.

The pattern lives in `coding_packet.LEAK_PATTERN`, as the single source of
truth. It is deliberately broad: a false positive costs one look, an unblinded
coder costs the correctness measure.

The file the coders produce:

| column | meaning |
|---|---|
| `participant_code` | e.g. `P0007` |
| `task_id` | `B1`…`B6`, `T1`…`T6` |
| `score` | consensus score, 0–3 (blank until adjudicated) |
| `coder_1`, `coder_2` | pre-adjudication scores, for quadratic-weighted κ |

κ is computed from `coder_1`/`coder_2`, so keep both columns after adjudicating —
the plan reports the *pre*-adjudication figure.

If the file is absent the pipeline still runs and correctness outputs are empty.
Rows with a blank `score` are dropped; `run_all.py` reports the coded fraction
and flags anything under 100%, so a half-finished adjudication cannot quietly
shrink the model.

## Files

| File | Does |
|---|---|
| `load.py` | Reads the three CSVs, parses the JSON payload column. The only place that knows the server's export shape. |
| `derive.py` | Task-level and participant-level tables: baseline normalisation, idle rule, TLX/SUS/trust scales, delegation. |
| `analyze.py` | Manipulation check, primary timing model, focused contrasts, correctness, TLX deltas, agent funnel, RQ3 correlations. |
| `figures.py` | Figures 2–5. |
| `synthesize.py` | Fake exports for testing the pipeline. |
| `coding_packet.py` | Builds the blinded correctness-coding packet; merges the coders' scores back. |
| `run_all.py` | Orchestrates everything, fits the R model of record, writes the report. |
| `sensitivity.py` | Minimum-detectable-effect calculation quoted in §5.3. Importable; `run_all.py` folds its headline into the report. |
| `test_pipeline.py` | Regression tests for the invariants above (`make test`). |
| `Makefile` | Every workflow step as a target (`make help`). |

## Decisions worth knowing about

**Baseline anchors.** Comprehension → B6; single-surface → B1; cross-surface →
mean of B2–B5. The baseline enters models as a covariate, not as a pre-divided
ratio; `normalised_time` exists for plotting and per-participant summaries.

**Idle rule.** ≥180 s between consecutive events inside a task counts as idle.
Because we have only the away *total* and not a full visibility timeline, the
adjustment subtracts only the excess over recorded away time — deliberately
conservative. The primary model is fitted both ways.

**Correctness model.** The plan specifies a cumulative-link *mixed* model.
statsmodels has no CLMM and R is not a dependency here, so `analyze.py` reports
per-cell descriptives plus Kruskal–Wallis, flags itself as approximate, and
writes `out/correctness_model.R`.

**`run_all.py` runs it for you** when `Rscript` is on PATH, writes the output to
`out/correctness_model_R_output.txt`, and echoes the script's own STATUS line
into the report — so the reported model is the CLMM, not the approximation,
without anyone having to remember a second command. If R is missing the report
says so loudly and the rest of the pipeline still completes; `--no-r` skips it
deliberately. To run it by hand:

```bash
Rscript out/correctness_model.R      # needs `ordinal`; `lme4` optional
```

It runs from any working directory — the data path is resolved when the file is
written. The script is not just the `clmm` call: the plan *expects* the mixed
model to struggle at 7–10 per cell with scores piling up at 3, and pre-specifies
what to do instead, so the script decides for itself and says which result is the
one of record. It fits the CLMM, rejects the fit if the standard errors come back
missing or absurd (a returned object is not a usable fit), tests proportional
odds, then computes the fallback — fully-correct vs not, per-cell Wilson
intervals, a mixed logistic if `lme4` is installed, and a permutation test that
shuffles condition **across participants**, since condition is between-subjects
and shuffling tasks would break the dependence the random effect models.

The fallback is computed on every run: supplementary when the primary model
holds, the result when it doesn't. Deciding which after seeing the numbers is how
a pre-specified fallback turns into a post-hoc choice.

**Trust composite** is three items — `trust_reliable`, `trust_predictable`,
`trust_competent`. The other `trust_*` items are reported individually and are
deliberately excluded; `trust_context` especially, since it is a perception of
the manipulation and would build the IV into the outcome. Cronbach's α is
reported, and the report shouts if it falls below the pre-committed 0.70.

**TLX** is raw (unweighted) 0–100. `tlx_performance` is the one item authored
against the workload direction — the app anchors it Failure(low)→Perfect(high),
so high means *good* performance — and `derive.py` reverse-scores it
(`MAX + MIN − raw`, the sliders being 1–100) before it enters the composite or
any subscale delta. This is what `../WELD_ANALYSIS_PLAN.tex` §"Cognitive load (H3)"
pre-specifies. Verify the anchors in `survey-config.ts` before touching it: the
direction being wrong corrupts both phases, and therefore the delta too,
silently and in a way none of the sanity checks below would catch.

**Delegation** counts only agent-*completed* actions in the numerator. An
executed `post_message` tool call drafts a message; the participant reviews and
sends it, and that send is already counted as a user action, so counting the
draft as well would put one outcome on both sides of the ratio. Drafts are
carried separately as `agent_chat_drafts` / `chat_drafts` so the drafting
channel of delegation stays visible.

**The manipulation check is structural, not textual.** It decides which context
sources a snapshot carried from the gate's own `sources_included` field unioned
with the top-level keys actually attached — never by searching the payload for
the words "board" or "github". The snapshot embeds real chat message text, and
the seeded teammates discuss the board and GitHub in it; a substring scan reads
that ordinary English as a leak and reports 100% violations in C1 and C2. It did
exactly that on the first real export, against a gate that was working
correctly. `synthesize.py` now puts the same kind of prose in its fake
snapshots, so any return to a text-based check fails on synthetic data first.

**Inter-coder agreement** is quadratic-weighted Cohen's κ over the fixed 0–3
rubric scale (not over the categories that happen to appear), computed on
`coder_1`/`coder_2` *before* adjudication — after adjudication the coders agree
by construction. κ is undefined, not 1.0, when both coders used a single
category throughout; the report says so rather than printing a perfect score.

**Figures** use Okabe-Ito colours validated for colour-vision deficiency (worst
adjacent pair ΔE 11.4 protan). Marker shape duplicates colour so the figures
survive greyscale printing. Every participant point is drawn — at 7–10 per cell a
bar of means hides what a reader needs to judge.

## Sanity checks that should hold

- Manipulation check: C1 shows 0% board and 0% GitHub; C2 shows 0% GitHub;
  `violations` is 0 everywhere. Anything else is a leak — report it, never drop it.
- C1 delegation ratio is exactly 0.00 and `chat_drafts` is 0 (C1 has no tools
  at all by construction).
- Every participant has 12 completed tasks; fewer means skips or abandonment,
  which belong in the participant-flow figure.
