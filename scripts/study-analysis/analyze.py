#!/usr/bin/env python3
"""Generate every figure and table in WELD_ANALYSIS_PLAN.tex from the study export.

Consumes the three CSVs the backend already serves
(``GET /api/v1/study/export?dataset=…&token=$STUDY_EXPORT_TOKEN``):
``participants``, ``events`` (with a JSON ``payload`` column), ``survey``.

    # from downloaded CSVs (participants.csv, events.csv, survey.csv in ./exports)
    python analyze.py --data ./exports

    # or fetch straight from a running server
    python analyze.py --url https://your-host --token "$STUDY_EXPORT_TOKEN"

Outputs to ``figures/`` (vector PDF for LaTeX + PNG preview):
    fig1_time.*            Normalized time by condition x task type   (H2, headline)
    fig2_correctness.*     Correctness by condition x task type       (hand-coded input)
    fig3_perception.*      Perceived vs. measured speedup             (H5)
    fig4_delegation.*      Division of labor by condition             (RQ2/H4)
    fig_h3_tlx.*           TLX delta (agent - baseline) by condition  (H3)
plus tables (CSV) and stats.txt (Spearman, group means/SDs).

Fig. 2 correctness is NOT in telemetry (hand-coded per the plan): the script writes
``correctness_template.csv`` for you to fill; until then Fig. 2 is skipped.

Run ``python analyze.py --selftest`` to sanity-check the derived-quantity math.
"""

from __future__ import annotations

import argparse
import io
import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pandas as pd
from scipy import stats

# ── Constants tied to the study design ──────────────────────────────────────
CONDITIONS = ["C1", "C2", "C3"]
TASK_TYPES = ["comprehension", "single_surface", "cross_surface"]
TASK_TYPE_LABEL = {
    "comprehension": "Comprehension",
    "single_surface": "Single-surface",
    "cross_surface": "Cross-surface",
}
# Colorblind-safe (Okabe-Ito) per condition, kept consistent across every figure.
CONDITION_COLOR = {"C1": "#0072B2", "C2": "#E69F00", "C3": "#009E73"}
AGENT_TASKS = ["T1", "T2", "T3", "T4", "T5", "T6"]

# Correctness is hand-scored 0–3 (WELD_AGENT_TASK_SCRIPTS.md §correctness), NOT in
# telemetry. Two independent coders score the FINAL STATE from `task_final_state`;
# report Cohen's kappa on a 20% sample. These anchors (what a 3 requires) are
# printed onto the coding sheet so scoring is objective.
CORRECTNESS_SCALE = "0 = none, 1 = wrong, 2 = partial, 3 = fully correct"
CRITERIA = {
    "T1": "User acted on a genuinely top-priority item consistent with chat context.",
    "T2": "Accurate 2–3 sentence status summary posted in #general, no major omissions.",
    "T3": "A coherent, accurate status message about the vaccination reminders posted to #pettracker.",
    "T4": "All three new-issue cards have priorities set; ordering defensible (data-loss/sync > timezone > typo).",
    "T5": "card_login_crash is in In Progress.",
    "T6": "Correct PR linked to the correct card AND a note posted in #pettracker (2 if one of the two; 3 if both).",
    # Baseline anchors kept for reference; Fig. 2 scores the agent tasks only.
    "B1": "card_feeding_schedule_ui is in In Progress.",
    "B2": "Status reply in #general correctly states the card's actual column.",
    "B3": "card_vet_directory priority set to High.",
    "B4": "Correct PR number (22) posted in #general.",
    "B5": "card_db_decision description records the actual decision (SQLite).",
    "B6": "#general reply correctly states what CI does (tests/lint on every PR).",
}

# The six NASA-TLX subscales. `tlx_performance` is anchored Failure->Perfect
# (high = good), the opposite of the other five, so it is reverse-scored before
# compositing. Slider range is 1..100 -> reverse = 101 - raw. Keep in sync with
# apps/web/src/lib/survey-config.ts and WELD_EXIT_SURVEY.md.
TLX_SUBSCALES = ["tlx_mental", "tlx_physical", "tlx_temporal",
                 "tlx_performance", "tlx_effort", "tlx_frustration"]
TLX_REVERSED = {"tlx_performance"}
TLX_MIN, TLX_MAX = 1, 100

FIG_DIR_DEFAULT = "figures"


