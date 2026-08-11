"""
Turn raw exports into the analysis tables the paper's Measures section names.

Everything in here is a decision that was fixed in advance in
WELD_ANALYSIS_PLAN.tex. Where a definition is a judgement call, the docstring
says so and names the section that fixed it. If you change a rule here, you are
deviating from the plan and must report it in the Results section.

Produces two tables:

  task_table()        one row per participant x task (the modelling unit)
  participant_table() one row per participant (surveys, scales, delegation)
"""

from __future__ import annotations

import numpy as np
import pandas as pd

from load import StudyData, of_type, payload_field

# --- Fixed study structure (mirrors apps/web/src/lib/study-tasks.ts) ---------

BASELINE_TASKS = ["B1", "B2", "B3", "B4", "B5", "B6"]
AGENT_TASKS = ["T1", "T2", "T3", "T4", "T5", "T6"]

TASK_TYPE = {
    "B1": "single_surface",
    "B2": "cross_surface",
    "B3": "cross_surface",
    "B4": "cross_surface",
    "B5": "cross_surface",
    "B6": "comprehension",
    "T1": "comprehension",
    "T2": "comprehension",
    "T3": "single_surface",
    "T4": "single_surface",
    "T5": "cross_surface",
    "T6": "cross_surface",
}

#: Type-matched baseline anchor for each agent task type (paper Section 4,
#: "Baseline-normalised time"). Cross-surface averages four baselines and is the
#: most stable, which is where the hypotheses live.
BASELINE_ANCHOR = {
    "comprehension": ["B6"],
    "single_surface": ["B1"],
    "cross_surface": ["B2", "B3", "B4", "B5"],
}

#: Pre-specified idle rule (paper Section 5, "Timing validity"): within a task,
#: this much visible time containing zero telemetry events is treated as idle
#: and subtracted.
IDLE_GAP_SECONDS = 180

#: Surface actions attributable to the participant, used as the denominator of
#: the delegation ratio alongside executed agent tool calls.
USER_ACTION_EVENTS = [
    "card_created", "card_moved", "card_edited", "card_priority_set",
    "card_assigned", "card_labeled", "card_deleted",
    "message_sent", "message_edited",
    "issue_linked_to_card", "pr_linked_to_card",
    "pr_review_submitted", "pr_merged",
]

CROSS_REFERENCE_EVENTS = [
    "card_referenced_from_chat", "chat_referenced_from_card",
    "pr_referenced_from_card", "pr_referenced_from_chat",
    "card_referenced_from_pr", "issue_referenced_from_card",
]

TLX_SUBSCALES = ["mental", "physical", "temporal", "performance", "effort", "frustration"]

#: Endpoints of the TLX sliders as the app actually renders them (ExitSurvey.svelte
#: and BaselineTlx.svelte both use min=1, max=100). Used to reverse-score
#: `tlx_performance`; see tlx_composite.
TLX_SLIDER_MIN, TLX_SLIDER_MAX = 1, 100

#: The three attitudinal items that form the trust composite (paper Section 4,
#: "Trust, and what we mean by it"). The other trust_* items are reported
#: individually and deliberately excluded -- do not add them here.
TRUST_COMPOSITE_ITEMS = ["trust_reliable", "trust_predictable", "trust_competent"]

SUS_ITEMS = [f"sus_{i:02d}" for i in range(1, 11)]


# --- Task-level table -------------------------------------------------------


def _idle_adjustment(task_events: pd.DataFrame, away_ms: float) -> float:
    """Milliseconds to subtract for visible-but-idle time.

    The client clock already excludes time with the tab hidden, so a long gap
    between events is only idle *visible* time to the extent it exceeds the
    recorded away time. This is an approximation: we do not have a full
    visibility timeline, only the away total. It is deliberately conservative --
    it never subtracts more than the gap actually exceeds away time by -- and
    the primary model is fitted with and without it (paper Section 5).
    """
    if len(task_events) < 2:
        return 0.0
    ts = task_events["ts"].dropna().sort_values()
    if len(ts) < 2:
        return 0.0
    gaps = ts.diff().dt.total_seconds().dropna()
    idle = gaps[gaps >= IDLE_GAP_SECONDS].sum() * 1000.0
    return float(max(0.0, idle - (away_ms or 0.0)))


