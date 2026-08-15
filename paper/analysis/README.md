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
| `make export BASE=… TOKEN=…` | Pull the four real exports into `data/`. |
| `make run` | The full analysis on `data/`. |
| `make packet` | Build the blinded correctness coding packet (`PHASE=agent\|baseline\|all`, `CAP=10`). |
| `make merge` | Merge the coders' scores into `data/correctness.csv` (same `PHASE`). |
| `make packet-agent-errors` | Build the agent-error coding packet (H4). Not blind. |
| `make merge-agent-errors` | Merge them into `data/agent_errors.csv`. |
| `make packet-themes` | Build the qualitative (open-ended) coding packet. |
| `make merge-themes` | Merge them into `data/themes.csv`. |
| `make test` | Regression tests (a few seconds). |
| `make clean` / `distclean` | Remove outputs / also venv and synthetic data. |

Three human codings feed this pipeline, not one. Correctness is the load-bearing
one and is blinded; the other two are new and are described under
"The three codings" below.

`DATA`, `OUT`, `N`, `SEED`, `PHASE`, `CAP` and `RELFRAC` are overridable:
`make run DATA=data_pilot OUT=out_pilot`.

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

Writes `data/study_{participants,events,survey,agent_turns}.csv`. The endpoint
404s unless `STUDY_EXPORT_TOKEN` is set on the deploy — the target fails loudly
and tells you so rather than leaving you with empty files.

`agent_turns` is the agent's own conversation. It is what the H4 calibration test
is coded from, and until recently the server wrote it but never exported it, so
the transcripts would have stayed in the database with no way to reach them.

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

Defaults: the **agent-phase tasks** (`PHASE=agent`) for the **first 10
participants per condition** in enrolment order (`CAP=10`). Both are
pre-registered decisions, not conveniences — see "Why the coded set is capped"
below. Override with `make packet PHASE=all CAP=0` to code everything.

Send each coder `coding/CODING_PACKET_agent.md` and **one** of
`coding/coder_1_agent.csv` / `coding/coder_2_agent.csv`. They score every row
0–3 independently, without conferring.

**Do not send anyone `study_events.csv`.** Blinding is mandatory and that file
carries `agent_condition` on every row, plus timestamps and the agent's whole
transcript — it unblinds a coder on line one. `make packet` verifies the packet
itself and **fails** if it finds a leak, printing the offending lines; a clean
build says so explicitly.

**Do not send anyone `coding/RESEARCHER_ONLY_CODED_SET.csv` either.** It is the
cap's audit trail — who was selected, who wasn't, and why — and it names each
participant's condition. It is the one file in `coding/` that would unblind the
whole round, which is why it is named the way it is.

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

### 4b. The other two codings

Correctness is not the only human input any more. Start these once correctness
coding is under way — they are independent of it, but correctness goes first
(see below for why).

```bash
make packet-agent-errors    # -> coding/AGENT_ERROR_PACKET.md + ANSWER_KEY.md + two CSVs
make merge-agent-errors     # -> data/agent_errors.csv

make packet-themes          # -> coding/QUALITATIVE_PACKET.md + THEME_CODEBOOK.md
make merge-themes           # -> data/themes.csv
```

Send the agent-error coders **both** `AGENT_ERROR_PACKET.md` and
`ANSWER_KEY.md` — the key is what makes `factual_error` answerable.

All three merges follow the same rule: agreements become the consensus,
disagreements are held back for resolution, and the two coder columns are kept so
κ is computed pre-adjudication. See "Adjudication" below for how to resolve them.

For themes, **revise `THEME_CODEBOOK.md` before coding starts**. One coder reads
every response, adjusts the codebook, and then it is frozen — changing it
mid-coding makes κ meaningless. One primary theme per response.

Themes are the **one coding that is not fully double-coded**, so they ship as two
different packets:

| Coder | Send them | They code |
|---|---|---|
| 1 | `QUALITATIVE_PACKET.md` + `theme_coder_1.csv` | every response |
| 2 | `QUALITATIVE_PACKET_reliability.md` + `theme_coder_2.csv` | a stratified 25% (`RELFRAC`) |

κ is computed on the overlap only, and `theme_agreement.csv` reports `n_items`
(the double-coded sample) beside `n_coded` (the corpus) so it can't be misread as
agreement over everything. Rows only coder 1 saw take coder 1's theme as the
consensus — there is no disagreement to adjudicate — and the merge says how many
those were. This is licensed by the themes being *illustrative rather than
confirmatory*; correctness and agent error stay fully double-coded because they
feed pre-specified tests.

