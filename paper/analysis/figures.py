"""
The paper's figures.

Design decisions, so they are not re-litigated every time someone regenerates:

* **Every participant point is drawn.** At 7-10 per cell, a bar of means hides
  the thing a reader most needs to judge -- the spread. Condition means sit on
  top of the raw points with 95% CIs.
* **Colour is Okabe-Ito**, validated for colour-vision deficiency (worst adjacent
  pair dE 11.4 protan, 24.2 normal). Hues are assigned to C1/C2/C3 in fixed
  order and never cycled or re-assigned when a series is filtered out.
* **Marker shape duplicates colour** (circle / square / triangle) so the figures
  survive grayscale printing, which is the paper equivalent of a texture fill.
* **One y-axis, never two.** Time and correctness are separate figures.
* Grid and spines are recessive; all text is ink-coloured, never series-coloured.
"""

from __future__ import annotations

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
from scipy import stats

# Fixed categorical assignment. Do not cycle, do not reorder.
CONDITION_COLOR = {"C1": "#0072B2", "C2": "#E69F00", "C3": "#009E73"}
CONDITION_MARKER = {"C1": "o", "C2": "s", "C3": "^"}
CONDITION_LABEL = {
    "C1": "C1 — chat only",
    "C2": "C2 — + board",
    "C3": "C3 — + GitHub",
}
CONDITIONS = ["C1", "C2", "C3"]

TASK_TYPE_ORDER = ["comprehension", "single_surface", "cross_surface"]
TASK_TYPE_LABEL = {
    "comprehension": "Comprehension",
    "single_surface": "Single-surface",
    "cross_surface": "Cross-surface",
}

INK = "#1a1a1a"
INK_MUTED = "#6b6b6b"
GRID = "#e0e0e0"


def _style(ax: plt.Axes) -> None:
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.spines["left"].set_color(GRID)
    ax.spines["bottom"].set_color(GRID)
    ax.tick_params(colors=INK_MUTED, labelsize=9, length=3)
    ax.yaxis.grid(True, color=GRID, linewidth=0.8)
    ax.set_axisbelow(True)
    for lbl in list(ax.get_xticklabels()) + list(ax.get_yticklabels()):
        lbl.set_color(INK)


def _mean_ci(values: np.ndarray) -> tuple[float, float]:
    """Mean and half-width of its 95% CI (t-based; NaN when n < 2)."""
    v = np.asarray(values, dtype=float)
    v = v[~np.isnan(v)]
    if len(v) < 2:
        return (float(v.mean()) if len(v) else np.nan, np.nan)
    se = v.std(ddof=1) / np.sqrt(len(v))
    return float(v.mean()), float(stats.t.ppf(0.975, len(v) - 1) * se)


def _interaction_plot(
    tasks: pd.DataFrame, value_col: str, ylabel: str, title: str,
    reference: float | None, out: Path,
) -> None:
    agent = tasks[tasks["phase"] == "agent"].dropna(subset=[value_col])
    fig, ax = plt.subplots(figsize=(6.4, 3.9))
    x = np.arange(len(TASK_TYPE_ORDER))

    if reference is not None:
        ax.axhline(reference, color=INK_MUTED, linewidth=1, linestyle=(0, (4, 3)), zorder=1)
        # Sit the label just above its own line, never on it.
        ax.annotate("same as own baseline", xy=(len(TASK_TYPE_ORDER) - 0.55, reference),
                    xytext=(0, 4), textcoords="offset points", fontsize=8,
                    color=INK_MUTED, va="bottom", ha="right")

    for i, cond in enumerate(CONDITIONS):
        sub = agent[agent["agent_condition"] == cond]
        if sub.empty:
            continue
        offset = (i - 1) * 0.13
        means, errs = [], []
        for j, ttype in enumerate(TASK_TYPE_ORDER):
            vals = sub.loc[sub["task_type"] == ttype, value_col].values
            m, e = _mean_ci(vals)
            means.append(m)
            errs.append(e if not np.isnan(e) else 0.0)
            # Raw participant points, jittered, behind the summary.
            if len(vals):
                jitter = (np.random.default_rng(j * 31 + i).random(len(vals)) - 0.5) * 0.09
                ax.scatter(np.full(len(vals), j + offset) + jitter, vals, s=13,
                           color=CONDITION_COLOR[cond], alpha=0.28, linewidths=0, zorder=2)
        ax.errorbar(
            x + offset, means, yerr=errs, color=CONDITION_COLOR[cond],
            marker=CONDITION_MARKER[cond], markersize=7, linewidth=2, capsize=3,
            markeredgecolor="white", markeredgewidth=1.1, zorder=3,
            label=CONDITION_LABEL[cond],
        )

    ax.set_xticks(x)
    ax.set_xticklabels([TASK_TYPE_LABEL[t] for t in TASK_TYPE_ORDER])
    ax.set_ylabel(ylabel, color=INK, fontsize=10)
    ax.set_title(title, color=INK, fontsize=11, loc="left", pad=10)
    ax.set_xlim(-0.5, len(TASK_TYPE_ORDER) - 0.5)
    _style(ax)
    ax.legend(frameon=False, fontsize=9, labelcolor=INK, loc="best")
    fig.tight_layout()
    fig.savefig(out, dpi=200)
    plt.close(fig)


