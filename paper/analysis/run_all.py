"""
End-to-end: exports in, tables and figures out.

    python run_all.py --data data_synthetic --out out

Writes `out/tables/*.csv`, `out/figures/*.png`, and `out/report.txt`, which is a
human-readable summary in the order the Results section presents them.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import pandas as pd

import analyze
import figures
from load import load


def _fmt(df: pd.DataFrame, floatfmt: str = "%.3f", index: bool = False) -> str:
    """Render a table. `index=True` where the row labels carry meaning -- model
    coefficient names, for one, which are otherwise silently dropped."""
    if df is None or (hasattr(df, "empty") and df.empty):
        return "  (no data)\n"
    return df.to_string(index=index, float_format=lambda v: floatfmt % v) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", required=True, type=Path, help="directory of exports")
    ap.add_argument("--out", default=Path("out"), type=Path)
    args = ap.parse_args()

    data = load(args.data)
    results = analyze.run_all(data)

    tables = args.out / "tables"
    figs = args.out / "figures"
    tables.mkdir(parents=True, exist_ok=True)

    for name, value in results.items():
        if isinstance(value, pd.DataFrame):
            value.to_csv(tables / f"{name}.csv", index=False)
        elif isinstance(value, analyze.ModelResult):
            value.table.to_csv(tables / f"{name}.csv")

    written = figures.render_all(results["task_table"], results["participant_table"], figs)
    # Absolute, so the emitted script runs from any working directory rather
    # than only from inside --out.
    task_table_path = (tables / "task_table.csv").resolve()
    (args.out / "correctness_model.R").write_text(
        analyze.r_snippet(task_table_path), encoding="utf-8"
    )

    lines: list[str] = []
    add = lines.append
    add("WELD AGENT-SCOPE STUDY — ANALYSIS REPORT")
    add("=" * 60)
    add("")

    p = results["participant_table"]
    add(f"Participants: {len(p)}")
    if not p.empty:
        add(_fmt(p.groupby("agent_condition").size().reset_index(name="n")))
    add("")

    add("1. MANIPULATION CHECK  (expect 0 violations)")
    add("-" * 60)
    mc = results["manipulation_check"]
    add(_fmt(mc, "%.1f"))
    if not mc.empty and mc["violations"].sum() > 0:
        add("  *** SCOPING VIOLATION DETECTED — report it, do not drop it. ***")
    add("")

    add("2. RQ1 — PRIMARY TIMING MODEL")
    add("-" * 60)
    m = results["primary_time_model"]
    add(_fmt(m.table, index=True))
    for n in m.notes:
        add(f"  note: {n}")
    add("")
    add("   Confirmatory + secondary contrasts (per-participant means):")
    add(_fmt(results["focused_contrast"]))
    add("")

    add("3. RQ1 — CORRECTNESS")
    add("-" * 60)
    c = results["correctness"]
    add(_fmt(c.table))
    for n in c.notes:
        add(f"  note: {n}")
    add("  Model of record: run out/correctness_model.R")
    add("")
    add("   Inter-coder agreement (pre-adjudication):")
    ka = results["coder_agreement"]
    add(_fmt(ka))
    if not ka.empty and pd.notna(ka.iloc[0]["kappa_quadratic"]):
        if ka.iloc[0]["kappa_quadratic"] < 0.60:
            add("   *** quadratic-weighted kappa < 0.60 — agreement is too weak to")
            add("       report the scores as a settled measure; say so in Results. ***")
    add("")

    add("4. H3 — COGNITIVE LOAD (TLX delta, agent - baseline)")
    add("-" * 60)
    tl = results["tlx_delta"]
    add(_fmt(tl, "%.2f"))
    if hasattr(tl, "attrs") and "kruskal_p" in getattr(tl, "attrs", {}):
        add(f"  Kruskal-Wallis H = {tl.attrs['kruskal_H']:.3f}, p = {tl.attrs['kruskal_p']:.4f}")
    add("")

    add("5. RQ2 — RELIANCE")
    add("-" * 60)
    add("   Agent funnel:")
    add(_fmt(results["agent_funnel"], "%.0f"))
    add("   Delegation ratio:")
    add(_fmt(results["delegation"]))
    add("")

    add("6. RQ3 — PERCEPTION vs MEASUREMENT")
    add("-" * 60)
    alpha = results["trust_alpha"]
    add(f"   Trust composite Cronbach's alpha = {alpha:.3f}")
    if alpha == alpha and alpha < 0.70:
        add("   *** alpha < 0.70 — pre-committed fallback: report the three items")
        add("       individually and treat composite analyses as descriptive. ***")
    add("")
    add(_fmt(results["perception_correlations"]))
    add("")
    add("   Reliance calibration (rely vs verify):")
    add(_fmt(results["reliance_calibration"]))
    add("")

    add(f"Figures written: {', '.join(f.name for f in written) or '(none)'}")

    report = "\n".join(lines)
    (args.out / "report.txt").write_text(report, encoding="utf-8")
    print(report)
    print(f"\nTables  -> {tables}\nFigures -> {figs}\nReport  -> {args.out / 'report.txt'}")


if __name__ == "__main__":
    main()