Without `agent_errors.csv` the H4 calibration test falls back to outcome
correctness and says so loudly in the report. Without `themes.csv` the paper's
Qualitative Findings section cannot be written.

### 5. Final analysis pass

```bash
make run
```

Now the correctness section is populated and reports both counts — items scored
and participants coded. Items below 100% is *expected* when the cap binds:
participants past the cap were never meant to be coded, and section 0a of the
report gives that gap as a table. What the report shouts about is the other case
— a **coded** participant with unscored tasks, which means adjudication is
unfinished and the model is quietly fitted on a subset.

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

## Why the coded set is capped

Recruitment and coding scale differently, and the pipeline is built around that.

Sessions are self-serve and unproctored, so one more participant costs you
nothing and improves **every instrumented measure** — task time (the primary
outcome), TLX, effort, the survey, delegation, the funnel, the manipulation
check. The three human codings cost roughly 1.5 person-hours per participant,
are linear in N, and are essentially the whole researcher cost of the study.
Recruitment is by open link, so N can comfortably run past 30.

So **recruitment is uncapped and the coded set is capped**: the first `CAP`
participants per condition in enrolment order (`provisioned_at`, ties broken by
code). Everyone else contributes to every instrumented analysis. The rule lives
in `coding_packet.coded_set` and is applied identically to all three packets, so
the three codings describe one group of people.

Why this survives review:

- **Enrolment order is stamped at provisioning**, before the participant does
  anything, so nothing about how a session went can move someone across the cap.
  Selecting on completion, data quality or agent use would all fail this.
- **The cap is per condition**, and condition is assigned by minimisation
  (least-filled arm first), so enrolment order is already decorrelated from
  condition. A prefix of it cannot load one arm.
- **The residual risk is drift** — early enrollees may differ from late ones —
  and it is *tested*, not asserted: `coded_set_comparison.csv` compares coded
  against uncoded participants on baseline time, baseline effort, agent-task time
  and invocation rate. Those exist for uncoded participants precisely because the
  cap removes hand coding, not telemetry. Drift would bound generalisation; it
  cannot bias the between-condition contrasts, since the cap is within condition.
- **The gap is reported, not smoothed over.** `coding_coverage.csv` gives
  collected vs. coded per condition, and the sensitivity panel refits the
  confirmatory contrast on the coded participants alone, so time and correctness
  can be read on a common set.

Two knobs, both pre-registered: `CAP=10` (per condition) and `PHASE=agent`
(agent-phase tasks only — the only ones any correctness model reads). Together
they hold correctness to 6×30 items instead of 12×N. The raw exports are kept for
everyone, so both are reversible without re-running a single session.

## The three codings

Item counts below assume the defaults (`CAP=10`, `PHASE=agent`) and N collected
above the cap:

| Coding | File | Items | Double-coded? | Blind to condition? |
|---|---|---|---|---|
| Correctness 0–3, per task | `correctness.csv` | 180 (6 × 30) | Fully | **Yes, mandatory** |
| Factual + judgement error, per invoked agent task | `agent_errors.csv` | ~160 | Fully | No — cannot be |
| Theme, per open-ended response | `themes.csv` | 120 (4 × 30) | 25% reliability sample | Mostly |

**Code correctness first, and keep it separate.** The agent-error packet shows the
agent's transcript, which gives the condition away — a C1 agent never acts, a C3
one talks about pull requests. That is unavoidable: the transcript is the
evidence being coded. So the blinded measure is finished before the unblinded one
starts.

**But condition is not the thing to worry about here.** Condition is not the
contrast in the calibration test — it compares override rates *between strata of
agent correctness* — so a coder who infers the condition has no route to move the
result. What must be hidden is the **outcome**: a coder who reads "no, that's
wrong, undo it" has inferred the stratifying variable from the behaviour the
hypothesis predicts, and H4 confirms itself out of nothing. The packet therefore
truncates each exchange after the agent's last turn.

**Why agent-error coding exists at all.** H4 has two halves. "Delegation rises
with scope" is the delegation ratio. "Reliance is imperfectly calibrated to where
the agent is actually correct" needs a judgement about whether *the agent* was
right, and correctness cannot supply it: a participant who catches a wrong
suggestion and fixes it produces a fully correct outcome from a wrong agent —
exactly H4's case. The fallback to outcome correctness is real but weaker, and
the report labels it that way rather than substituting it silently.