# ── Loading ─────────────────────────────────────────────────────────────────
def load_from_dir(data_dir: Path) -> dict[str, pd.DataFrame]:
    def read(name: str) -> pd.DataFrame:
        path = data_dir / f"{name}.csv"
        if not path.exists():
            sys.exit(f"error: {path} not found (expected participants/events/survey CSVs)")
        return pd.read_csv(path)

    return {"participants": read("participants"),
            "events": read("events"),
            "survey": read("survey")}


def load_from_url(base_url: str, token: str) -> dict[str, pd.DataFrame]:
    import requests  # local import so file-mode users need no `requests`

    out = {}
    for name in ("participants", "events", "survey"):
        r = requests.get(
            f"{base_url.rstrip('/')}/api/v1/study/export",
            params={"dataset": name, "token": token},
            timeout=30,
        )
        if r.status_code != 200:
            sys.exit(f"error: export fetch for '{name}' returned {r.status_code}: {r.text[:200]}")
        out[name] = pd.read_csv(io.StringIO(r.text))
    return out


# ── Tidy frames ─────────────────────────────────────────────────────────────
def parse_payloads(events: pd.DataFrame) -> pd.DataFrame:
    """Expand the JSON `payload` column into a dict per row (`p`)."""
    events = events.copy()
    events["p"] = events["payload"].apply(
        lambda s: json.loads(s) if isinstance(s, str) and s.strip() else {}
    )
    return events


def task_phase(task_id: str) -> str:
    return "baseline" if str(task_id).startswith("B") else "agent"


def build_tasks(events: pd.DataFrame) -> pd.DataFrame:
    """One row per completed task, with active time and interruption flags."""
    rows = []
    for _, e in events[events["event_type"] == "task_completed"].iterrows():
        p = e["p"]
        rows.append({
            "participant": e["participant_code"],
            "condition": e["agent_condition"],
            "task_id": p.get("task_id"),
            "task_type": p.get("task_type"),
            "phase": task_phase(p.get("task_id", "")),
            "active_ms": p.get("task_time_ms"),
            "wall_ms": p.get("wall_ms"),
            "away_ms": p.get("away_ms"),
            "pause_count": p.get("pause_count"),
            "was_interrupted": p.get("was_interrupted"),
        })
    df = pd.DataFrame(rows)
    if not df.empty:
        df["active_ms"] = pd.to_numeric(df["active_ms"], errors="coerce")
    return df


def add_normalized_time(tasks: pd.DataFrame) -> pd.DataFrame:
    """Baseline-normalized ratio: each agent task's active time / that participant's
    mean baseline active time for the SAME task type (the quantity the plan plots)."""
    if tasks.empty:
        return tasks
    base = (tasks[tasks["phase"] == "baseline"]
            .groupby(["participant", "task_type"])["active_ms"].mean()
            .rename("baseline_ms"))
    tasks = tasks.merge(base, on=["participant", "task_type"], how="left")
    tasks["norm_time"] = tasks["active_ms"] / tasks["baseline_ms"]
    return tasks


def build_context(events: pd.DataFrame) -> pd.DataFrame:
    """One row per agent turn (agent_context_sent) with which sources were included."""
    rows = []
    for _, e in events[events["event_type"] == "agent_context_sent"].iterrows():
        p = e["p"]
        rows.append({
            "participant": e["participant_code"],
            "condition": p.get("condition") or e["agent_condition"],
            "has_board": ("board" in p),
            "has_github": ("github" in p),
        })
    return pd.DataFrame(rows)


def build_toolcalls(events: pd.DataFrame) -> pd.DataFrame:
    rows = []
    for _, e in events[events["event_type"] == "agent_tool_call"].iterrows():
        p = e["p"]
        rows.append({
            "participant": e["participant_code"],
            "condition": e["agent_condition"],
            "tool": p.get("tool"),
            "executed": bool(p.get("executed")),
            "drafted": bool(p.get("drafted")),
        })
    return pd.DataFrame(rows)


def survey_wide(survey: pd.DataFrame) -> pd.DataFrame:
    """participant x item_id -> numeric value, plus the condition."""
    if survey.empty:
        return pd.DataFrame()
    survey = survey.copy()
    survey["num"] = pd.to_numeric(survey["value"], errors="coerce")
    wide = survey.pivot_table(index="participant_code", columns="item_id",
                              values="num", aggfunc="first")
    cond = survey.groupby("participant_code")["agent_condition"].first()
    wide["condition"] = cond
    return wide.reset_index().rename(columns={"participant_code": "participant"})


