"""
The models and tests named in the paper's Analysis Plan.

Order matters and mirrors the Results section: manipulation check first, then
RQ1, RQ2, RQ3, then load and usability. Nothing here decides anything that was
not fixed in advance; where a tool limitation forces a deviation it is stated
loudly in the returned object and in the printed report.
"""

from __future__ import annotations

import warnings
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pandas as pd
from scipy import stats

import derive
from load import StudyData, of_type

with warnings.catch_warnings():
    warnings.simplefilter("ignore")
    import statsmodels.formula.api as smf


# --- Manipulation check -----------------------------------------------------


#: The context sources the gate can attach, and what each condition may receive.
CONTEXT_SOURCES = {"chat", "board", "github"}
ALLOWED_SOURCES = {
    "C1": {"chat"},
    "C2": {"chat", "board"},
    "C3": {"chat", "board", "github"},
}


def _sources_present(payload: object) -> set[str]:
    """Which context sources a snapshot actually carried.

    Structural, and deliberately so. The snapshot embeds real chat message
    *text*, and the seeded teammates talk to each other about the board and
    about GitHub — Alex ends the SQLite thread with "can someone capture this
    decision on the board". Searching the payload for those words as substrings
    reads ordinary English as a scoping leak and marks 100% of C1 and C2
    snapshots as violations, which is precisely backwards: it condemns the gate
    for chat content the gate is supposed to include.

    The gate declares what it attached in `sources_included` and attaches each
    source under its own top-level key. Both are consulted and unioned, so a gate
    bug that attached board data while declaring chat-only cannot hide behind its
    own declaration.
    """
    if not isinstance(payload, dict):
        return set()
    declared = payload.get("sources_included") or []
    if isinstance(declared, str):  # tolerate a scalar
        declared = [declared]
    declared = {str(s).lower() for s in declared}
    attached = {str(k).lower() for k in payload}
    return (declared | attached) & CONTEXT_SOURCES


def manipulation_check(data: StudyData) -> pd.DataFrame:
    """Prove the scoping held, from the `agent_context_sent` snapshots.

    Expectation (paper Section 5.1): 100% of C1 snapshots contain no board and no
    GitHub content, and 100% of C2 snapshots contain no GitHub content. Any
    non-zero cell is a leak and must be reported, not dropped.
    """
    snaps = of_type(data.events, "agent_context_sent")
    if snaps.empty:
        return pd.DataFrame(
            columns=["agent_condition", "snapshots", "with_board", "with_github",
                     "pct_board", "pct_github", "violations"]
        )

    rows = []
    for cond, grp in snaps.groupby("agent_condition"):
        present = grp["payload"].map(_sources_present)
        has_board = present.map(lambda s: "board" in s)
        has_gh = present.map(lambda s: "github" in s)
        n = len(grp)
        # A violation is any source outside this condition's allowlist — stated
        # once, rather than per-condition, so an unknown source counts too.
        allowed = ALLOWED_SOURCES.get(cond, CONTEXT_SOURCES)
        violations = int(present.map(lambda s: bool(s - allowed)).sum())
        rows.append(
            {
                "agent_condition": cond,
                "snapshots": n,
                "with_board": int(has_board.sum()),
                "with_github": int(has_gh.sum()),
                "pct_board": 100.0 * has_board.mean(),
                "pct_github": 100.0 * has_gh.mean(),
                "violations": violations,
            }
        )
    return pd.DataFrame(rows).sort_values("agent_condition").reset_index(drop=True)


# --- RQ1: primary timing model ---------------------------------------------


@dataclass
class ModelResult:
    name: str
    summary: str
    table: pd.DataFrame
    notes: list[str] = field(default_factory=list)