**`ANSWER_KEY.md` does most of the work.** Every participant works in a clone of
one fixed seed — 12 cards, 18 messages, 3 issues, 3 PRs — so "PR #34 is the
vaccination-reminder one" is a fact with an answer. `make packet-agent-errors`
generates the key from `pettracker_seed.json` and it ships with the packet. That
is what makes `factual_error` a lookup rather than a memory test, and it is why
that column, not `judgement_error`, is the stratifier the analysis uses.

### Adjudication: use a third coder, not a discussion

Disagreements resolve one of two ways, and the default is the weaker one:

- **Discussion to consensus** — cheap, but the more confident or more senior
  coder tends to win, and if that person knows the hypotheses, bias re-enters at
  exactly the step blinding was protecting.
- **Independent third coder** (`--adjudicator`) — resolves disputed items blind,
  without discussion. `make merge` writes `*_disputes.csv` containing **keys
  only**: the adjudicator never sees what either coder chose, because seeing a 1
  and a 0 anchors the judgement and turns the third pass into a vote.

```bash
make merge-agent-errors        # writes agent_errors_disputes.csv (keys only)
# third coder fills the blank value column(s) from the packet, then:
.venv/bin/python coding_packet.py --kind agent-errors \
  --merge coding/agent_error_coder_1.csv coding/agent_error_coder_2.csv \
  --adjudicator data/agent_errors_disputes.csv --out data/agent_errors.csv
```

The volume is proportional to disagreement, not to the corpus — a third person
adjudicating all three codings does far less work than a fourth coder would.

### Correctness in two rounds (agent tasks first — and only)

Correctness would be 12 items per participant — over half the total coding — and
**every correctness model in this pipeline fits on agent-phase tasks only**. The
baseline scores would feed the balance check and the κ sample, nothing else. So
the packet is built in halves, and the pre-registered decision is that only the
first half is run:

```bash
make packet                  # PHASE=agent by default — the half the models read
make merge                   # -> data/correctness.csv

# later, only if a reviewer asks:
make packet PHASE=baseline   # the other 6 per participant
make merge  PHASE=baseline   # merged into the SAME correctness.csv
```

Round two **adds to** the file rather than replacing it: rows whose keys the new
merge covers are updated, rows it doesn't cover are carried through untouched.
Re-running a round after adjudication replaces that round in place — no
duplicates. Coder files are suffixed (`coder_1_agent.csv`,
`coder_1_baseline.csv`) so a second round cannot overwrite the first.

**Nothing is lost by deferring.** `task_final_state` is exported and retained for
all twelve tasks, so the baseline packet can be built at any point — including
after a reviewer asks. What you'd give up by never doing it:

- one of three rows in the baseline balance check (it keeps time and effort,
  both free from telemetry, and time is the one that matters — it's the covariate
  in the primary model);
- κ reported on ~144 items instead of ~288. Measured on synthetic data this
  barely moves (0.80 vs 0.77), and the kappa-paradox worry — that ceilinged
  agent-task scores would depress κ — does not materialise at these counts.

`PHASE=agent` is the default because the decision is already taken and recorded
in `../WELD_ANALYSIS_PLAN.tex` (§"Scope of the correctness coding"). **It was
taken before coding started, which is the part that matters** — deciding it after
the first session, or after seeing a balance result you dislike, would turn a
defensible scoping choice into a post-hoc one.

### Dividing the work

The two knobs above are what keep this tractable as N grows. Uncapped and
unscoped, the coding grows without bound: at N=45 it is 540 correctness items,
~270 agent-error items and 180 theme responses, all double-coded — about 2,000
individual codings, or 30+ hours each for two people. That is longer than the
study takes to collect.

With the defaults (`CAP=10`, `PHASE=agent`, `RELFRAC=0.25`) the coding is fixed
at roughly 460 items regardless of how many people you recruit:

| Coding | Items | Coder 1 | Coder 2 |
|---|---|---|---|
| Correctness | 180 | 180 | 180 |
| Agent error | ~160 | 160 | 160 |
| Themes | 120 | 120 | 30 |

