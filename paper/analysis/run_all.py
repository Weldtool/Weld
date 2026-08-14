"""
End-to-end: exports in, tables and figures out.

    python run_all.py --data data_synthetic --out out

Writes `out/tables/*.csv`, `out/figures/*.png`, and `out/report.txt`, which is a
human-readable summary in the order the Results section presents them.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
from pathlib import Path

import pandas as pd

import analyze
import figures
import sensitivity
from load import load


def _fmt(df: pd.DataFrame, floatfmt: str = "%.3f", index: bool = False) -> str:
    """Render a table. `index=True` where the row labels carry meaning -- model
    coefficient names, for one, which are otherwise silently dropped."""
    if df is None or (hasattr(df, "empty") and df.empty):
        return "  (no data)\n"
    return df.to_string(index=index, float_format=lambda v: floatfmt % v) + "\n"


def _run_r_model(out: Path, skip: bool = False) -> list[str]:
    """Fit the correctness model of record, if R is available.

    Returns the report lines describing what happened. Degrades to the old
    "run it yourself" instruction rather than failing the whole pipeline: R is
    not a declared dependency of this project, and a missing Rscript must not
    cost you every other table.
    """
    script = out / "correctness_model.R"
    log = out / "correctness_model_R_output.txt"
    if skip:
        return [f"  Model of record: skipped (--no-r). Run: Rscript {script}"]
    if shutil.which("Rscript") is None:
        return [
            "  Model of record NOT FITTED — Rscript not found on PATH.",
            f"  The reported correctness model is the CLMM, not the approximation",
            f"  above. Install R plus the `ordinal` package and run: Rscript {script}",
        ]
    proc = subprocess.run(
        ["Rscript", str(script)], capture_output=True, text=True, check=False
    )
    log.write_text(proc.stdout + proc.stderr, encoding="utf-8")
    if proc.returncode != 0:
        return [
            f"  *** Model of record FAILED (exit {proc.returncode}). See {log}. ***",
            "  Correctness results above are the approximation, not the reported model.",
        ]
    # The script prints exactly one STATUS line saying which result is of record.
    status = [ln.strip() for ln in proc.stdout.splitlines() if ln.startswith("STATUS:")]
    return [f"  Model of record: fitted -> {log}"] + [f"    {s}" for s in status]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", required=True, type=Path, help="directory of exports")
    ap.add_argument("--out", default=Path("out"), type=Path)
    ap.add_argument("--no-r", action="store_true",
                    help="skip the R correctness model of record")
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

    # The rely x verify cross-tab rides on .attrs, so the generic DataFrame
    # sweep above misses it; it is a table the paper asks for by name.
    crosstab = getattr(results["reliance_calibration"], "attrs", {}).get("crosstab")
    if crosstab is not None:
        crosstab.to_csv(tables / "rely_verify_crosstab.csv")

    written = figures.render_all(results["task_table"], results["participant_table"], figs)
    # Absolute, so the emitted script runs from any working directory rather
    # than only from inside --out.
    task_table_path = (tables / "task_table.csv").resolve()
    (args.out / "correctness_model.R").write_text(
        analyze.r_snippet(task_table_path), encoding="utf-8"
    )

    # The plan's correctness model of record is a CLMM, which only R can fit.
    # Running it here means one command produces the reported model instead of
    # leaving a step that is easy to forget on run day -- and the script decides
    # for itself whether the CLMM holds or the pre-specified fallback becomes the
    # result, so that decision still happens before anyone reads the numbers.
    r_status = _run_r_model(args.out, skip=args.no_r)

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
    add("   T6 alone — C3 vs C2, the top-of-the-ladder test (one task, so")
    add("   materially less powered than the pooled contrast above):")
    add(_fmt(results["t6_contrast"]))
    add("")

    add("3. RQ1 — CORRECTNESS")
    add("-" * 60)
    # Coverage first. A missing or half-adjudicated correctness.csv shrinks the
    # model silently -- rows with a blank score are simply dropped -- so the
    # report says up front how much of the coding is actually in.
    agent_tasks = results["task_table"]
    agent_tasks = agent_tasks[agent_tasks["phase"] == "agent"]
    coded = int(agent_tasks["correctness"].notna().sum())
    total = len(agent_tasks)
    if total:
        add(f"   Coded: {coded}/{total} agent-phase tasks "
            f"({100.0 * coded / total:.0f}%)")
        if coded == 0:
            add("   *** No correctness coding present. Everything in this section")
            add("       is empty until data/correctness.csv exists — see README. ***")
        elif coded < total:
            add("   *** INCOMPLETE. Rows with a blank score are dropped, so the")
            add("       model below is fitted on a subset. Finish adjudication")
            add("       before reporting these numbers. ***")
    add("")
    c = results["correctness"]
    add(_fmt(c.table))
    for n in c.notes:
        add(f"  note: {n}")
    for line in r_status:
        add(line)
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
    add("   Subscales (positive = more workload; `performance` is reverse-scored")
    add("   upstream, so positive there means worse perceived performance):")
    sub = results["tlx_subscales"]
    if not sub.empty:
        wide = sub.pivot(index="subscale", columns="agent_condition", values="mean_delta")
        add(_fmt(wide.reset_index(), "%.2f"))
        kw = sub.drop_duplicates("subscale")[["subscale", "kruskal_H", "kruskal_p"]]
        add(_fmt(kw, "%.4f"))
    else:
        add(_fmt(sub))
    add("")

    add("4b. USABILITY (SUS, against the published norm of 68)")
    add("-" * 60)
    su = results["sus"]
    add(_fmt(su, "%.2f"))
    if hasattr(su, "attrs") and "kruskal_p" in getattr(su, "attrs", {}):
        add(f"  Across conditions: Kruskal-Wallis H = {su.attrs['kruskal_H']:.3f}, "
            f"p = {su.attrs['kruskal_p']:.4f}")
        if su.attrs["kruskal_p"] < 0.05:
            add("  *** SUS differs by condition — the agent's scope appears to have")
            add("      changed how usable the workspace itself felt, which the design")
            add("      does not intend. Report it. ***")
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
    rc = results["reliance_calibration"]
    add(_fmt(rc))
    ct = getattr(rc, "attrs", {}).get("crosstab")
    if ct is not None:
        add("")
        add("   trust_rely (rows) x trust_verify (cols):")
        add(_fmt(ct.reset_index(), "%.0f"))
        add(f"   rely > verify: {rc.attrs['over_reliant']}"
            f" | rely < verify: {rc.attrs['under_reliant']}"
            f" | equal: {rc.attrs['balanced']}"
            f"  (of {rc.attrs['n_total']})")
    add("")

    add("7. SENSITIVITY (design's minimum detectable effect, Section 5.3)")
    add("-" * 60)
    h = sensitivity.headline()
    add(f"   Assumed sigma={h['assumed_sigma']}, ICC={h['assumed_icc']}, "
        f"r={h['assumed_r']}; {h['n_per_cell']}/cell, k={h['k_tasks']}.")
    add(f"   MDE = {h['mde_log']:.3f} log units = {h['pct_reduction']:.0f}% "
        f"time reduction, d = {h['cohens_d']:.2f}")
    add(f"   Simulated power at that MDE = {h['power_sim']:.3f}")
    n_needed, target = sensitivity.n_for_reduction(20.0)
    if n_needed is None:
        add("   A 20% reduction is out of range for this design at any feasible N.")
    else:
        add(f"   Detecting a 20% reduction would need n = {n_needed} per cell "
            f"({n_needed * 3} total).")
    add("")

    add(f"Figures written: {', '.join(f.name for f in written) or '(none)'}")

    report = "\n".join(lines)
    (args.out / "report.txt").write_text(report, encoding="utf-8")
    print(report)
    print(f"\nTables  -> {tables}\nFigures -> {figs}\nReport  -> {args.out / 'report.txt'}")


if __name__ == "__main__":
    main()