def primary_time_model(tasks: pd.DataFrame, idle_adjusted: bool = False) -> ModelResult:
    """log(time) ~ condition * task_type + log(baseline) + (1 | participant).

    Fitted on agent-phase tasks only; the baseline enters as a covariate rather
    than as a pre-computed ratio (paper Section 5.2).
    """
    agent = tasks[(tasks["phase"] == "agent")].copy()
    if idle_adjusted:
        agent["log_time"] = np.log(
            agent["task_time_ms_idle_adj"].where(agent["task_time_ms_idle_adj"] > 0)
        )
    agent = agent.dropna(subset=["log_time", "log_baseline", "agent_condition", "task_type"])

    notes = []
    if agent["participant_code"].nunique() < 3 or len(agent) < 10:
        return ModelResult("primary_time", "insufficient data to fit", pd.DataFrame(),
                           ["Too few observations to fit the mixed model."])

    model = smf.mixedlm(
        "log_time ~ C(agent_condition, Treatment('C1')) * C(task_type, Treatment('cross_surface')) + log_baseline",
        data=agent,
        groups=agent["participant_code"],
    )
    # statsmodels' default tries a sequence of optimizers; a single forced
    # choice can hit a singular Hessian on well-conditioned data (lbfgs does on
    # ours). Fall back explicitly rather than let the whole run die.
    fit = None
    for method in (None, "powell", "cg", "nm"):
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                fit = model.fit(reml=True) if method is None else model.fit(
                    reml=True, method=method
                )
            if method is not None:
                notes.append(f"Primary optimizer failed; converged with '{method}'.")
            break
        except Exception as exc:  # noqa: BLE001 - report, do not mask
            last = f"{type(exc).__name__}: {exc}"
    if fit is None:
        return ModelResult("primary_time", f"model did not converge ({last})",
                           pd.DataFrame(), notes + ["Mixed model failed to converge."])
    if not getattr(fit, "converged", True):
        notes.append("WARNING: optimizer reported non-convergence; treat with caution.")

    tbl = pd.DataFrame(
        {
            "coef": fit.params,
            "se": fit.bse,
            "z": fit.tvalues,
            "p": fit.pvalues,
            "ci_low": fit.conf_int()[0],
            "ci_high": fit.conf_int()[1],
        }
    )
    # log coefficients are multiplicative effects on time; report both.
    tbl["time_ratio"] = np.exp(tbl["coef"])
    tbl["ratio_ci_low"] = np.exp(tbl["ci_low"])
    tbl["ratio_ci_high"] = np.exp(tbl["ci_high"])
    if idle_adjusted:
        notes.append("Fitted on idle-adjusted task time (pre-specified sensitivity).")
    return ModelResult("primary_time", str(fit.summary()), tbl, notes)


def focused_contrast(tasks: pd.DataFrame) -> pd.DataFrame:
    """The confirmatory test: C3 vs C1 on cross-surface agent tasks.

    Fitted per participant first (mean of their cross-surface normalised times),
    so each person contributes one value and the between-subjects comparison is
    not inflated by treating two tasks as independent.
    """
    cross = tasks[(tasks["phase"] == "agent") & (tasks["task_type"] == "cross_surface")]
    per_p = (
        cross.groupby(["participant_code", "agent_condition"])["normalised_time"]
        .mean()
        .reset_index()
        .dropna()
    )
    rows = []
    for a, b in (("C1", "C3"), ("C1", "C2"), ("C2", "C3")):
        x = per_p.loc[per_p["agent_condition"] == a, "normalised_time"]
        y = per_p.loc[per_p["agent_condition"] == b, "normalised_time"]
        if len(x) < 2 or len(y) < 2:
            continue
        t, p = stats.ttest_ind(y, x, equal_var=False)
        pooled = np.sqrt((x.var(ddof=1) + y.var(ddof=1)) / 2)
        d = (y.mean() - x.mean()) / pooled if pooled > 0 else np.nan
        # Welch CI on the difference in means.
        se = np.sqrt(x.var(ddof=1) / len(x) + y.var(ddof=1) / len(y))
        dfree = (x.var(ddof=1) / len(x) + y.var(ddof=1) / len(y)) ** 2 / (
            (x.var(ddof=1) / len(x)) ** 2 / (len(x) - 1)
            + (y.var(ddof=1) / len(y)) ** 2 / (len(y) - 1)
        )
        crit = stats.t.ppf(0.975, dfree)
        diff = y.mean() - x.mean()
        rows.append(
            {
                "contrast": f"{b} - {a}",
                "n_a": len(x), "n_b": len(y),
                "mean_a": x.mean(), "mean_b": y.mean(),
                "diff": diff,
                "ci_low": diff - crit * se, "ci_high": diff + crit * se,
                "cohens_d": d, "t": t, "p": p,
                "confirmatory": (a, b) == ("C1", "C3"),
            }
        )
    return pd.DataFrame(rows)