def tlx_composite(wide: pd.DataFrame, suffix: str) -> pd.Series:
    """Mean of the six subscales for one phase, reverse-scoring Performance so
    higher = more workload. `suffix` is '' (agent/exit) or '_baseline'."""
    cols = []
    for base in TLX_SUBSCALES:
        col = base + suffix
        if col not in wide.columns:
            continue
        vals = wide[col]
        if base in TLX_REVERSED:
            vals = (TLX_MAX + TLX_MIN) - vals
        cols.append(vals)
    if not cols:
        return pd.Series(dtype=float)
    return pd.concat(cols, axis=1).mean(axis=1)


# ── Plot helpers ────────────────────────────────────────────────────────────
def _style():
    plt.rcParams.update({
        "figure.dpi": 120, "savefig.dpi": 300, "font.size": 11,
        "axes.spines.top": False, "axes.spines.right": False,
        "axes.grid": True, "grid.alpha": 0.25, "grid.linewidth": 0.6,
    })


def save_fig(fig, out_dir: Path, name: str):
    out_dir.mkdir(parents=True, exist_ok=True)
    for ext in ("pdf", "png"):
        fig.savefig(out_dir / f"{name}.{ext}", bbox_inches="tight")
    plt.close(fig)
    print(f"  wrote {name}.pdf / .png")


def _grouped_bars_with_dots(ax, df, value_col, ylabel, title):
    """Grouped bars over task_type (x) x condition (series) with participant dots."""
    n_ct = len(TASK_TYPES)
    width = 0.25
    for ci, cond in enumerate(CONDITIONS):
        xs, heights, dots_x, dots_y = [], [], [], []
        for ti, tt in enumerate(TASK_TYPES):
            sub = df[(df["condition"] == cond) & (df["task_type"] == tt)][value_col].dropna()
            x = ti + (ci - 1) * width
            xs.append(x)
            heights.append(sub.mean() if len(sub) else 0.0)
            dots_x += [x] * len(sub)
            dots_y += list(sub)
        ax.bar(xs, heights, width=width, color=CONDITION_COLOR[cond],
               label=cond, alpha=0.85, edgecolor="white", linewidth=0.5)
        ax.scatter(dots_x, dots_y, s=18, color="#333", alpha=0.55, zorder=3,
                   linewidths=0)
    ax.set_xticks(range(n_ct))
    ax.set_xticklabels([TASK_TYPE_LABEL[t] for t in TASK_TYPES])
    ax.set_ylabel(ylabel)
    ax.set_title(title, fontsize=12, loc="left", weight="bold")
    ax.legend(title="Condition", frameon=False, ncols=3, loc="upper left")


# ── Figures ─────────────────────────────────────────────────────────────────
def fig1_time(tasks: pd.DataFrame, out_dir: Path):
    agent = tasks[(tasks["phase"] == "agent") & tasks["norm_time"].notna()]
    if agent.empty:
        print("  fig1: no agent task times yet — skipped")
        return
    fig, ax = plt.subplots(figsize=(7, 4.2))
    _grouped_bars_with_dots(ax, agent, "norm_time",
                            "Normalized time (× matched baseline)",
                            "Fig. 1 — Normalized time by condition × task type")
    ax.axhline(1.0, color="#888", lw=0.8, ls="--")
    save_fig(fig, out_dir, "fig1_time")


def fig2_correctness(correctness: pd.DataFrame | None, out_dir: Path):
    if correctness is None or correctness.empty:
        print("  fig2: score correctness_template.csv (0–3, see coding_sheet.md) — skipped")
        return
    df = correctness.copy()
    df["score"] = pd.to_numeric(df["score"], errors="coerce")
    fig, ax = plt.subplots(figsize=(7, 4.2))
    _grouped_bars_with_dots(ax, df, "score", "Correctness (0–3)",
                            "Fig. 2 — Correctness by condition × task type")
    ax.set_ylim(0, 3.2)
    save_fig(fig, out_dir, "fig2_correctness")