def task_table(data: StudyData) -> pd.DataFrame:
    """One row per participant x task, with timing, effort and correctness.

    Only `task_completed` produces a row. `task_skipped` is recorded separately
    and reported as an exclusion, never silently coerced to a completion.
    """
    completed = of_type(data.events, "task_completed").copy()
    if completed.empty:
        return pd.DataFrame()

    completed["task_id"] = payload_field(completed, "task_id")
    completed["task_time_ms"] = pd.to_numeric(payload_field(completed, "task_time_ms"))
    completed["wall_ms"] = pd.to_numeric(payload_field(completed, "wall_ms"))
    completed["away_ms"] = pd.to_numeric(payload_field(completed, "away_ms"))
    completed["pause_count"] = pd.to_numeric(payload_field(completed, "pause_count"))
    completed["was_interrupted"] = payload_field(completed, "was_interrupted").astype("boolean")

    rows = []
    for _, ev in completed.iterrows():
        pcode, task_id = ev["participant_code"], ev["task_id"]
        if task_id not in TASK_TYPE:
            continue

        # Events belonging to this task: everything tagged with it, up to and
        # including the completion event.
        in_task = data.events[
            (data.events["participant_code"] == pcode)
            & (data.events["current_task_id"] == task_id)
        ]
        idle_ms = _idle_adjustment(in_task, ev["away_ms"])

        agent_calls = in_task[in_task["event_type"] == "agent_tool_call"]
        # An executed `post_message` call only *drafts* a message -- the agent
        # never posts. The participant reviews, edits and sends it, which lands
        # as its own `message_sent` user action. Counting the draft as an agent
        # action too would put one outcome on both sides of the delegation
        # ratio; the server flags exactly this case with `drafted` so the
        # analysis can tell them apart. Drafts are kept separately below, since
        # "the agent wrote it and I sent it" is real delegation -- it is just not
        # an action the agent completed.
        executed = agent_calls["payload"].map(
            lambda p: bool(p.get("executed")) and not p.get("drafted")
        ).sum()
        drafted = agent_calls["payload"].map(
            lambda p: bool(p.get("executed")) and bool(p.get("drafted"))
        ).sum()
        user_actions = int(in_task["event_type"].isin(USER_ACTION_EVENTS).sum())

        rows.append(
            {
                "participant_code": pcode,
                "agent_condition": ev["agent_condition"],
                "task_id": task_id,
                "task_type": TASK_TYPE[task_id],
                "phase": "baseline" if task_id in BASELINE_TASKS else "agent",
                "task_time_ms": ev["task_time_ms"],
                "task_time_ms_idle_adj": (ev["task_time_ms"] or np.nan) - idle_ms,
                "idle_ms": idle_ms,
                "idle_rule_fired": idle_ms > 0,
                "wall_ms": ev["wall_ms"],
                "away_ms": ev["away_ms"],
                "pause_count": ev["pause_count"],
                "was_interrupted": ev["was_interrupted"],
                "agent_invocations": int((in_task["event_type"] == "agent_invoked").sum()),
                "agent_tool_calls_executed": int(executed),
                "agent_chat_drafts": int(drafted),
                "agent_tool_calls_attempted": int(len(agent_calls)),
                "user_actions": user_actions,
                "cross_references": int(
                    in_task["event_type"].isin(CROSS_REFERENCE_EVENTS).sum()
                ),
                "view_changes": int((in_task["event_type"] == "view_changed").sum()),
                "github_views": int((in_task["event_type"] == "github_view_opened").sum()),
            }
        )

    tt = pd.DataFrame(rows)
    if tt.empty:
        return tt

    tt = _attach_effort(tt, data)
    tt = _attach_baseline(tt)
    tt = _attach_correctness(tt, data)

    tt["log_time"] = np.log(tt["task_time_ms"].where(tt["task_time_ms"] > 0))
    tt["log_baseline"] = np.log(tt["baseline_ms"].where(tt["baseline_ms"] > 0))
    tt["normalised_time"] = tt["task_time_ms"] / tt["baseline_ms"]
    return tt