→ ~460 and ~370 codings respectively, roughly 12–16 hours each including joint
adjudication, spread over 2–3 weeks because fatigue binds well before hours do.
**And it does not grow.** Recruiting 60 people costs the same coding as
recruiting 30, while every instrumented result gets better.

With **three** people, split by coding — A+B on correctness, B+C on agent error,
A+C on themes — and nobody codes both correctness and agent error for the same
participant, which removes the residual risk that a coder remembers a final state
while reading that participant's transcript. If a third is available, that is the
cleaner design, and it is also who adjudicates.

Either way, **finish and lock correctness before agent-error coding opens.**

## What is in the correctness coding packet

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
| `load.py` | Reads the exports plus the three human codings, parses the JSON payload column. The only place that knows the server's export shape. |
| `derive.py` | Task-level and participant-level tables: baseline normalisation, idle rule, TLX/SUS/trust scales, delegation. |
| `analyze.py` | Manipulation check, participant flow, primary timing model + model contrasts + sensitivity panel, timing validity, agent uptake, correctness, TLX deltas and the effort model, agent funnel, reliance detail, H4 calibration, themes, RQ3 correlations. |
| `figures.py` | Figures 2–5. |
| `synthesize.py` | Fake exports for testing the pipeline. |
| `coding_packet.py` | Builds all three coding packets (`--kind correctness\|agent-errors\|themes`); merges the coders' scores back. |
| `run_all.py` | Orchestrates everything, fits the R model of record, writes the report. |
| `sensitivity.py` | Minimum-detectable-effect calculation quoted in §5.3. Importable; `run_all.py` folds its headline into the report. |
| `test_pipeline.py` | Regression tests for the invariants above (`make test`). |
| `Makefile` | Every workflow step as a target (`make help`). |

## Decisions worth knowing about

**Baseline anchors.** Comprehension → B6; single-surface → B1; cross-surface →
mean of B2–B5. The baseline enters models as a covariate, not as a pre-divided
ratio; `normalised_time` exists for plotting and per-participant summaries. The
same anchors are built for per-task *effort*, which is what the H3(b) effort
ANCOVA regresses on.

**The confirmatory contrast comes from the model.** `model_contrasts` takes
C3-vs-C1 on cross-surface tasks out of the fitted ANCOVA; `focused_contrast`
computes the same comparison on pre-divided ratios and is the *descriptive*
companion, not the test. Having argued against pre-dividing, the pipeline cannot
then take its headline from a t-test on those ratios. C3-vs-C2 differences two
coefficients and gets its SE from the covariance matrix — differencing two
independently quoted intervals overstates the width on exactly the contrast that
isolates the GitHub rung.

**Four fits, every run.** `sensitivity_panel` reports the confirmatory contrast
under primary (ITT), idle-adjusted, interruptions-excluded, and as-used. The
primary row is of record, fixed in advance. The report shouts if the ratio moves
by more than 0.10 across variants or if significance is unstable — because a
result that depends on which variant you quote has to be reported as such.

**Two different "executed" counts.** The delegation ratio's numerator excludes
drafted messages (the participant is the one who posted). "Did the call land"
must *include* them — a draft ran. Conflating the two reports a ~7% gate-block
rate as a ~34% failure rate. `reliance_detail` carries `executed_total`,
`executed_actions` and `executed_drafts` separately so the distinction survives.

**RQ3 multiplicity.** The correlation table is ~100 cells; five would clear p<.05
with nothing there. One correlation is pre-specified primary
(`analyze.PRIMARY_PERCEPTION`) and reported unadjusted; the rest are exploratory
and carry a Benjamini–Hochberg `p_fdr` over that family, with the test count
printed. Read no single exploratory cell as a finding.

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
- Participant flow: the per-condition rows sum to the `ALL` row, and
  `with_all_12_tasks` matches `completed` unless someone skipped.
- The sensitivity panel's four rows point the same way. If they don't, that is a
  result about robustness, and it goes in Results.
- Agent uptake is well under 100% somewhere — if every task shows agent use, check
  whether `agent_invoked` is being emitted correctly rather than assuming
  universal uptake.
- The H4 calibration table says `agent-error coding`, not `FALLBACK`. If it says
  FALLBACK, the agent-error coding is missing and the weaker measure is in use.
- κ is reported for all three codings, pre-adjudication, and none of them is
  exactly 1.000 (which would mean the coder columns are identical — check the
  merge, not your luck).