def fig3_perception(tasks: pd.DataFrame, wide: pd.DataFrame, out_dir: Path):
    if wide.empty or "trust_speed_perceived" not in wide.columns:
        print("  fig3: no trust_speed_perceived responses yet — skipped")
        return
    agent = tasks[(tasks["phase"] == "agent") & tasks["norm_time"].notna()]
    measured = agent.groupby("participant")["norm_time"].mean().rename("measured")
    df = wide[["participant", "condition", "trust_speed_perceived"]].merge(
        measured, on="participant", how="inner").dropna()
    if df.empty:
        print("  fig3: no participant has both measured time and perception — skipped")
        return
    fig, ax = plt.subplots(figsize=(5.4, 5))
    for cond in CONDITIONS:
        sub = df[df["condition"] == cond]
        ax.scatter(sub["measured"], sub["trust_speed_perceived"],
                   color=CONDITION_COLOR[cond], label=cond, s=40, alpha=0.8)
    ax.set_xlabel("Measured normalized time (lower = faster)")
    ax.set_ylabel("Perceived speedup (trust_speed_perceived)")
    ax.set_title("Fig. 3 — Perception vs. behavior", fontsize=12, loc="left", weight="bold")
    ax.legend(title="Condition", frameon=False)
    save_fig(fig, out_dir, "fig3_perception")


def fig4_delegation(toolcalls: pd.DataFrame, out_dir: Path):
    if toolcalls.empty:
        print("  fig4: no agent tool calls yet — skipped")
        return
    executed = toolcalls[toolcalls["executed"] & ~toolcalls["drafted"]]
    per = (executed.groupby("condition")["participant"].value_counts()
           .rename("n").reset_index())
    fig, ax = plt.subplots(figsize=(6, 4.2))
    for ci, cond in enumerate(CONDITIONS):
        vals = per[per["condition"] == cond]["n"]
        ax.bar(ci, vals.mean() if len(vals) else 0, width=0.6,
               color=CONDITION_COLOR[cond], alpha=0.85)
        ax.scatter([ci] * len(vals), vals, color="#333", s=20, alpha=0.6, zorder=3)
    ax.set_xticks(range(len(CONDITIONS)))
    ax.set_xticklabels(CONDITIONS)
    ax.set_ylabel("Executed agent actions per participant")
    ax.set_title("Fig. 4 — Division of labor by condition", fontsize=12, loc="left", weight="bold")
    save_fig(fig, out_dir, "fig4_delegation")


def fig_h3_tlx(wide: pd.DataFrame, out_dir: Path):
    if wide.empty:
        print("  fig_h3: no survey data yet — skipped")
        return
    agent = tlx_composite(wide, "")
    base = tlx_composite(wide, "_baseline")
    if agent.empty or base.empty:
        print("  fig_h3: TLX (agent and/or baseline) not present — skipped")
        return
    df = pd.DataFrame({"condition": wide["condition"],
                       "delta": agent - base}).dropna()
    if df.empty:
        print("  fig_h3: no participant with both phases — skipped")
        return
    fig, ax = plt.subplots(figsize=(6, 4.2))
    for ci, cond in enumerate(CONDITIONS):
        vals = df[df["condition"] == cond]["delta"]
        ax.bar(ci, vals.mean() if len(vals) else 0, width=0.6,
               color=CONDITION_COLOR[cond], alpha=0.85)
        ax.scatter([ci] * len(vals), vals, color="#333", s=20, alpha=0.6, zorder=3)
    ax.axhline(0, color="#888", lw=0.8)
    ax.set_xticks(range(len(CONDITIONS)))
    ax.set_xticklabels(CONDITIONS)
    ax.set_ylabel("TLX delta (agent − baseline)")
    ax.set_title("Fig. H3 — Cognitive-load change by condition", fontsize=12, loc="left", weight="bold")
    save_fig(fig, out_dir, "fig_h3_tlx")


# ── Tables + stats ──────────────────────────────────────────────────────────
def manipulation_check(context: pd.DataFrame, out_dir: Path):
    """Prove scoping held: C1 turns carry no board/GitHub, C2 no GitHub."""
    rows = []
    for cond in CONDITIONS:
        sub = context[context["condition"] == cond]
        n = len(sub)
        rows.append({
            "condition": cond, "agent_turns": n,
            "pct_no_board": round(100 * (~sub["has_board"]).mean(), 1) if n else None,
            "pct_no_github": round(100 * (~sub["has_github"]).mean(), 1) if n else None,
        })
    df = pd.DataFrame(rows)
    df.to_csv(out_dir / "table_manipulation_check.csv", index=False)
    print("  wrote table_manipulation_check.csv")
    return df


def consort_flow(participants: pd.DataFrame, events: pd.DataFrame, out_dir: Path):
    by_status = participants["status"].value_counts().rename_axis("status").reset_index(name="n")
    abandoned = int((events["event_type"] == "session_abandoned").sum())
    by_status.to_csv(out_dir / "table_consort_flow.csv", index=False)
    with open(out_dir / "table_consort_flow.csv", "a") as f:
        f.write(f"session_abandoned_events,{abandoned}\n")
    print("  wrote table_consort_flow.csv")
    return by_status


