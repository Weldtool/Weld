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
    #: The fitted statsmodels result, kept so contrasts can be taken from the
    #: model's own covariance matrix rather than re-derived from ratios.
    fit: object | None = None
    #: Observations and participants the fit actually used, for the missingness
    #: accounting in `timing_validity`.
    n_obs: int = 0
    n_participants: int = 0


def coded_participants(tasks: pd.DataFrame) -> set[str]:
    """Participants whose agent-phase tasks carry a human correctness score.

    Read off the merged data rather than off the coding manifest, so it reflects
    what was actually coded and not what was intended to be.
    """
    if tasks.empty or "correctness" not in tasks.columns:
        return set()
    scored = tasks[(tasks["phase"] == "agent") & tasks["correctness"].notna()]
    return set(scored["participant_code"])


def primary_time_model(
    tasks: pd.DataFrame,
    idle_adjusted: bool = False,
    agent_used_only: bool = False,
    exclude_interrupted: bool = False,
    coded_subset: bool = False,
    name: str = "primary_time",
) -> ModelResult:
    """log(time) ~ condition * task_type + log(baseline) + (1 | participant).

    Fitted on agent-phase tasks only; the baseline enters as a covariate rather
    than as a pre-computed ratio (paper Section 5.2).

    The four flags are the pre-specified sensitivity variants, not analyst
    choices: `idle_adjusted` applies the visible-idle correction (plan, "Timing
    validity"), `exclude_interrupted` drops tasks the participant flagged as
    interrupted (same section), `agent_used_only` restricts to tasks with at
    least one `agent_invoked` -- the as-used companion to the intention-to-treat
    default (plan, "Availability, not instruction") -- and `coded_subset`
    restricts to participants whose correctness was hand-coded. Every variant is
    fitted on every run and reported side by side, so which one is quoted is not
    decided after seeing the numbers.

    `coded_subset` exists because recruitment is uncapped and coding is not
    (plan, "The coded set is capped by enrolment order"), so time is estimated on
    more people than correctness is. The plan's standing commitment is that
    correctness is *always* reported beside time -- and a speed result from N
    people paired with an accuracy result from fewer is not that pairing unless
    the two are also shown on a common set. This variant is that showing: same
    model, restricted to the participants correctness exists for, so the reader
    can see whether the headline is a property of the manipulation or of the
    extra participants.
    """
    agent = tasks[(tasks["phase"] == "agent")].copy()
    if coded_subset:
        coded = coded_participants(tasks)
        if not coded:
            return ModelResult(name, "no correctness coding present",
                               pd.DataFrame(),
                               ["Correctness has not been coded yet; the "
                                "coded-subset refit has nothing to fit on."])
        agent = agent[agent["participant_code"].isin(coded)]
    if idle_adjusted:
        agent["log_time"] = np.log(
            agent["task_time_ms_idle_adj"].where(agent["task_time_ms_idle_adj"] > 0)
        )
    if exclude_interrupted and "was_interrupted" in agent.columns:
        agent = agent[agent["was_interrupted"] != True]  # noqa: E712 - nullable boolean
    if agent_used_only and "agent_invocations" in agent.columns:
        agent = agent[agent["agent_invocations"] > 0]
    agent = agent.dropna(subset=["log_time", "log_baseline", "agent_condition", "task_type"])

    notes = []
    if (
        agent["participant_code"].nunique() < 3
        or len(agent) < 10
        or agent["agent_condition"].nunique() < 2
        or agent["task_type"].nunique() < 2
    ):
        return ModelResult(name, "insufficient data to fit", pd.DataFrame(),
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
        return ModelResult(name, f"model did not converge ({last})",
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
    if exclude_interrupted:
        notes.append("Interrupted tasks excluded (pre-specified sensitivity).")
    if agent_used_only:
        notes.append("As-used: restricted to tasks with >=1 agent invocation.")
    if coded_subset:
        notes.append("Restricted to correctness-coded participants, so the "
                     "time and correctness results rest on one set of people.")
    return ModelResult(
        name, str(fit.summary()), tbl, notes, fit=fit,
        n_obs=len(agent), n_participants=int(agent["participant_code"].nunique()),
    )


#: The confirmatory test, fixed in advance (plan, "Confirmatory vs supporting"):
#: C3 vs C1 on cross-surface tasks. `cross_surface` is the reference level of
#: `task_type` and `C1` the reference level of `agent_condition`, so this contrast
#: is a single model coefficient -- but it is named here rather than left for a
#: reader to locate in a coefficient table, and the secondary rungs below need the
#: covariance matrix to difference two coefficients correctly.
_COND_TERM = "C(agent_condition, Treatment('C1'))[T.{}]"


def model_contrasts(result: ModelResult) -> pd.DataFrame:
    """Baseline-adjusted condition contrasts on cross-surface tasks, from the fit.

    This is the inferential form of the confirmatory test. `focused_contrast`
    below computes the same comparison on pre-divided normalised ratios, which
    the plan is explicit is the *descriptive* quantity: dividing by the anchor
    treats it as measured without error, and the plan's whole reason for an
    ANCOVA is to avoid exactly that. So the number the paper leads with comes
    from here, and the ratio contrast is reported beside it as the intuitive
    companion. If the two ever disagree materially that is itself reportable.

    Standard errors for C3-vs-C2 come from the model's covariance matrix
    (Var(a-b) = Var(a) + Var(b) - 2Cov(a,b)); computing it as a difference of two
    independently-quoted CIs would overstate the width.
    """
    fit = result.fit
    if fit is None:
        return pd.DataFrame()
    params, cov = fit.params, fit.cov_params()
    rows = []
    for a, b in (("C1", "C3"), ("C1", "C2"), ("C2", "C3")):
        # Contrast vector in the coefficient basis: +1 on b's term, -1 on a's
        # (C1 is the reference, so its term is the implicit zero vector).
        weights: dict[str, float] = {}
        if b != "C1":
            weights[_COND_TERM.format(b)] = weights.get(_COND_TERM.format(b), 0.0) + 1.0
        if a != "C1":
            weights[_COND_TERM.format(a)] = weights.get(_COND_TERM.format(a), 0.0) - 1.0
        if not all(term in params.index for term in weights):
            continue
        est = float(sum(w * params[t] for t, w in weights.items()))
        var = 0.0
        for t1, w1 in weights.items():
            for t2, w2 in weights.items():
                var += w1 * w2 * float(cov.loc[t1, t2])
        se = float(np.sqrt(var)) if var > 0 else np.nan
        lo, hi = (est - 1.96 * se, est + 1.96 * se) if np.isfinite(se) else (np.nan, np.nan)
        z = est / se if np.isfinite(se) and se > 0 else np.nan
        rows.append({
            "contrast": f"{b} - {a}",
            "log_diff": est, "se": se, "ci_low": lo, "ci_high": hi,
            "time_ratio": np.exp(est),
            "ratio_ci_low": np.exp(lo), "ratio_ci_high": np.exp(hi),
            "pct_change": 100.0 * (np.exp(est) - 1.0),
            "z": z,
            "p": 2 * (1 - stats.norm.cdf(abs(z))) if np.isfinite(z) else np.nan,
            "confirmatory": (a, b) == ("C1", "C3"),
        })
    return pd.DataFrame(rows)


def _welch_contrast(x: pd.Series, y: pd.Series, label: str) -> dict[str, object] | None:
    """Welch t-test of `y` against `x`, with a CI on the difference and Cohen's d.

    Returns None when either arm is too small to estimate a variance.
    """
    if len(x) < 2 or len(y) < 2:
        return None
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
    return {
        "contrast": label,
        "n_a": len(x), "n_b": len(y),
        "mean_a": x.mean(), "mean_b": y.mean(),
        "diff": diff,
        "ci_low": diff - crit * se, "ci_high": diff + crit * se,
        "cohens_d": d, "t": t, "p": p,
    }


def focused_contrast(tasks: pd.DataFrame) -> pd.DataFrame:
    """The descriptive companion to `model_contrasts`: C3 vs C1, cross-surface.

    Computed per participant first (mean of their cross-surface normalised
    times), so each person contributes one value and the between-subjects
    comparison is not inflated by treating two tasks as independent.

    This is *not* the confirmatory number. It divides by the baseline anchor,
    which the plan ("Why ANCOVA rather than a pre-divided ratio") rejects for
    inference because it treats the anchor as measured without error. It is
    reported because it is the quantity Figure 2 plots and the one a reader can
    hold in their head -- "C3 took 0.52x their own manual time" -- and because
    agreement between it and the model contrast is worth being able to see.
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
        row = _welch_contrast(
            per_p.loc[per_p["agent_condition"] == a, "normalised_time"],
            per_p.loc[per_p["agent_condition"] == b, "normalised_time"],
            f"{b} - {a}",
        )
        if row is not None:
            rows.append(row | {"confirmatory": (a, b) == ("C1", "C3")})
    return pd.DataFrame(rows)


def single_task_contrast(tasks: pd.DataFrame, task_id: str = "T6") -> pd.DataFrame:
    """One task on its own, C3 vs C2 first -- the top-of-the-ladder test.

    `focused_contrast` pools every cross-surface agent task, so the comparison
    the paper names specifically ("T6 specifically: C3 vs C2, the cleanest
    top-of-the-ladder test") is not any row it returns. T6 is the only task
    where C2 and C3 differ solely in whether GitHub is in scope, which is what
    makes it the cleanest read on the top rung -- and, being one task rather
    than a mean of two, it is materially less powered than the pooled contrast.
    Reported alongside it rather than instead of it.
    """
    one = tasks[(tasks["phase"] == "agent") & (tasks["task_id"] == task_id)]
    one = one.dropna(subset=["normalised_time"])
    rows = []
    for a, b in (("C2", "C3"), ("C1", "C3"), ("C1", "C2")):
        row = _welch_contrast(
            one.loc[one["agent_condition"] == a, "normalised_time"],
            one.loc[one["agent_condition"] == b, "normalised_time"],
            f"{b} - {a}",
        )
        if row is not None:
            rows.append(row | {"task_id": task_id, "headline": (a, b) == ("C2", "C3")})
    return pd.DataFrame(rows)


# --- RQ1: timing validity and pre-specified sensitivities -------------------


def timing_validity(tasks: pd.DataFrame) -> pd.DataFrame:
    """How often the timing caveats actually bit, per condition.

    The plan commits to three numbers that are easy to promise and easy to
    forget: how many tasks triggered the visible-idle rule, the interruption
    rate, and how much time the idle correction removed. Reported per condition,
    because a correction that lands unevenly across arms is a confound rather
    than a cleanup -- that is the thing a reader needs to be able to check.
    """
    agent = tasks[tasks["phase"] == "agent"]
    if agent.empty:
        return pd.DataFrame()
    rows = []
    for cond, g in list(agent.groupby("agent_condition")) + [("ALL", agent)]:
        interrupted = g["was_interrupted"].fillna(False).astype(bool) if "was_interrupted" in g else pd.Series(dtype=bool)
        fired = g["idle_rule_fired"].fillna(False).astype(bool) if "idle_rule_fired" in g else pd.Series(dtype=bool)
        idle_min = (g["idle_ms"].sum() / 60000.0) if "idle_ms" in g else np.nan
        rows.append({
            "agent_condition": cond,
            "tasks": len(g),
            "idle_rule_fired": int(fired.sum()),
            "pct_idle_fired": 100.0 * float(fired.mean()) if len(g) else np.nan,
            "idle_minutes_removed": idle_min,
            "interrupted": int(interrupted.sum()),
            "pct_interrupted": 100.0 * float(interrupted.mean()) if len(g) else np.nan,
            "median_pause_count": float(g["pause_count"].median()) if "pause_count" in g else np.nan,
        })
    return pd.DataFrame(rows)


def agent_uptake(tasks: pd.DataFrame) -> pd.DataFrame:
    """Agent uptake per condition -- the exposure the ITT estimate is diluted by.

    Task wording says the assistant *is available*, never to use it, so that RQ2
    measures chosen delegation rather than compliance (plan, "Availability, not
    instruction"). The cost is that some agent-phase tasks are completed with no
    agent use at all, which the intention-to-treat models still include. This is
    the number that tells a reader how much of the ITT estimate is diluted, and
    it is what makes the as-used model interpretable rather than cherry-picked.
    """
    agent = tasks[tasks["phase"] == "agent"]
    if agent.empty or "agent_invocations" not in agent.columns:
        return pd.DataFrame()
    rows = []
    for cond, g in list(agent.groupby("agent_condition")) + [("ALL", agent)]:
        used = g["agent_invocations"] > 0
        by_p = g.groupby("participant_code")["agent_invocations"].sum()
        rows.append({
            "agent_condition": cond,
            "tasks": len(g),
            "tasks_with_agent_use": int(used.sum()),
            "pct_tasks_with_agent_use": 100.0 * float(used.mean()),
            "participants": int(g["participant_code"].nunique()),
            "participants_never_invoking": int((by_p == 0).sum()),
            "median_invocations_per_task": float(g["agent_invocations"].median()),
        })
    return pd.DataFrame(rows)


#: The pre-specified sensitivity variants of the primary model. The first is the
#: model of record; the rest exist so that a result which depends on which one is
#: used gets reported as such rather than resolved in the direction we prefer.
SENSITIVITY_VARIANTS = {
    "primary (ITT, active time)": {},
    "idle-adjusted": {"idle_adjusted": True},
    "interrupted tasks excluded": {"exclude_interrupted": True},
    "as-used (>=1 invocation)": {"agent_used_only": True},
    # The coded set is capped and recruitment is not, so this is the row that
    # shows the headline on the same people the correctness result is computed
    # on. It is a variant rather than the model of record because throwing away
    # collected, instrumented observations to match the coding budget would be
    # the wrong direction: the primary outcome is free, and precision on it is
    # what the extra participants are for.
    "coded subset (same people as correctness)": {"coded_subset": True},
}


def sensitivity_panel(tasks: pd.DataFrame) -> pd.DataFrame:
    """The confirmatory contrast under every pre-specified variant, side by side.

    One row per variant, all fitted on every run. A reader should be able to see
    in one place whether the headline survives the idle correction, the dropped
    interruptions and the as-used restriction -- and the plan pre-commits to
    reporting divergence rather than adjudicating it.
    """
    rows = []
    for label, kwargs in SENSITIVITY_VARIANTS.items():
        res = primary_time_model(tasks, name=label, **kwargs)
        con = model_contrasts(res)
        if con.empty:
            rows.append({"variant": label, "n_obs": res.n_obs,
                         "n_participants": res.n_participants,
                         "note": res.summary or "not fitted"})
            continue
        conf = con[con["confirmatory"]]
        if conf.empty:
            continue
        r = conf.iloc[0]
        rows.append({
            "variant": label,
            "n_obs": res.n_obs, "n_participants": res.n_participants,
            "contrast": r["contrast"],
            "time_ratio": r["time_ratio"],
            "ratio_ci_low": r["ratio_ci_low"], "ratio_ci_high": r["ratio_ci_high"],
            "pct_change": r["pct_change"], "p": r["p"],
            "note": "",
        })
    return pd.DataFrame(rows)


def participant_flow(data: StudyData, tasks: pd.DataFrame) -> pd.DataFrame:
    """CONSORT-style flow: provisioned -> completed, with the losses named.

    The plan asks for this twice (RQ2's supporting descriptives and "Participant
    flow and timing validity") and it is the first thing in the reporting order.
    Dropouts are not a footnote at n~8 per cell: one withdrawal is an eighth of
    an arm, so where it landed has to be visible.
    """
    p = data.participants
    if p.empty:
        return pd.DataFrame()
    abandoned = set(
        of_type(data.events, "session_abandoned")["participant_code"].unique()
    )
    skipped = of_type(data.events, "task_skipped")
    rows = []
    for cond, g in list(p.groupby("agent_condition")) + [("ALL", p)]:
        codes = set(g["participant_code"])
        their_tasks = tasks[tasks["participant_code"].isin(codes)] if not tasks.empty else tasks
        completed_12 = 0
        if not their_tasks.empty:
            per_p = their_tasks.groupby("participant_code")["task_id"].nunique()
            completed_12 = int((per_p >= 12).sum())
        rows.append({
            "agent_condition": cond,
            "provisioned": len(g),
            "consented": int(g["consent_given_at"].notna().sum()),
            "completed": int((g["status"] == "completed").sum()),
            "abandoned": len(codes & abandoned),
            "tasks_skipped": int(skipped["participant_code"].isin(codes).sum()),
            "with_all_12_tasks": completed_12,
            "analysed_agent_phase": int(
                tasks[(tasks["phase"] == "agent") & tasks["participant_code"].isin(codes)][
                    "participant_code"
                ].nunique()
            ) if not tasks.empty else 0,
        })
    return pd.DataFrame(rows)


def coding_coverage(data: StudyData, tasks: pd.DataFrame) -> pd.DataFrame:
    """How much of the collected sample each human coding actually covers.

    Recruitment is self-serve, so the collected sample is whatever the link
    brings in; the coded sample is capped by a pre-registered rule
    (`coding_packet.coded_set`). That gap has to be a number in the paper rather
    than a sentence in Limitations -- a reader comparing a time result on N
    people with a correctness result on fewer needs to see the two counts side
    by side to judge the comparison, and burying it invites the suspicion that
    the coded set was chosen rather than capped.

    One row per condition plus ALL, so an accidentally lopsided coded set --
    which the per-condition cap exists to prevent -- would be visible here rather
    than inferred from a model that quietly lost an arm.
    """
    if tasks.empty:
        return pd.DataFrame()
    p = data.participants
    agent = tasks[tasks["phase"] == "agent"]
    coded = coded_participants(tasks)
    themed = set(data.themes["participant_code"]) if not data.themes.empty else set()
    errored = (set(data.agent_errors["participant_code"])
               if not data.agent_errors.empty else set())

    rows = []
    for cond, g in list(p.groupby("agent_condition")) + [("ALL", p)]:
        codes = set(g["participant_code"])
        theirs = agent[agent["participant_code"].isin(codes)]
        n_items = len(theirs)
        n_scored = int(theirs["correctness"].notna().sum()) if n_items else 0
        rows.append({
            "agent_condition": cond,
            "collected": len(codes),
            "with_agent_tasks": int(theirs["participant_code"].nunique()),
            "correctness_coded": len(codes & coded),
            "agent_error_coded": len(codes & errored),
            "themes_coded": len(codes & themed),
            "agent_items": n_items,
            "agent_items_scored": n_scored,
            "pct_items_scored": (100.0 * n_scored / n_items) if n_items else np.nan,
        })
    return pd.DataFrame(rows)


#: Instrumented, coder-free measures on which the coded and uncoded halves of the
#: sample are compared. All of them exist for every participant regardless of
#: whether anyone hand-coded them, which is the whole point: they are the only
#: evidence available about whether the cap selected a different kind of person.
CODED_SET_MEASURES = [
    ("baseline time (ms)", "task_time_ms", "baseline"),
    ("baseline effort", "effort", "baseline"),
    ("agent-task time (ms)", "task_time_ms", "agent"),
    ("agent invocations / task", "agent_invocations", "agent"),
]


def coded_set_comparison(tasks: pd.DataFrame) -> pd.DataFrame:
    """Did the enrolment-order cap select a different kind of participant?

    The cap's defence is that enrolment order is fixed at provisioning and so
    cannot be steered by anything that happens in a session. That argument is
    sound about *manipulation* but silent about *drift*: people who sign up in
    the first week may differ from people who sign up in the third -- keener,
    or reached through a different recruitment channel -- and a coded set that
    is a prefix of enrolment order would inherit the difference.

    So we test it rather than argue it. Every measure here is instrumented and
    therefore exists for coded and uncoded participants alike, which is exactly
    what makes the comparison possible at all: the cap removes hand coding, not
    telemetry. Mann-Whitney because these are small, skewed samples.

    Two readings, both pre-specified. No difference is evidence the coded set
    stands in for the collected one. A difference does *not* invalidate the
    between-condition contrasts -- the cap is applied within condition, so any
    drift lands on all three arms alike -- but it does bound how far the
    correctness result generalises to the full collected sample, and that bound
    is reported rather than assumed away.
    """
    if tasks.empty:
        return pd.DataFrame()
    coded = coded_participants(tasks)
    if not coded:
        return pd.DataFrame()
    rows = []
    for label, col, phase in CODED_SET_MEASURES:
        if col not in tasks.columns:
            continue
        sub = tasks[tasks["phase"] == phase]
        per_p = sub.groupby("participant_code")[col].mean().dropna()
        if per_p.empty:
            continue
        inside = per_p[per_p.index.isin(coded)]
        outside = per_p[~per_p.index.isin(coded)]
        u, p = (np.nan, np.nan)
        if len(inside) >= 2 and len(outside) >= 2:
            u, p = stats.mannwhitneyu(inside, outside, alternative="two-sided")
        rows.append({
            "measure": label,
            "n_coded": len(inside),
            "n_uncoded": len(outside),
            "mean_coded": float(inside.mean()) if len(inside) else np.nan,
            "mean_uncoded": float(outside.mean()) if len(outside) else np.nan,
            "mannwhitney_U": u,
            "p": p,
            "note": "" if len(outside) >= 2 else
                    "no uncoded participants — the cap did not bind",
        })
    return pd.DataFrame(rows)


def baseline_balance(tasks: pd.DataFrame) -> pd.DataFrame:
    """Were the three arms comparable *before* the agent appeared?

    Condition is assigned at provisioning, but the six baseline tasks are
    identical across arms and run with no agent, so baseline performance is a
    randomisation check: if C3 participants were simply faster or more accurate
    to begin with, the between-condition contrasts inherit that head start. At
    n~8 per cell, minimisation keeps the cells equal in *size* but cannot make
    them equal in *ability*, so this is exactly where a small sample can bite.

    It also gives the baseline correctness coding a job. Coders score all twelve
    tasks, but every correctness model in this pipeline fits on agent-phase tasks
    only -- without this check the six baseline scores per participant would be
    collected and never used, which is a lot of human effort for nothing.

    Expected result: no differences. A difference here is not a finding about the
    manipulation, it is a caveat on every finding that follows, and it belongs in
    Results with the flow numbers.
    """
    base = tasks[tasks["phase"] == "baseline"]
    if base.empty:
        return pd.DataFrame()
    rows = []
    for measure, col in (("baseline time (ms)", "task_time_ms"),
                         ("baseline correctness", "correctness"),
                         ("baseline effort", "effort")):
        if col not in base.columns:
            continue
        # One value per participant, so the test is between people rather than
        # between tasks -- condition is a property of the person.
        per_p = base.groupby(["participant_code", "agent_condition"])[col].mean().reset_index()
        per_p = per_p.dropna(subset=[col])
        if per_p.empty:
            continue
        arrays = [g[col].values for _, g in per_p.groupby("agent_condition")]
        h, p = (np.nan, np.nan)
        if len(arrays) >= 2 and all(len(a) >= 2 for a in arrays):
            h, p = stats.kruskal(*arrays)
        row = {"measure": measure, "n": len(per_p), "kruskal_H": h, "kruskal_p": p}
        for cond, g in per_p.groupby("agent_condition"):
            row[cond] = float(g[col].mean())
        rows.append(row)
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


def tlx_subscales_by_condition(participants: pd.DataFrame) -> pd.DataFrame:
    """Per-subscale TLX deltas. The paper asks for "composite and subscales".

    `derive.py` already computes a `tlx_<subscale>_delta` per participant; this
    only aggregates them. Note `performance` is reverse-scored upstream into the
    workload direction, so a *positive* delta there means worse perceived
    performance, the same sign convention as every other subscale.
    """
    cols = [f"tlx_{s}_delta" for s in derive.TLX_SUBSCALES]
    cols = [c for c in cols if c in participants.columns]
    if not cols:
        return pd.DataFrame()
    rows = []
    for col in cols:
        d = participants.dropna(subset=[col])
        if d.empty:
            continue
        arrays = [g[col].values for _, g in d.groupby("agent_condition")]
        h, p = (np.nan, np.nan)
        if len(arrays) >= 2 and all(len(a) >= 2 for a in arrays):
            h, p = stats.kruskal(*arrays)
        for cond, g in d.groupby("agent_condition"):
            rows.append({
                "subscale": col.removeprefix("tlx_").removesuffix("_delta"),
                "agent_condition": cond,
                "n": len(g),
                "mean_delta": g[col].mean(),
                "sd": g[col].std(ddof=1),
                "kruskal_H": h,
                "kruskal_p": p,
                "reverse_scored": col == "tlx_performance_delta",
            })
    return pd.DataFrame(rows)


def effort_model(tasks: pd.DataFrame) -> ModelResult:
    """H3(b): per-task effort, anchored on each participant's baseline effort.

    The plan specifies two load analyses and calls this one out by name --
    `effort ~ condition * task_type + baseline_effort + (1 | participant)`, the
    same ANCOVA logic as the timing model. The global TLX delta is the headline;
    this is what localises it by task type, and without it H3 rests on a single
    six-item scale collected twice.
    """
    agent = tasks[tasks["phase"] == "agent"].copy()
    notes: list[str] = []
    if agent.empty or "effort" not in agent.columns:
        return ModelResult("effort", "no per-task effort ratings", pd.DataFrame(),
                           ["`task_rating.effort` absent from the export."])
    agent = agent.dropna(subset=["effort", "baseline_effort", "agent_condition", "task_type"])
    if (
        len(agent) < 10
        or agent["participant_code"].nunique() < 3
        or agent["effort"].nunique() < 2
    ):
        return ModelResult("effort", "insufficient data to fit", pd.DataFrame(),
                           ["Too few rated agent tasks to fit the effort model."])

    model = smf.mixedlm(
        "effort ~ C(agent_condition, Treatment('C1')) * C(task_type, Treatment('cross_surface'))"
        " + baseline_effort",
        data=agent,
        groups=agent["participant_code"],
    )
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
        return ModelResult("effort", f"model did not converge ({last})", pd.DataFrame(),
                           notes + ["Effort model failed to converge."])

    tbl = pd.DataFrame({
        "coef": fit.params, "se": fit.bse, "z": fit.tvalues, "p": fit.pvalues,
        "ci_low": fit.conf_int()[0], "ci_high": fit.conf_int()[1],
    })
    notes.append("Effort is 1-7; coefficients are scale points, not log units.")
    return ModelResult("effort", str(fit.summary()), tbl, notes, fit=fit,
                       n_obs=len(agent),
                       n_participants=int(agent["participant_code"].nunique()))


#: Sauro & Lewis's benchmark mean for SUS. The paper asks for the score "against
#: published norms", and this is the number that phrase points at.
SUS_NORM = 68.0


def sus_by_condition(participants: pd.DataFrame) -> pd.DataFrame:
    """SUS per condition and pooled, each tested against the published norm.

    Usability is not the manipulation -- every condition uses the same app --
    so the informative reading is the pooled row against the norm, plus a
    across-condition test that should come back null. A condition difference
    here would mean the agent's scope had leaked into how usable the workspace
    itself felt, which is worth knowing.
    """
    if "sus" not in participants.columns:
        return pd.DataFrame()
    d = participants.dropna(subset=["sus"])
    if d.empty:
        return pd.DataFrame()
    rows = []
    for cond, g in list(d.groupby("agent_condition")) + [("ALL", d)]:
        v = g["sus"]
        t, p = stats.ttest_1samp(v, SUS_NORM) if len(v) >= 2 else (np.nan, np.nan)
        rows.append({
            "agent_condition": cond, "n": len(v),
            "mean": v.mean(), "sd": v.std(ddof=1) if len(v) >= 2 else np.nan,
            "vs_norm": v.mean() - SUS_NORM, "norm": SUS_NORM, "t": t, "p": p,
        })
    out = pd.DataFrame(rows)
    arrays = [g["sus"].values for _, g in d.groupby("agent_condition")]
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
    out = (
        d.groupby("agent_condition")["delegation_ratio"]
        .agg(["count", "mean", "std", "median"])
        .reset_index()
    )
    # Drafts are excluded from the ratio's numerator by construction (see
    # derive.py); carrying the count here keeps that channel of delegation
    # visible next to the ratio it was deliberately left out of.
    if "chat_drafts" in participants.columns:
        drafts = d.groupby("agent_condition")["chat_drafts"].mean()
        out = out.merge(
            drafts.rename("mean_chat_drafts").reset_index(), on="agent_condition", how="left"
        )
    arrays = [g["delegation_ratio"].values for _, g in d.groupby("agent_condition")]
    if len(arrays) >= 2 and all(len(a) >= 2 for a in arrays):
        h, p = stats.kruskal(*arrays)
        out.attrs["kruskal_H"], out.attrs["kruskal_p"] = h, p
    return out


def reliance_detail(tasks: pd.DataFrame, participants: pd.DataFrame) -> pd.DataFrame:
    """The RQ2 descriptives the plan lists but the funnel's raw counts don't give.

    The funnel counts events; these are rates, which is what a reader can
    actually compare across conditions that invoked the agent different numbers
    of times. Tool error and rejection rates are per attempted call, not per
    participant. Cross-reference traversals and view switches are the behavioural
    side of "integration usage" -- the second is also what RQ3 pairs against
    perceived context-switching.
    """
    agent = tasks[tasks["phase"] == "agent"]
    if agent.empty:
        return pd.DataFrame()
    rows = []
    for cond, g in list(agent.groupby("agent_condition")) + [("ALL", agent)]:
        attempted = float(g["agent_tool_calls_attempted"].sum())
        # Two different "executed" counts exist in this pipeline and they must
        # not be confused. The delegation ratio's numerator excludes drafted
        # messages (the participant is the one who posted). "Did the call land"
        # is a different question, and a draft landed: it ran, and produced a
        # draft. So this rate is computed on both together, and the draft count
        # is carried beside it. Using the delegation numerator here would report
        # a ~34% failure rate for a gate blocking ~7%.
        actions = float(g["agent_tool_calls_executed"].sum())
        drafts = float(g["agent_chat_drafts"].sum())
        executed = actions + drafts
        codes = set(g["participant_code"])
        vsr = np.nan
        if "view_switch_rate" in participants.columns:
            sub = participants[participants["participant_code"].isin(codes)]
            vsr = float(sub["view_switch_rate"].mean()) if not sub.empty else np.nan
        rows.append({
            "agent_condition": cond,
            "tool_calls_attempted": int(attempted),
            "executed_total": int(executed),
            "executed_actions": int(actions),
            "executed_drafts": int(drafts),
            # Not-executed covers both a gate block and a runtime failure; the
            # funnel separates them, this is the headline "how often did an
            # attempted call not land".
            "pct_not_executed": 100.0 * (1 - executed / attempted) if attempted else np.nan,
            "cross_references": int(g["cross_references"].sum()),
            "cross_refs_per_task": float(g["cross_references"].mean()),
            "view_switches_per_task": float(g["view_changes"].mean()),
            "view_switch_rate_per_min": vsr,
            "github_views_per_task": float(g["github_views"].mean()),
        })
    return pd.DataFrame(rows)


#: Events that mean the participant overrode the agent rather than accepting it.
#: `agent_output_edited` counts: editing a drafted message before sending is a
#: correction, not an acceptance.
OVERRIDE_EVENTS = [
    "agent_suggestion_rejected", "agent_output_edited", "agent_action_undone",
]

#: The gap in override rate (percentage points, wrong minus right) that would
#: count as users tracking the agent's actual correctness. Fixed in advance and
#: deliberately unambitious: H4 predicts users do *worse* than this, and a
#: threshold chosen after seeing the data could be moved to make any result
#: support the hypothesis. Used only to decide whether an interval is wide
#: enough to be uninformative, never as a test.
CALIBRATED_GAP_PCT = 25.0


def _wilson(k: int, n: int, conf: float = 0.95) -> tuple[float, float]:
    """Wilson score interval -- correct near 0 and 1, where the normal
    approximation is not, which is where override rates will sit."""
    if n == 0:
        return (np.nan, np.nan)
    z = stats.norm.ppf(1 - (1 - conf) / 2)
    p = k / n
    centre = (p + z * z / (2 * n)) / (1 + z * z / n)
    half = z * np.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return (max(0.0, centre - half), min(1.0, centre + half))


def _newcombe_diff_ci(k1: int, n1: int, k2: int, n2: int,
                      conf: float = 0.95) -> tuple[float, float]:
    """Newcombe's interval for a difference of two independent proportions.

    Built from the two Wilson intervals rather than from a pooled normal
    approximation, because at ~30 tasks in the smaller stratum the normal
    interval can run past 0 or 1 and understate the uncertainty. This interval is
    what the plan pre-commits to reporting for the H4 gap: the hypothesis
    predicts a *small* difference, so it is satisfied by a null, and only an
    interval narrow enough to exclude good calibration is evidence for it.
    """
    if n1 == 0 or n2 == 0:
        return (np.nan, np.nan)
    p1, p2 = k1 / n1, k2 / n2
    l1, u1 = _wilson(k1, n1, conf)
    l2, u2 = _wilson(k2, n2, conf)
    lo = (p1 - p2) - np.sqrt((p1 - l1) ** 2 + (u2 - p2) ** 2)
    hi = (p1 - p2) + np.sqrt((u1 - p1) ** 2 + (p2 - l2) ** 2)
    return (float(lo), float(hi))


def _stratify(values: pd.Series, label: object) -> pd.Series:
    """Map a numeric coding onto string strata, leaving uncoded rows as None.

    A plain `np.where` cannot do this: mixing NaN with string branches has no
    common dtype, and coercing to object silently turns missing codings into the
    string "nan", which would then be counted as its own stratum.
    """
    return pd.Series(
        [None if pd.isna(v) else label(v) for v in values],
        index=values.index, dtype="object",
    )


def calibration(data: StudyData, tasks: pd.DataFrame) -> pd.DataFrame:
    """H4's calibration claim: do users override where the agent is wrong?

    H4 has two halves. "Delegation rises with scope" is the delegation ratio.
    "Reliance is imperfectly calibrated to where the agent is actually correct"
    is this, and it needs a ground truth for *the agent's* output, which is a
    different judgement from whether the participant's task ended up right.

    Two sources, in order of preference:

    1. `agent_errors.csv` — per-task coding of whether the agent's contribution
       was correct, from the exported transcript (see `coding_packet.py
       --agent-errors`). This is the measure the paper names.
    2. Coded task correctness, as a fallback when the agent-error coding is not
       available. Weaker: a task can end correct because the participant fixed a
       wrong agent suggestion, which is precisely the case H4 cares about, so
       the fallback under-detects good calibration. Labelled as such in the
       output rather than silently substituted.

    The quantity is the override rate within each stratum. Good calibration is
    overriding much more often when the agent was wrong; H4 predicts the gap is
    smaller than it should be, and a gap near zero is the strong form of the
    claim.
    """
    agent = tasks[tasks["phase"] == "agent"].copy()
    if agent.empty:
        return pd.DataFrame()

    ev = data.events
    overrides = ev[ev["event_type"].isin(OVERRIDE_EVENTS)]
    key = ["participant_code", "current_task_id"]
    counts = (
        overrides.groupby(key).size().rename("overrides").reset_index()
        .rename(columns={"current_task_id": "task_id"})
    )
    agent = agent.merge(counts, on=["participant_code", "task_id"], how="left")
    agent["overrides"] = agent["overrides"].fillna(0)
    agent["overrode"] = agent["overrides"] > 0

    errors = data.agent_errors
    # `factual_error` is the pre-specified stratifier: it is settled by looking a
    # claim up in the seeded workspace, so two coders can agree on it, and it is
    # the closest available reading of H4's "where the agent is actually
    # correct." `agent_error` (factual OR judgement) is the secondary stratum,
    # softer because the judgement half is an opinion about helpfulness.
    stratifier = next(
        (c for c in ("factual_error", "agent_error") if c in errors.columns),
        None,
    ) if not errors.empty else None
    if stratifier is not None:
        e = errors[["participant_code", "task_id", stratifier]].copy()
        e[stratifier] = pd.to_numeric(e[stratifier], errors="coerce")
        agent = agent.merge(e, on=["participant_code", "task_id"], how="left")
        agent["stratum"] = _stratify(
            agent[stratifier], lambda v: "agent wrong" if v > 0 else "agent correct"
        )
        source = f"agent-error coding ({stratifier})"
    elif "correctness" in agent.columns:
        agent["stratum"] = _stratify(
            agent["correctness"],
            lambda v: "outcome not fully correct" if v < 3 else "outcome fully correct",
        )
        source = "FALLBACK: outcome correctness (agent_errors.csv absent)"
    else:
        return pd.DataFrame()

    # Only tasks where the agent was actually used can show override behaviour;
    # including untouched tasks would dilute both strata with structural zeros.
    used = agent[(agent["agent_invocations"] > 0) & agent["stratum"].notna()]
    if used.empty:
        return pd.DataFrame()

    rows = []
    for (cond, stratum), g in used.groupby(["agent_condition", "stratum"]):
        rows.append({
            "agent_condition": cond, "stratum": stratum, "tasks": len(g),
            "overrode": int(g["overrode"].sum()),
            "override_rate": 100.0 * float(g["overrode"].mean()),
            "mean_overrides": float(g["overrides"].mean()),
            "source": source,
        })
    for stratum, g in used.groupby("stratum"):
        rows.append({
            "agent_condition": "ALL", "stratum": stratum, "tasks": len(g),
            "overrode": int(g["overrode"].sum()),
            "override_rate": 100.0 * float(g["overrode"].mean()),
            "mean_overrides": float(g["overrides"].mean()),
            "source": source,
        })
    out = pd.DataFrame(rows)

    # The headline is the gap between strata pooled: how much more often people
    # override when the agent was wrong. Fisher's exact, because at n~8 per cell
    # the 2x2 is sparse and chi-square is not trustworthy there.
    pooled = used.groupby("stratum")["overrode"].agg(["sum", "count"])
    if len(pooled) == 2:
        table = [
            [int(r["sum"]), int(r["count"] - r["sum"])]
            for _, r in pooled.iterrows()
        ]
        try:
            odds, p = stats.fisher_exact(table)
        except ValueError:
            odds, p = np.nan, np.nan
        out.attrs["fisher_odds_ratio"] = odds
        out.attrs["fisher_p"] = p
        rates = used.groupby("stratum")["overrode"].mean() * 100.0
        out.attrs["gap_pct_points"] = float(rates.max() - rates.min())
        out.attrs["strata"] = list(pooled.index)

        # The gap with its interval. H4 predicts a *small* gap, so it is a
        # hypothesis a null would satisfy; the plan pre-commits to reading a wide
        # interval as uninformative rather than as support, and that judgement
        # needs the interval, not just the point estimate and a p-value.
        wrong = [s for s in pooled.index if "wrong" in s or "not fully" in s]
        right = [s for s in pooled.index if s not in wrong]
        if len(wrong) == 1 and len(right) == 1:
            kw, nw = int(pooled.loc[wrong[0], "sum"]), int(pooled.loc[wrong[0], "count"])
            kr, nr = int(pooled.loc[right[0], "sum"]), int(pooled.loc[right[0], "count"])
            lo, hi = _newcombe_diff_ci(kw, nw, kr, nr)
            out.attrs["gap_signed_pct"] = 100.0 * (kw / nw - kr / nr)
            out.attrs["gap_ci_low"] = 100.0 * lo
            out.attrs["gap_ci_high"] = 100.0 * hi
            # "Uninformative" is defined here rather than after seeing it: an
            # interval that contains both no difference and a difference large
            # enough to count as good calibration cannot distinguish the two.
            out.attrs["gap_uninformative"] = bool(
                lo < 0.0 < hi or (lo <= 0.0 and hi >= CALIBRATED_GAP_PCT / 100.0)
            )
    out.attrs["source"] = source
    return out


def theme_summary(data: StudyData) -> pd.DataFrame:
    """Themes from the four open-ended items, with per-condition counts.

    The plan and the paper both promise a thematic analysis here; this is the
    aggregation half. The coding itself is human and arrives as `themes.csv`
    from `coding_packet.py --qualitative` (one row per participant x item x
    assigned theme, so an answer carrying two themes is two rows).
    """
    themes = data.themes
    if themes.empty or "theme" not in themes.columns:
        return pd.DataFrame()
    t = themes.dropna(subset=["theme"]).copy()
    if t.empty:
        return pd.DataFrame()
    cond = data.participants.set_index("participant_code")["agent_condition"]
    t["agent_condition"] = t["participant_code"].map(cond)
    out = (
        t.groupby(["item_id", "theme"])
        .agg(mentions=("participant_code", "size"),
             participants=("participant_code", "nunique"))
        .reset_index()
    )
    by_cond = (
        t.groupby(["item_id", "theme", "agent_condition"])["participant_code"]
        .nunique().unstack(fill_value=0)
    )
    out = out.merge(by_cond.reset_index(), on=["item_id", "theme"], how="left")
    return out.sort_values(["item_id", "mentions"], ascending=[True, False])


# --- RQ3: perception vs measurement ----------------------------------------


def _spearman_ci(rho: float, n: int, conf: float = 0.95) -> tuple[float, float]:
    """Fisher-z interval for a Spearman correlation.

    The SE carries the 1.06 Bonett-Wright correction for rank correlations --
    Fisher's 1/sqrt(n-3) is derived for Pearson and runs slightly narrow on
    Spearman. Undefined below n=5; the paper asks for CIs on the per-condition
    correlations too, and those are n=8, so the guard earns its keep.
    """
    if not np.isfinite(rho) or n < 5:
        return (np.nan, np.nan)
    z = np.arctanh(np.clip(rho, -0.999999, 0.999999))
    se = 1.06 / np.sqrt(n - 3)
    crit = stats.norm.ppf(1 - (1 - conf) / 2)
    return (float(np.tanh(z - crit * se)), float(np.tanh(z + crit * se)))


def _bh_fdr(p: pd.Series) -> pd.Series:
    """Benjamini-Hochberg adjusted p-values, NaNs passed through."""
    out = pd.Series(np.nan, index=p.index, dtype=float)
    valid = p.dropna().sort_values()
    if valid.empty:
        return out
    m = len(valid)
    adj = valid.values * m / np.arange(1, m + 1)
    adj = np.minimum.accumulate(adj[::-1])[::-1]  # enforce monotonicity
    out.loc[valid.index] = np.minimum(adj, 1.0)
    return out


#: The one RQ3 correlation fixed in advance (plan, "Perception-behavior gap"):
#: perceived speedup against measured normalised time on the cross-surface agent
#: tasks, pooled across conditions. Everything else in the table is exploratory
#: and FDR-adjusted as one family.
PRIMARY_PERCEPTION = ("pooled", "trust_speed_perceived", "mean_normalised_time_cross")


def perception_correlations(participants: pd.DataFrame) -> pd.DataFrame:
    """Spearman correlations of perception against measured performance.

    Pooled across conditions (headline) and repeated within each condition (the
    version immune to the seeing-vs-doing confound, but thinly powered).

    Multiplicity is handled rather than ignored. This table is ~100 correlations;
    at alpha=0.05 five of them are expected to clear significance with no effect
    present at all, and per-condition rows are n=7-8, where a single participant
    moves rho by 0.3. So: one correlation is pre-specified as primary
    (`PRIMARY_PERCEPTION`) and reported unadjusted, and every other row is marked
    exploratory and carries a Benjamini-Hochberg adjusted p over that family.
    The plan already labels all of H5 exploratory; this makes the table honest
    about how many ways it was looked at.
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
                lo, hi = _spearman_ci(rho, len(sub))
                rows.append({"scope": scope, "perception": pcol, "measured": mcol,
                             "n": len(sub), "spearman_rho": rho,
                             "ci_low": lo, "ci_high": hi, "p": p})

    _corr(participants, "pooled")
    for cond, grp in participants.groupby("agent_condition"):
        _corr(grp, cond)
    out = pd.DataFrame(rows)
    if out.empty:
        return out

    out["primary"] = [
        (s, pc, mc) == PRIMARY_PERCEPTION
        for s, pc, mc in zip(out["scope"], out["perception"], out["measured"])
    ]
    exploratory = out[~out["primary"]]
    out["p_fdr"] = _bh_fdr(exploratory["p"].reindex(out.index))
    out.attrs["n_exploratory_tests"] = int(len(exploratory))
    return out.sort_values("primary", ascending=False).reset_index(drop=True)


def switching_perception(participants: pd.DataFrame) -> pd.DataFrame:
    """Perceived context-switching against the behavioural view-switch rate.

    Promised by name in both the plan and the paper's RQ3 subsection, and it is
    the one place where a *self-report* is checked against a *behavioural*
    counterpart of the same construct rather than against an outcome. If people
    who switched views more also report switching more, the integration
    perception items are measuring something real; if not, that is worth as much
    as the trust result and belongs beside it.
    """
    need = {"int_switch_perceived", "view_switch_rate"}
    if not need.issubset(participants.columns):
        return pd.DataFrame()
    rows = []
    for scope, frame in [("pooled", participants)] + list(
        participants.groupby("agent_condition")
    ):
        sub = frame[["int_switch_perceived", "view_switch_rate"]].dropna()
        if len(sub) < 4:
            continue
        rho, p = stats.spearmanr(sub["int_switch_perceived"], sub["view_switch_rate"])
        lo, hi = _spearman_ci(rho, len(sub))
        rows.append({
            "scope": scope, "n": len(sub), "spearman_rho": rho,
            "ci_low": lo, "ci_high": hi, "p": p,
            "mean_perceived": float(sub["int_switch_perceived"].mean()),
            "mean_view_switch_rate": float(sub["view_switch_rate"].mean()),
        })
    return pd.DataFrame(rows)


def integration_perception(participants: pd.DataFrame) -> pd.DataFrame:
    """Per-condition means for the integration-perception items.

    Measured on every participant and, until now, analysed nowhere. These do not
    test a hypothesis: scope is manipulated, perceived integration is not. They
    are reported because a reader is entitled to ask whether participants
    noticed the thing the paper is about, and because a flat profile across
    conditions is itself informative about how visible agent scope was.
    """
    cols = [c for c in derive.INTEGRATION_ITEMS if c in participants.columns]
    if not cols:
        return pd.DataFrame()
    rows = []
    for item in cols:
        d = participants.dropna(subset=[item])
        if d.empty:
            continue
        arrays = [g[item].values for _, g in d.groupby("agent_condition")]
        h, p = (np.nan, np.nan)
        if len(arrays) >= 2 and all(len(a) >= 2 for a in arrays):
            h, p = stats.kruskal(*arrays)
        row = {"item": item, "n": len(d), "mean": float(d[item].mean()),
               "kruskal_H": h, "kruskal_p": p}
        for cond, g in d.groupby("agent_condition"):
            row[cond] = float(g[item].mean())
        rows.append(row)
    return pd.DataFrame(rows)


def reliance_calibration(participants: pd.DataFrame) -> pd.DataFrame:
    """trust_rely crossed with trust_verify -- over- vs under-reliance."""
    if not {"trust_rely", "trust_verify"}.issubset(participants.columns):
        return pd.DataFrame()
    d = participants.dropna(subset=["trust_rely", "trust_verify"])
    if d.empty:
        return pd.DataFrame()
    out = (
        d.groupby("agent_condition")[["trust_rely", "trust_verify"]]
        .agg(["count", "mean", "std"])
        .reset_index()
    )
    # The paper asks for the cross-tab itself, not only the per-condition means:
    # who says they lean on the agent more than they check it.
    out.attrs["crosstab"] = pd.crosstab(d["trust_rely"], d["trust_verify"])
    out.attrs["over_reliant"] = int((d["trust_rely"] > d["trust_verify"]).sum())
    out.attrs["under_reliant"] = int((d["trust_rely"] < d["trust_verify"]).sum())
    out.attrs["balanced"] = int((d["trust_rely"] == d["trust_verify"]).sum())
    out.attrs["n_total"] = len(d)
    return out


# --- Inter-coder agreement --------------------------------------------------

#: The rubric's fixed scale. Agreement is computed over this, not over the
#: values that happen to appear, so an unused category can't inflate kappa.
SCORE_MIN, SCORE_MAX = 0, 3


def nominal_agreement(frame: pd.DataFrame, label: str,
                      cols: tuple[str, str] = ("coder_1", "coder_2")) -> pd.DataFrame:
    """Unweighted Cohen's kappa for a categorical coding (themes, agent errors).

    Separate from `coder_agreement` on purpose. That one is quadratic-weighted
    because correctness is ordinal and a 2-vs-3 disagreement is milder than a
    0-vs-3. Themes are nominal -- there is no "close" between two theme labels --
    and the agent-error codes are binary, where weighting is meaningless. Either
    way, weighting here would invent an ordering the rubric does not have.
    """
    empty = pd.DataFrame(columns=["coding", "n_items", "n_coded", "kappa",
                                  "pct_exact", "note"])
    if frame is None or frame.empty or not set(cols).issubset(frame.columns):
        return empty
    # `dropna` is what makes a partially double-coded corpus safe here: themes
    # are single-coded outside a reliability subset, and rows only one person saw
    # must not enter the agreement calculation. Reporting `n_coded` beside
    # `n_items` keeps the coverage visible instead of leaving the reader to
    # assume kappa spans the whole corpus.
    n_coded = len(frame)
    d = frame[list(cols)].dropna()
    d.columns = ["coder_1", "coder_2"]
    d = d.astype(str)
    if d.empty:
        return pd.DataFrame([{"coding": label, "n_items": 0, "n_coded": n_coded,
                              "kappa": np.nan, "pct_exact": np.nan,
                              "note": "no coder pairs present"}])
    cats = sorted(set(d["coder_1"]) | set(d["coder_2"]))
    idx = {c: i for i, c in enumerate(cats)}
    k = len(cats)
    observed = np.zeros((k, k))
    for a, b in zip(d["coder_1"], d["coder_2"]):
        observed[idx[a], idx[b]] += 1
    n = observed.sum()
    observed /= n
    po = float(np.trace(observed))
    pe = float((observed.sum(axis=1) * observed.sum(axis=0)).sum())
    note = ""
    if pe >= 1.0:
        kappa, note = np.nan, "kappa undefined — coders used a single category"
    else:
        kappa = (po - pe) / (1 - pe)
    if int(n) < n_coded:
        note = (f"{note}; " if note else "") + (
            f"double-coded reliability sample: {int(n)} of {n_coded} responses "
            f"({100.0 * n / n_coded:.0f}%)"
        )
    return pd.DataFrame([{"coding": label, "n_items": int(n), "n_coded": n_coded,
                          "kappa": kappa, "pct_exact": 100.0 * po, "note": note}])


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
    primary = primary_time_model(tasks)
    return {
        "task_table": tasks,
        "participant_table": participants,
        "participant_flow": participant_flow(data, tasks),
        "coding_coverage": coding_coverage(data, tasks),
        "coded_set_comparison": coded_set_comparison(tasks),
        "baseline_balance": baseline_balance(tasks),
        "manipulation_check": manipulation_check(data),
        "primary_time_model": primary,
        "model_contrasts": model_contrasts(primary),
        "primary_time_model_idle_adjusted": primary_time_model(tasks, idle_adjusted=True),
        "sensitivity_panel": sensitivity_panel(tasks),
        "timing_validity": timing_validity(tasks),
        "agent_uptake": agent_uptake(tasks),
        "focused_contrast": focused_contrast(tasks),
        "t6_contrast": single_task_contrast(tasks, "T6"),
        "correctness": correctness_model(tasks),
        "coder_agreement": coder_agreement(data.correctness),
        "effort_model": effort_model(tasks),
        "tlx_delta": tlx_delta_by_condition(participants),
        "tlx_subscales": tlx_subscales_by_condition(participants),
        "sus": sus_by_condition(participants),
        "agent_funnel": agent_funnel(data, tasks),
        "delegation": delegation_by_condition(participants),
        "reliance_detail": reliance_detail(tasks, participants),
        "calibration": calibration(data, tasks),
        "agent_error_agreement": pd.concat(
            [
                nominal_agreement(data.agent_errors, name, (f"{name}_1", f"{name}_2"))
                for name in ("factual_error", "judgement_error")
            ] + [nominal_agreement(data.agent_errors, "agent_error")],
            ignore_index=True,
        ),
        "themes": theme_summary(data),
        "theme_agreement": nominal_agreement(data.themes, "themes"),
        "perception_correlations": perception_correlations(participants),
        "switching_perception": switching_perception(participants),
        "integration_perception": integration_perception(participants),
        "reliance_calibration": reliance_calibration(participants),
        "trust_alpha": derive.trust_alpha(data),
    }