def _attach_effort(tt: pd.DataFrame, data: StudyData) -> pd.DataFrame:
    ratings = of_type(data.events, "task_rating").copy()
    if ratings.empty:
        tt["effort"] = np.nan
        tt["agent_help"] = np.nan
        return tt
    ratings["task_id"] = payload_field(ratings, "task_id")
    ratings["effort"] = pd.to_numeric(payload_field(ratings, "effort"))
    ratings["agent_help"] = pd.to_numeric(payload_field(ratings, "agent_help"))
    keep = ratings[["participant_code", "task_id", "effort", "agent_help"]].drop_duplicates(
        ["participant_code", "task_id"], keep="last"
    )
    return tt.merge(keep, on=["participant_code", "task_id"], how="left")


def _attach_baseline(tt: pd.DataFrame) -> pd.DataFrame:
    """Attach each agent task's type-matched baseline anchor time."""
    base = tt[tt["phase"] == "baseline"]
    anchors: dict[tuple[str, str], float] = {}
    for pcode, grp in base.groupby("participant_code"):
        by_task = grp.set_index("task_id")["task_time_ms"]
        for task_type, ids in BASELINE_ANCHOR.items():
            vals = [by_task[i] for i in ids if i in by_task.index and pd.notna(by_task[i])]
            anchors[(pcode, task_type)] = float(np.mean(vals)) if vals else np.nan

    tt["baseline_ms"] = [
        anchors.get((p, t), np.nan) if ph == "agent" else np.nan
        for p, t, ph in zip(tt["participant_code"], tt["task_type"], tt["phase"])
    ]
    return tt


def _attach_correctness(tt: pd.DataFrame, data: StudyData) -> pd.DataFrame:
    if data.correctness.empty:
        tt["correctness"] = np.nan
        return tt
    cols = ["participant_code", "task_id", "score"]
    c = data.correctness[cols].rename(columns={"score": "correctness"})
    return tt.merge(c, on=["participant_code", "task_id"], how="left")


# --- Participant-level table ------------------------------------------------


def _survey_wide(data: StudyData) -> pd.DataFrame:
    if data.survey.empty:
        return pd.DataFrame()
    wide = data.survey.pivot_table(
        index="participant_code", columns="item_id", values="value", aggfunc="last"
    )
    return wide.apply(pd.to_numeric, errors="coerce").combine_first(wide)


def _numeric(wide: pd.DataFrame, items: list[str]) -> pd.DataFrame:
    present = [i for i in items if i in wide.columns]
    if not present:
        return pd.DataFrame(index=wide.index)
    return wide[present].apply(pd.to_numeric, errors="coerce")


def tlx_composite(wide: pd.DataFrame, suffix: str = "") -> pd.Series:
    """Raw (unweighted) NASA-TLX: the mean of the six subscales, 0-100.

    `tlx_performance` is authored anchored Failure(low) -> Perfect(high) in
    survey-config.ts -- high means *good* performance, the opposite of the other
    five items, which run low-to-high in the workload direction. It is therefore
    reverse-scored here before compositing, exactly as WELD_ANALYSIS_PLAN.tex
    Sec. "Cognitive load (H3)" pre-specifies (MAX + MIN - raw). The item's own
    comment in survey-config.ts says the same. Verify those anchors before
    changing this: getting the direction wrong silently corrupts every TLX
    number in the paper, in both phases and therefore in the delta as well.
    """
    items = [f"tlx_{s}{suffix}" for s in TLX_SUBSCALES]
    vals = _numeric(wide, items).copy()
    return _reverse_performance(vals, suffix).mean(axis=1, skipna=False) if not vals.empty \
        else pd.Series(dtype=float)


def _reverse_performance(vals: pd.DataFrame, suffix: str = "") -> pd.DataFrame:
    """Flip `tlx_performance` into the workload direction (see tlx_composite)."""
    perf = f"tlx_performance{suffix}"
    if perf in vals.columns:
        vals[perf] = (TLX_SLIDER_MAX + TLX_SLIDER_MIN) - vals[perf]
    return vals


def sus_score(wide: pd.DataFrame) -> pd.Series:
    """Standard SUS scoring on 1-5 Likert items, yielding 0-100."""
    vals = _numeric(wide, SUS_ITEMS)
    if vals.shape[1] != len(SUS_ITEMS):
        return pd.Series(np.nan, index=wide.index)
    odd = vals[[f"sus_{i:02d}" for i in range(1, 11, 2)]] - 1
    even = 5 - vals[[f"sus_{i:02d}" for i in range(2, 11, 2)]]
    return (odd.sum(axis=1) + even.sum(axis=1)) * 2.5


