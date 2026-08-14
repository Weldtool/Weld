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

`correctness.csv` comes from two human coders, not the server. `coding_packet.py`
builds what they work from and merges what they hand back.

**Do not send anyone `study_events.csv`.** Blinding is mandatory, and that file
carries `agent_condition` on every row, plus timestamps and the agent's whole
transcript — it unblinds a coder on line one.

```bash
# 1. Build the blinded packet (after the run, once all data is in)
python coding_packet.py --data data --out coding

# 2. Send each coder CODING_PACKET.md and ONE of coder_1.csv / coder_2.csv.
#    They score every row 0–3, independently, without conferring.

# 3. Merge what comes back
python coding_packet.py --merge coding/coder_1.csv coding/coder_2.csv \
    --out data/correctness.csv

# 4. Adjudicate: the merge leaves `score` blank wherever the two disagree and
#    prints those rows. The coders settle each one by discussion, still blind,
#    and you fill the blank cells in. The file is done when none are empty.

# 5. run_all.py picks it up automatically
python run_all.py --data data --out out
```

The packet contains final state only — board (cards, columns, priorities, links)
and chat, with `[[card:…]]` refs resolved to titles so a coder can tell which
card a teammate meant, the participant's own messages marked, and any in-app PR
review verdict and comment. Condition, timing and agent transcript are absent.
Items are shuffled with a fixed seed so nobody works through one participant's
session in order and carries an impression between tasks. Each item shows the
rubric anchor for its task, parsed live from
`../replication/WELD_AGENT_TASK_SCRIPTS.md` §2 rather than copied, so the packet
can't drift from the rubric.

Sanity-check before sending:
```bash
grep -icE 'agent_condition|latency|prompt_version|\bC[123]\b' coding/CODING_PACKET.md   # want 0
```

Resulting file:

| column | meaning |
|---|---|
| `participant_code` | e.g. `P0007` |
| `task_id` | `B1`…`B6`, `T1`…`T6` |
| `score` | consensus score, 0–3 (blank until adjudicated) |
| `coder_1`, `coder_2` | pre-adjudication scores, for quadratic-weighted κ |

κ is computed from `coder_1`/`coder_2`, so keep both columns after adjudicating —
the plan reports the *pre*-adjudication figure.

If the file is absent the pipeline still runs; correctness outputs are empty.
Rows with a blank `score` are dropped, so an unfinished adjudication shrinks the
correctness model silently.

## Files

| File | Does |
|---|---|
| `load.py` | Reads the three CSVs, parses the JSON payload column. The only place that knows the server's export shape. |
| `derive.py` | Task-level and participant-level tables: baseline normalisation, idle rule, TLX/SUS/trust scales, delegation. |
| `analyze.py` | Manipulation check, primary timing model, focused contrasts, correctness, TLX deltas, agent funnel, RQ3 correlations. |
| `figures.py` | Figures 2–5. |
| `synthesize.py` | Fake exports for testing the pipeline. |
| `coding_packet.py` | Builds the blinded correctness-coding packet; merges the coders' scores back. |
| `run_all.py` | Orchestrates everything and writes the report. |
| `sensitivity.py` | Minimum-detectable-effect calculation quoted in §5.3. |

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