def correctness_model(tasks: pd.DataFrame) -> ModelResult:
    """Correctness by condition x task type.

    DEVIATION, stated openly: the plan specifies an ordinal cumulative-link
    *mixed* model. statsmodels has no CLMM, and R is not a dependency of this
    pipeline. We fit an ordinal model without the participant random effect and
    report it as approximate; `r_snippet()` emits the exact clmm call to run in R
    for the version of record.
    """
    agent = tasks[tasks["phase"] == "agent"].dropna(subset=["correctness"])
    notes = [
        "Approximate: no participant random effect (statsmodels lacks a CLMM). "
        "Run the emitted R snippet with ordinal::clmm for the reported model."
    ]
    if agent.empty or agent["correctness"].nunique() < 2:
        return ModelResult("correctness", "no correctness coding available",
                           pd.DataFrame(), notes)

    tab = (
        agent.groupby(["agent_condition", "task_type"])["correctness"]
        .agg(["count", "mean", "std"])
        .reset_index()
    )
    # Kruskal-Wallis across conditions within each task type: distribution-free
    # and appropriate at this sample size.
    tests = []
    for ttype, grp in agent.groupby("task_type"):
        arrays = [g["correctness"].values for _, g in grp.groupby("agent_condition")]
        if len(arrays) >= 2 and all(len(a) > 0 for a in arrays):
            try:
                h, p = stats.kruskal(*arrays)
            except ValueError:
                h, p = np.nan, np.nan
            tests.append({"task_type": ttype, "H": h, "p": p})
    return ModelResult("correctness", "", tab.merge(pd.DataFrame(tests), on="task_type",
                                                    how="left"), notes)