def cronbach_alpha(frame: pd.DataFrame) -> float:
    """Cronbach's alpha for a set of items (rows = respondents)."""
    df = frame.dropna()
    k = df.shape[1]
    if k < 2 or len(df) < 3:
        return float("nan")
    item_var = df.var(axis=0, ddof=1).sum()
    total_var = df.sum(axis=1).var(ddof=1)
    if total_var == 0:
        return float("nan")
    return float((k / (k - 1)) * (1 - item_var / total_var))


def participant_table(data: StudyData, tasks: pd.DataFrame) -> pd.DataFrame:
    """One row per participant: condition, scales, and aggregate behaviour."""
    p = data.participants.set_index("participant_code")
    wide = _survey_wide(data)
    out = pd.DataFrame(index=p.index)
    out["agent_condition"] = p["agent_condition"]
    out["status"] = p["status"]

    if not wide.empty:
        out["tlx_baseline"] = tlx_composite(wide, "_baseline")
        out["tlx_agent"] = tlx_composite(wide)
        out["tlx_delta"] = out["tlx_agent"] - out["tlx_baseline"]
        for s in TLX_SUBSCALES:
            # Reverse-scored on both sides for `performance`, so every subscale
            # delta reads in the same direction as the composite: positive =
            # more workload under the agent.
            b = _reverse_performance(_numeric(wide, [f"tlx_{s}_baseline"]), "_baseline")
            a = _reverse_performance(_numeric(wide, [f"tlx_{s}"]))
            if not b.empty and not a.empty:
                out[f"tlx_{s}_delta"] = a.iloc[:, 0] - b.iloc[:, 0]
        out["sus"] = sus_score(wide)

        trust = _numeric(wide, TRUST_COMPOSITE_ITEMS)
        out["trust_composite"] = trust.mean(axis=1) if not trust.empty else np.nan
        for item in [
            "trust_rely", "trust_again", "trust_context", "trust_verify",
            "trust_speed_perceived", "int_switch_perceived",
        ]:
            col = _numeric(wide, [item])
            if not col.empty:
                out[item] = col.iloc[:, 0]

    if tasks is not None and not tasks.empty:
        agent = tasks[tasks["phase"] == "agent"]
        grp = agent.groupby("participant_code")
        out["agent_invocations"] = grp["agent_invocations"].sum()
        out["tool_calls_executed"] = grp["agent_tool_calls_executed"].sum()
        out["chat_drafts"] = grp["agent_chat_drafts"].sum()
        out["user_actions"] = grp["user_actions"].sum()
        # Numerator = actions the agent actually completed. Chat drafts are
        # excluded (see task_table): the participant is the one who posted, so
        # the send is already counted on the user side. `chat_drafts` is carried
        # alongside so the drafting channel of delegation stays visible, and so
        # a reader can recompute the ratio the other way if they disagree.
        denom = out["tool_calls_executed"] + out["user_actions"]
        out["delegation_ratio"] = (out["tool_calls_executed"] / denom).where(denom > 0)

        cross = agent[agent["task_type"] == "cross_surface"]
        cg = cross.groupby("participant_code")
        out["mean_normalised_time_cross"] = cg["normalised_time"].mean()
        out["mean_correctness_cross"] = cg["correctness"].mean()
        out["mean_normalised_time_all"] = grp["normalised_time"].mean()
        out["mean_correctness_all"] = grp["correctness"].mean()

        active_min = grp["task_time_ms"].sum() / 60000.0
        out["view_switch_rate"] = (grp["view_changes"].sum() / active_min).replace(
            [np.inf, -np.inf], np.nan
        )

    return out.reset_index()


def trust_alpha(data: StudyData) -> float:
    """Cronbach's alpha for the three-item trust composite.

    The plan pre-commits: if this is below 0.70, report the items individually
    and treat composite-based analyses as descriptive.
    """
    wide = _survey_wide(data)
    if wide.empty:
        return float("nan")
    return cronbach_alpha(_numeric(wide, TRUST_COMPOSITE_ITEMS))