def fig_normalised_time(tasks: pd.DataFrame, out: Path) -> None:
    """Figure 2 -- the H2 interaction: does the effect concentrate cross-surface?"""
    _interaction_plot(
        tasks, "normalised_time",
        "Task time ÷ own baseline",
        "Baseline-normalised task time by condition and task type",
        reference=1.0, out=out,
    )


def fig_correctness(tasks: pd.DataFrame, out: Path) -> None:
    """Figure 3 -- correctness on the same axes, reported beside every timing result."""
    if tasks.empty or tasks["correctness"].isna().all():
        return
    _interaction_plot(
        tasks, "correctness", "Correctness (0–3)",
        "Correctness by condition and task type", reference=None, out=out,
    )


def fig_delegation(participants: pd.DataFrame, out: Path) -> None:
    """Figure 4 -- delegation ratio by condition, every participant shown."""
    if "delegation_ratio" not in participants.columns:
        return
    d = participants.dropna(subset=["delegation_ratio"])
    if d.empty:
        return
    fig, ax = plt.subplots(figsize=(5.0, 3.6))
    for i, cond in enumerate(CONDITIONS):
        vals = d.loc[d["agent_condition"] == cond, "delegation_ratio"].values
        if not len(vals):
            continue
        jitter = (np.random.default_rng(i).random(len(vals)) - 0.5) * 0.16
        ax.scatter(np.full(len(vals), i) + jitter, vals, s=30,
                   color=CONDITION_COLOR[cond], marker=CONDITION_MARKER[cond],
                   alpha=0.55, linewidths=0, zorder=2)
        m, e = _mean_ci(vals)
        ax.errorbar([i], [m], yerr=[e if not np.isnan(e) else 0.0],
                    color=CONDITION_COLOR[cond], marker="_", markersize=26,
                    markeredgewidth=2.5, linewidth=2, capsize=4, zorder=3)
        ax.annotate(f"{m:.2f}", xy=(i + 0.22, m), fontsize=9, color=INK, va="center")

    ax.set_xticks(range(len(CONDITIONS)))
    ax.set_xticklabels([CONDITION_LABEL[c] for c in CONDITIONS], fontsize=9)
    ax.set_ylabel("Agent actions ÷ all actions", color=INK, fontsize=10)
    ax.set_title("Delegation to the agent, by condition", color=INK, fontsize=11,
                 loc="left", pad=10)
    ax.set_ylim(-0.03, 1.03)
    ax.set_xlim(-0.5, len(CONDITIONS) - 0.4)
    _style(ax)
    fig.tight_layout()
    fig.savefig(out, dpi=200)
    plt.close(fig)


def fig_perception_gap(participants: pd.DataFrame, out: Path) -> None:
    """Figure 5 -- the RQ3 scatter: trust against measured performance."""
    needed = {"trust_composite", "mean_normalised_time_cross"}
    if not needed.issubset(participants.columns):
        return
    d = participants.dropna(subset=list(needed))
    if len(d) < 3:
        return

    fig, ax = plt.subplots(figsize=(5.6, 4.0))
    for cond in CONDITIONS:
        sub = d[d["agent_condition"] == cond]
        if sub.empty:
            continue
        ax.scatter(sub["mean_normalised_time_cross"], sub["trust_composite"],
                   s=48, color=CONDITION_COLOR[cond], marker=CONDITION_MARKER[cond],
                   alpha=0.8, edgecolors="white", linewidths=1.1, zorder=3,
                   label=CONDITION_LABEL[cond])

    rho, p = stats.spearmanr(d["mean_normalised_time_cross"], d["trust_composite"])
    if len(d) >= 4:
        slope, intercept = np.polyfit(d["mean_normalised_time_cross"], d["trust_composite"], 1)
        xs = np.linspace(d["mean_normalised_time_cross"].min(),
                         d["mean_normalised_time_cross"].max(), 50)
        ax.plot(xs, slope * xs + intercept, color=INK_MUTED, linewidth=1.2,
                linestyle=(0, (5, 3)), zorder=2)
    ax.annotate(f"pooled Spearman ρ = {rho:.2f}  (p = {p:.3f}, n = {len(d)})",
                xy=(0.02, 0.03), xycoords="axes fraction", fontsize=8.5, color=INK_MUTED)

    ax.axvline(1.0, color=GRID, linewidth=1, zorder=1)
    ax.set_xlabel("Measured: cross-surface time ÷ own baseline  (lower = faster)",
                  color=INK, fontsize=9.5)
    ax.set_ylabel("Perceived: trust composite (1–7)", color=INK, fontsize=9.5)
    ax.set_title("Trust against measured effectiveness", color=INK, fontsize=11,
                 loc="left", pad=10)
    _style(ax)
    ax.legend(frameon=False, fontsize=9, labelcolor=INK, loc="best")
    fig.tight_layout()
    fig.savefig(out, dpi=200)
    plt.close(fig)


def render_all(tasks: pd.DataFrame, participants: pd.DataFrame, out_dir: Path) -> list[Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    written: list[Path] = []
    jobs = [
        ("fig2_normalised_time.png", lambda p: fig_normalised_time(tasks, p)),
        ("fig3_correctness.png", lambda p: fig_correctness(tasks, p)),
        ("fig4_delegation.png", lambda p: fig_delegation(participants, p)),
        ("fig5_perception_gap.png", lambda p: fig_perception_gap(participants, p)),
    ]
    for name, fn in jobs:
        path = out_dir / name
        fn(path)
        if path.exists():
            written.append(path)
    return written