def r_snippet(task_table_path: str | Path = "tables/task_table.csv") -> str:
    """The R for the correctness model of record, plus its pre-specified fallback.

    `task_table_path` is baked in absolute by `run_all.py` so the script runs
    from any working directory — it used to be relative to `out/`, which meant
    the documented invocation failed unless you happened to `cd` there first.

    The fallback matters as much as the primary model here. The plan
    (`WELD_ANALYSIS_PLAN.tex`, "Correctness") *expects* the CLMM to struggle at
    7--10 per cell with scores piling up at 3, and pre-specifies what to do
    instead. So this script decides for itself: it fits the CLMM, checks whether
    the fit is real, tests proportional odds, and computes the fallback either
    way — labelling which one is the model of record. Leaving that to be
    improvised on results day is how a pre-specified fallback quietly becomes a
    post-hoc choice.
    """
    return f"""\
# Correctness model of record -- paper Section 5.2, WELD_ANALYSIS_PLAN.tex
# ("Correctness"). Generated by run_all.py; re-generated every run.
#
#   Rscript correctness_model.R
#
# Runs from any directory: the data path below was resolved when this file was
# written. Needs `ordinal`; `lme4` is optional and only used for the fallback's
# mixed logistic.

library(ordinal)

d <- read.csv("{task_table_path}")
d <- subset(d, phase == "agent" & !is.na(correctness))
if (nrow(d) == 0) {{
  stop("No coded agent-phase tasks. Add correctness.csv (see README) and re-run run_all.py.")
}}

d$correctness     <- factor(d$correctness, levels = 0:3, ordered = TRUE)
d$agent_condition <- relevel(factor(d$agent_condition), ref = "C1")
# cross_surface is the reference everywhere else in this analysis -- H2 is about
# cross-surface work. R would otherwise order factor levels alphabetically and
# silently report every contrast against comprehension instead.
d$task_type        <- relevel(factor(d$task_type), ref = "cross_surface")
d$participant_code <- factor(d$participant_code)

cat("N =", nrow(d), "agent-phase tasks from",
    nlevels(d$participant_code), "participants\\n\\n")
cat("Score distribution:\\n"); print(table(d$correctness)); cat("\\n")

## --- Primary: ordinal cumulative-link mixed model --------------------------

m <- tryCatch(
  clmm(correctness ~ agent_condition * task_type + (1 | participant_code),
       data = d),
  error = function(e) {{ cat("CLMM failed to fit:", conditionMessage(e), "\\n\\n"); NULL }}
)

# A returned object is not the same as a usable fit. Sparse cells and a ceiling
# produce fits whose standard errors are missing or enormous; those coefficients
# mean nothing and must not be reported as if they did.
clmm_usable <- FALSE
if (!is.null(m)) {{
  se <- tryCatch(coef(summary(m))[, "Std. Error"], error = function(e) NA_real_)
  clmm_usable <- all(is.finite(se)) && max(se, na.rm = TRUE) < 25
  cat("=== PRIMARY: cumulative-link mixed model ===\\n")
  print(summary(m))
  if (!clmm_usable) {{
    cat("\\n*** Fit is degenerate (missing or implausibly large standard errors).\\n")
    cat("*** Do not report these coefficients. The fallback below is the result.\\n")
  }}
}}

## --- Proportional-odds check -----------------------------------------------
# nominal_test() needs a clm, so this refits without the random effect. It is an
# approximation of the assumption check for the mixed model, which is the best
# available in `ordinal`; treat a flag here as reason to prefer the fallback.

po_violated <- NA
if (clmm_usable) {{
  po <- tryCatch({{
    fixed <- clm(correctness ~ agent_condition * task_type, data = d)
    nominal_test(fixed)
  }}, error = function(e) {{ cat("\\nProportional-odds test unavailable:",
                              conditionMessage(e), "\\n"); NULL }})
  if (!is.null(po)) {{
    cat("\\n=== Proportional-odds test (no random effect) ===\\n")
    print(po)
    p <- po[["Pr(>Chi)"]]
    po_violated <- any(!is.na(p) & p < 0.05)
    if (isTRUE(po_violated)) {{
      cat("\\n*** Proportional odds is violated -- the plan's fallback applies.\\n")
    }}
  }}
}}

## --- Pre-specified fallback ------------------------------------------------
# Collapse to fully-correct (== 3) vs not, per WELD_ANALYSIS_PLAN.tex. Computed
# every run: when the primary model holds this is a robustness check, and when
# it doesn't this is the result. Which one applies is stated below, not left to
# be decided after seeing it.

required <- !clmm_usable || isTRUE(po_violated)
cat("\\n\\n=== PRE-SPECIFIED FALLBACK: fully correct (3) vs not ===\\n")
cat(if (required) {{
  "STATUS: this is the model of record -- the primary model did not hold.\\n\\n"
}} else {{
  "STATUS: supplementary. The primary model above holds and is the result.\\n\\n"
}})

d$full <- as.integer(as.integer(as.character(d$correctness)) == 3)

# Wilson score interval -- correct at the proportions near 0 and 1 that a
# ceiling effect produces, where the normal approximation is not.
wilson <- function(x, n, conf = 0.95) {{
  if (n == 0) return(c(NA, NA))
  z <- qnorm(1 - (1 - conf) / 2); p <- x / n
  centre <- (p + z^2 / (2 * n)) / (1 + z^2 / n)
  half <- z * sqrt(p * (1 - p) / n + z^2 / (4 * n^2)) / (1 + z^2 / n)
  c(max(0, centre - half), min(1, centre + half))
}}

cells <- aggregate(full ~ agent_condition + task_type, data = d,
                   FUN = function(v) c(k = sum(v), n = length(v)))
cells <- do.call(data.frame, cells)
names(cells)[3:4] <- c("k", "n")
ci <- t(mapply(wilson, cells$k, cells$n))
cells$prop <- cells$k / cells$n
cells$lo <- ci[, 1]; cells$hi <- ci[, 2]
print(cells[order(cells$task_type, cells$agent_condition),
            c("agent_condition", "task_type", "k", "n", "prop", "lo", "hi")],
      row.names = FALSE, digits = 3)

# Mixed-effects logistic where it converges (plan: "where it converges").
if (requireNamespace("lme4", quietly = TRUE)) {{
  fit <- tryCatch(
    lme4::glmer(full ~ agent_condition * task_type + (1 | participant_code),
                data = d, family = binomial),
    error = function(e) {{ cat("\\nMixed logistic did not converge:",
                             conditionMessage(e), "\\n"); NULL }}
  )
  if (!is.null(fit)) {{ cat("\\n=== Mixed-effects logistic ===\\n"); print(summary(fit)) }}
}} else {{
  cat("\\n(lme4 not installed -- skipping the mixed logistic.",
      "install.packages(\\"lme4\\") to include it.)\\n")
}}

# Permutation test -- the plan's "exact test where it does not [converge]".
# Condition is between-subjects, so the labels are shuffled across participants,
# not across tasks; shuffling tasks would break the within-participant
# dependence the random effect exists to model and give a spuriously small p.
cat("\\n=== Permutation test on participant-level accuracy ===\\n")
pdat <- aggregate(full ~ participant_code + agent_condition, data = d, FUN = mean)
obs <- unname(kruskal.test(full ~ agent_condition, data = pdat)$statistic)
set.seed(20260812)
B <- 10000
null_stats <- replicate(B, {{
  shuffled <- pdat
  shuffled$agent_condition <- sample(shuffled$agent_condition)
  unname(kruskal.test(full ~ agent_condition, data = shuffled)$statistic)
}})
cat(sprintf("Kruskal-Wallis H = %.4f, permutation p = %.4f (B = %d)\\n",
            obs, (1 + sum(null_stats >= obs)) / (B + 1), B))
print(aggregate(full ~ agent_condition, data = pdat,
                FUN = function(v) c(n = length(v), mean = mean(v))))
"""