def write_stats(tasks, wide, out_dir: Path):
    lines = ["WELD study — summary statistics\n" + "=" * 34 + "\n"]

    agent = tasks[(tasks["phase"] == "agent") & tasks["norm_time"].notna()]
    if not agent.empty:
        lines.append("Normalized time by condition × task type (mean ± sd, n):")
        g = agent.groupby(["condition", "task_type"])["norm_time"].agg(["mean", "std", "count"])
        lines.append(g.round(3).to_string())
        lines.append("")

    # H5: Spearman perceived vs measured, across participants.
    if not wide.empty and "trust_speed_perceived" in wide.columns and not agent.empty:
        measured = agent.groupby("participant")["norm_time"].mean().rename("measured")
        df = wide[["participant", "trust_speed_perceived"]].merge(measured, on="participant").dropna()
        if len(df) >= 3:
            rho, p = stats.spearmanr(df["measured"], df["trust_speed_perceived"])
            lines.append(f"H5 Spearman(perceived, measured normalized time): "
                         f"rho={rho:.3f}, p={p:.3f}, n={len(df)}")
        else:
            lines.append(f"H5 Spearman: need >=3 paired participants (have {len(df)})")
        lines.append("")

    if not wide.empty:
        a, b = tlx_composite(wide, ""), tlx_composite(wide, "_baseline")
        if not a.empty and not b.empty:
            delta = pd.DataFrame({"condition": wide["condition"], "delta": a - b}).dropna()
            lines.append("TLX delta (agent − baseline) by condition (mean ± sd, n):")
            lines.append(delta.groupby("condition")["delta"].agg(["mean", "std", "count"]).round(2).to_string())
            lines.append("")

    (out_dir / "stats.txt").write_text("\n".join(lines))
    print("  wrote stats.txt")


def build_snapshots(events: pd.DataFrame) -> dict[tuple[str, str], dict]:
    """(participant, task_id) -> latest task_final_state payload (board + chat)."""
    snaps: dict[tuple[str, str], dict] = {}
    for _, e in events[events["event_type"] == "task_final_state"].iterrows():
        tid = e.get("current_task_id")
        if not isinstance(tid, str) or not tid:
            continue
        snaps[(e["participant_code"], tid)] = e["p"]  # later rows win (latest)
    return snaps


def _render_evidence(snap: dict | None, participant_uid: str | None) -> str:
    """The board state + the messages the participant authored — everything a
    coder needs to score correctness from the snapshot alone."""
    if not snap:
        return "_(no task_final_state snapshot captured for this task)_"
    lines = ["**Board:**"]
    for c in (snap.get("board") or {}).get("tasks") or []:
        pr = f", PR#{c['github_pr_id']}" if c.get("github_pr_id") else ""
        lines.append(f"- \"{c.get('title')}\" [col: {c.get('column')}, "
                     f"priority: {c.get('priority')}{pr}]")
    authored = [(ch.get("channel"), m.get("content"))
                for ch in (snap.get("chat") or [])
                for m in (ch.get("messages") or [])
                if participant_uid and m.get("sender_id") == participant_uid]
    lines.append("")
    if authored:
        lines.append("**Messages the participant posted:**")
        lines += [f"- #{ch}: {content}" for ch, content in authored]
    else:
        lines.append("**Messages the participant posted:** _(none in snapshot)_")
    return "\n".join(lines)


def write_coding_sheet(tasks: pd.DataFrame, snapshots: dict, participants: pd.DataFrame,
                       template_path: Path):
    """Emit (1) a fill-in scoring CSV and (2) a Markdown sheet pairing each agent
    task with its correctness anchor and the participant's final-state evidence."""
    agent = (tasks[tasks["phase"] == "agent"][["participant", "condition", "task_id", "task_type"]]
             .drop_duplicates().sort_values(["participant", "task_id"]))
    if agent.empty:
        print("  coding sheet: no agent tasks completed yet — skipped")
        return
    uid = participants.set_index("participant_code")["user_id"].astype(str).to_dict()

    if not template_path.exists():  # never clobber a partially-coded sheet
        tmpl = agent.copy()
        tmpl["criterion"] = tmpl["task_id"].map(CRITERIA)
        tmpl["score"] = ""  # coder fills 0..3
        tmpl.to_csv(template_path, index=False)
        print(f"  wrote {template_path.name} (blank 'score' 0–3: {CORRECTNESS_SCALE})")

    md = [f"# Correctness coding sheet\n",
          f"Score each row **0–3** in `{template_path.name}` ({CORRECTNESS_SCALE}). "
          "Two independent coders; report Cohen's kappa on a 20% sample. Score the "
          "*final state*, regardless of whether the agent or the user produced it.\n"]
    for participant, sub in agent.groupby("participant"):
        md.append(f"\n## {participant}  ({sub['condition'].iloc[0]})")
        for _, r in sub.iterrows():
            md.append(f"\n### {r['task_id']} — {r['task_type']}")
            md.append(f"**A score of 3 requires:** {CRITERIA.get(r['task_id'], '—')}\n")
            md.append(_render_evidence(snapshots.get((participant, r["task_id"])),
                                       uid.get(participant)))
    (template_path.parent / "coding_sheet.md").write_text("\n".join(md))
    print("  wrote coding_sheet.md (criterion + final-state evidence per task)")