# --- RQ1 / H3: cognitive load ----------------------------------------------


def tlx_delta_by_condition(participants: pd.DataFrame) -> pd.DataFrame:
    """Between-condition comparison of the within-person TLX change."""
    if "tlx_delta" not in participants.columns:
        return pd.DataFrame()
    d = participants.dropna(subset=["tlx_delta"])
    if d.empty:
        return pd.DataFrame()
    rows = [
        {
            "agent_condition": cond,
            "n": len(g),
            "mean_delta": g["tlx_delta"].mean(),
            "sd": g["tlx_delta"].std(ddof=1),
        }
        for cond, g in d.groupby("agent_condition")
    ]
    out = pd.DataFrame(rows)
    arrays = [g["tlx_delta"].values for _, g in d.groupby("agent_condition")]
    if len(arrays) >= 2 and all(len(a) >= 2 for a in arrays):
        h, p = stats.kruskal(*arrays)
        out.attrs["kruskal_H"] = h
        out.attrs["kruskal_p"] = p
    return out


# --- RQ2: reliance ----------------------------------------------------------


def agent_funnel(data: StudyData, tasks: pd.DataFrame) -> pd.DataFrame:
    """Invocation -> tool attempted -> executed -> accepted / edited / undone."""
    ev = data.events
    steps = {
        "invoked": "agent_invoked",
        "panel_opened": "agent_panel_opened",
        "tool_attempted": "agent_tool_call",
        "suggestion_accepted": "agent_suggestion_accepted",
        "suggestion_rejected": "agent_suggestion_rejected",
        "output_edited": "agent_output_edited",
        "action_undone": "agent_action_undone",
        "tool_rejected": "agent_tool_rejected",
        "error": "agent_error",
    }
    rows = []
    for cond, grp in ev.groupby("agent_condition"):
        row = {"agent_condition": cond,
               "participants": grp["participant_code"].nunique()}
        for label, etype in steps.items():
            row[label] = int((grp["event_type"] == etype).sum())
        calls = grp[grp["event_type"] == "agent_tool_call"]
        row["tool_executed"] = int(calls["payload"].map(lambda p: bool(p.get("executed"))).sum())
        row["tool_blocked"] = int(calls["payload"].map(lambda p: p.get("allowed") is False).sum())
        rows.append(row)
    return pd.DataFrame(rows).sort_values("agent_condition").reset_index(drop=True)


def delegation_by_condition(participants: pd.DataFrame) -> pd.DataFrame:
    if "delegation_ratio" not in participants.columns:
        return pd.DataFrame()
    d = participants.dropna(subset=["delegation_ratio"])
    return (
        d.groupby("agent_condition")["delegation_ratio"]
        .agg(["count", "mean", "std", "median"])
        .reset_index()
    )


# --- RQ3: perception vs measurement ----------------------------------------


def perception_correlations(participants: pd.DataFrame) -> pd.DataFrame:
    """Spearman correlations of perception against measured performance.

    Pooled across conditions (headline) and repeated within each condition (the
    version immune to the seeing-vs-doing confound, but thinly powered).
    """
    perception = ["trust_composite", "trust_speed_perceived", "trust_rely",
                  "trust_verify", "trust_context"]
    measured = ["mean_normalised_time_cross", "mean_correctness_cross",
                "mean_normalised_time_all", "mean_correctness_all"]
    rows = []

    def _corr(frame: pd.DataFrame, scope: str) -> None:
        for pcol in perception:
            for mcol in measured:
                if pcol not in frame.columns or mcol not in frame.columns:
                    continue
                sub = frame[[pcol, mcol]].dropna()
                if len(sub) < 4:
                    continue
                rho, p = stats.spearmanr(sub[pcol], sub[mcol])
                rows.append({"scope": scope, "perception": pcol, "measured": mcol,
                             "n": len(sub), "spearman_rho": rho, "p": p})

    _corr(participants, "pooled")
    for cond, grp in participants.groupby("agent_condition"):
        _corr(grp, cond)
    return pd.DataFrame(rows)


def reliance_calibration(participants: pd.DataFrame) -> pd.DataFrame:
    """trust_rely crossed with trust_verify -- over- vs under-reliance."""
    if not {"trust_rely", "trust_verify"}.issubset(participants.columns):
        return pd.DataFrame()
    d = participants.dropna(subset=["trust_rely", "trust_verify"])
    if d.empty:
        return pd.DataFrame()
    return (
        d.groupby("agent_condition")[["trust_rely", "trust_verify"]]
        .agg(["count", "mean", "std"])
        .reset_index()
    )


# --- Inter-coder agreement --------------------------------------------------

#: The rubric's fixed scale. Agreement is computed over this, not over the
#: values that happen to appear, so an unused category can't inflate kappa.
SCORE_MIN, SCORE_MAX = 0, 3


def coder_agreement(correctness: pd.DataFrame) -> pd.DataFrame:
    """Quadratic-weighted Cohen's kappa between the two coders, pre-adjudication.

    The paper commits to reporting this (Section 5; `WELD_ANALYSIS_PLAN.tex`
    "Correctness coding"), and it must be the *pre*-adjudication figure — after
    adjudication the coders agree by construction and the number is meaningless.

    Quadratic weights because the score is ordinal: disagreeing 0 vs 3 is a
    worse failure than 2 vs 3, and unweighted kappa treats them alike.
    """
    cols = {"coder_1", "coder_2"}
    if correctness.empty or not cols.issubset(correctness.columns):
        return pd.DataFrame(columns=["n_items", "kappa_quadratic", "pct_exact",
                                     "pct_within_1", "note"])

    d = correctness[["coder_1", "coder_2"]].apply(pd.to_numeric, errors="coerce").dropna()
    if d.empty:
        return pd.DataFrame([{"n_items": 0, "kappa_quadratic": np.nan,
                              "pct_exact": np.nan, "pct_within_1": np.nan,
                              "note": "no coder pairs present"}])

    k = SCORE_MAX - SCORE_MIN + 1
    idx = range(SCORE_MIN, SCORE_MAX + 1)
    observed = np.zeros((k, k))
    for a, b in zip(d["coder_1"], d["coder_2"]):
        ai, bi = int(round(a)) - SCORE_MIN, int(round(b)) - SCORE_MIN
        if 0 <= ai < k and 0 <= bi < k:
            observed[ai, bi] += 1
    n = observed.sum()
    if n == 0:
        return pd.DataFrame([{"n_items": 0, "kappa_quadratic": np.nan,
                              "pct_exact": np.nan, "pct_within_1": np.nan,
                              "note": "all coder scores fell outside the rubric scale"}])
    observed /= n

    # Expected agreement under independence, from each coder's own marginals.
    expected = np.outer(observed.sum(axis=1), observed.sum(axis=0))
    i, j = np.meshgrid(np.array(idx), np.array(idx), indexing="ij")
    weights = ((i - j) ** 2) / float((k - 1) ** 2)

    denom = float((weights * expected).sum())
    note = ""
    if denom == 0:
        # Both coders used a single identical category throughout: chance
        # agreement is total, so kappa is undefined rather than perfect.
        kappa = np.nan
        note = "kappa undefined — no expected disagreement (coders used one category)"
    else:
        kappa = 1.0 - float((weights * observed).sum()) / denom

    diff = (d["coder_1"] - d["coder_2"]).abs()
    return pd.DataFrame([{
        "n_items": int(n),
        "kappa_quadratic": kappa,
        "pct_exact": 100.0 * float((diff == 0).mean()),
        "pct_within_1": 100.0 * float((diff <= 1).mean()),
        "note": note,
    }])


# --- Orchestration ----------------------------------------------------------


def run_all(data: StudyData) -> dict[str, object]:
    tasks = derive.task_table(data)
    participants = derive.participant_table(data, tasks)
    return {
        "task_table": tasks,
        "participant_table": participants,
        "manipulation_check": manipulation_check(data),
        "primary_time_model": primary_time_model(tasks),
        "primary_time_model_idle_adjusted": primary_time_model(tasks, idle_adjusted=True),
        "focused_contrast": focused_contrast(tasks),
        "correctness": correctness_model(tasks),
        "coder_agreement": coder_agreement(data.correctness),
        "tlx_delta": tlx_delta_by_condition(participants),
        "agent_funnel": agent_funnel(data, tasks),
        "delegation": delegation_by_condition(participants),
        "perception_correlations": perception_correlations(participants),
        "reliance_calibration": reliance_calibration(participants),
        "trust_alpha": derive.trust_alpha(data),
    }