# ── Orchestration ───────────────────────────────────────────────────────────
def run(data: dict[str, pd.DataFrame], out_dir: Path, correctness_path: Path):
    _style()
    events = parse_payloads(data["events"])
    tasks = add_normalized_time(build_tasks(events))
    context = build_context(events)
    toolcalls = build_toolcalls(events)
    snapshots = build_snapshots(events)
    wide = survey_wide(data["survey"])

    write_coding_sheet(tasks, snapshots, data["participants"], correctness_path)
    correctness = pd.read_csv(correctness_path) if correctness_path.exists() else None
    if correctness is not None and not pd.to_numeric(
        correctness["score"], errors="coerce").notna().any():
        correctness = None  # sheet exists but not yet scored (all blank/NaN)

    print("figures + tables:")
    fig1_time(tasks, out_dir)
    fig2_correctness(correctness, out_dir)
    fig3_perception(tasks, wide, out_dir)
    fig4_delegation(toolcalls, out_dir)
    fig_h3_tlx(wide, out_dir)
    manipulation_check(context, out_dir)
    consort_flow(data["participants"], events, out_dir)
    write_stats(tasks, wide, out_dir)
    print(f"done → {out_dir}/")


def selftest():
    """Sanity-check the two non-obvious computations: baseline normalization and
    the reverse-scored TLX composite."""
    tasks = pd.DataFrame([
        {"participant": "P1", "condition": "C3", "task_id": "B1", "task_type": "comprehension",
         "phase": "baseline", "active_ms": 100_000},
        {"participant": "P1", "condition": "C3", "task_id": "T1", "task_type": "comprehension",
         "phase": "agent", "active_ms": 50_000},
    ])
    tasks = add_normalized_time(tasks)
    ratio = tasks.loc[tasks["task_id"] == "T1", "norm_time"].iloc[0]
    assert abs(ratio - 0.5) < 1e-9, ratio  # 50k / 100k

    wide = pd.DataFrame([{
        "participant": "P1", "condition": "C3",
        "tlx_mental": 80, "tlx_physical": 80, "tlx_temporal": 80,
        "tlx_performance": 90, "tlx_effort": 80, "tlx_frustration": 80,
    }])
    comp = tlx_composite(wide, "").iloc[0]
    # Performance 90 -> reversed 101-90=11; mean(80,80,80,11,80,80)=68.5
    assert abs(comp - 68.5) < 1e-9, comp
    print("selftest OK: normalization and reverse-scored TLX composite correct")


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--data", type=Path, help="dir with participants/events/survey CSVs")
    ap.add_argument("--url", help="base URL of a running server (with --token)")
    ap.add_argument("--token", help="STUDY_EXPORT_TOKEN for --url fetch")
    ap.add_argument("--out", type=Path, default=Path(FIG_DIR_DEFAULT), help="output dir")
    ap.add_argument("--correctness", type=Path, default=Path("correctness_template.csv"),
                    help="hand-coded correctness CSV for Fig. 2")
    ap.add_argument("--selftest", action="store_true", help="run math self-checks and exit")
    args = ap.parse_args()

    if args.selftest:
        selftest()
        return
    if args.url:
        if not args.token:
            sys.exit("error: --url requires --token")
        data = load_from_url(args.url, args.token)
    elif args.data:
        data = load_from_dir(args.data)
    else:
        sys.exit("error: pass --data DIR or --url URL --token TOKEN (or --selftest)")

    run(data, args.out, args.correctness)


if __name__ == "__main__":
    main()
