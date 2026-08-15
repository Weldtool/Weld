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

    add("0. PARTICIPANT FLOW (CONSORT-style)")
    add("-" * 60)
    add(_fmt(results["participant_flow"], "%.0f"))
    flow = results["participant_flow"]
    if not flow.empty:
        allrow = flow[flow["agent_condition"] == "ALL"]
        if not allrow.empty:
            r = allrow.iloc[0]
            if r["with_all_12_tasks"] < r["completed"]:
                add("  *** Some completed participants are missing tasks. They belong")
                add("      in the flow figure as partial, not silently in the models. ***")
    add("")

    add("0a. CODING COVERAGE (collected vs. hand-coded)")
    add("-" * 60)
    add("   Recruitment is self-serve and costs nothing per participant; hand")
    add("   coding is linear in N and is the study's whole researcher cost. So")
    add("   the collected sample is uncapped and the coded sample is capped at")
    add("   the first CAP participants per condition in enrolment order. The gap")
    add("   is a number here rather than a sentence in Limitations.")
    add(_fmt(results["coding_coverage"], "%.1f"))
    cmp_tbl = results["coded_set_comparison"]
    if cmp_tbl is not None and not cmp_tbl.empty:
        add("   Did the cap select a different kind of participant? Every measure")
        add("   below is instrumented, so it exists for uncoded participants too.")
        add("   Expect no differences; a difference bounds how far the correctness")
        add("   result generalises, and does not touch the between-condition")
        add("   contrasts (the cap is applied within condition).")
        add(_fmt(cmp_tbl, "%.3f"))
        flagged = cmp_tbl[cmp_tbl["p"] < 0.05] if "p" in cmp_tbl.columns else cmp_tbl.iloc[:0]
        if not flagged.empty:
            add("  *** Coded and uncoded participants differ on: "
                f"{', '.join(flagged['measure'])}. Report it with the coverage")
            add("      table and state the generalisation limit. ***")
    add("")

    add("0b. BASELINE BALANCE (randomisation check — expect no differences)")
    add("-" * 60)
    add("   The six baseline tasks are identical across arms and run with no")
    add("   agent, so a difference here is a head start the later contrasts")
    add("   inherit — a caveat on everything that follows, not a finding.")
    bb = results["baseline_balance"]
    add(_fmt(bb, "%.3f"))
    if not bb.empty and "kruskal_p" in bb.columns:
        flagged = bb[bb["kruskal_p"] < 0.05]
        if not flagged.empty:
            add("  *** The arms differ at baseline on: "
                f"{', '.join(flagged['measure'])}. Report this with the flow")
            add("      numbers and carry it as a limitation. ***")
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
    add(f"   Fitted on {m.n_obs} agent-phase tasks from {m.n_participants} participants.")
    add("")
    add(_fmt(m.table, index=True))
    for n in m.notes:
        add(f"  note: {n}")
    add("")
    add("   CONFIRMATORY — baseline-adjusted condition contrasts on cross-surface")
    add("   tasks, taken from the model above. This is the number the paper leads")
    add("   with; C3 - C1 is the pre-specified confirmatory test.")
    add(_fmt(results["model_contrasts"]))
    add("")
    add("   Descriptive companion — the same contrasts on pre-divided normalised")
    add("   ratios (what Figure 2 plots). Not the inferential test: dividing by")
    add("   the anchor treats it as measured without error.")
    add(_fmt(results["focused_contrast"]))
    add("")
    add("   T6 alone — C3 vs C2, the top-of-the-ladder test (one task, so")
    add("   materially less powered than the pooled contrast above):")
    add(_fmt(results["t6_contrast"]))
    add("")

    add("2b. RQ1 — TIMING VALIDITY AND PRE-SPECIFIED SENSITIVITIES")
    add("-" * 60)
    add("   How often the timing caveats bit:")
    add(_fmt(results["timing_validity"], "%.1f"))
    add("   Agent uptake (the exposure the ITT estimate is diluted by):")
    add(_fmt(results["agent_uptake"], "%.1f"))
    add("   The confirmatory contrast under every pre-specified variant. A")
    add("   headline that moves across these rows is reported as moving, not")
    add("   resolved in the direction we prefer. The last row refits the")
    add("   headline on the correctness-coded participants alone, so the")
    add("   standing commitment — correctness always reported beside time —")
    add("   holds on a common set of people and not just in principle:")
    sp = results["sensitivity_panel"]
    add(_fmt(sp, "%.3f"))
    if not sp.empty and "time_ratio" in sp.columns:
        ratios = sp["time_ratio"].dropna()
        if len(ratios) > 1 and (ratios.max() - ratios.min()) > 0.10:
            add("  *** The confirmatory ratio moves by more than 0.10 across")
            add("      variants. Report the spread, and say which variant is of")
            add("      record (the primary ITT row) and why. ***")
        sig = sp["p"].dropna() < 0.05 if "p" in sp.columns else pd.Series(dtype=bool)
        if len(sig) > 1 and sig.nunique() > 1:
            add("  *** Significance is not stable across the pre-specified")
            add("      variants. This must be stated in Results. ***")
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
        n_coded_p = len(analyze.coded_participants(results["task_table"]))
        n_all_p = int(results["task_table"]["participant_code"].nunique())
        add(f"   Coded: {coded}/{total} agent-phase tasks "
            f"({100.0 * coded / total:.0f}%), "
            f"{n_coded_p}/{n_all_p} participants")
        if n_coded_p < n_all_p:
            add("   The timing result above is estimated on more people than this")
            add("   one. That is the coding cap working as designed, not a loss:")
            add("   see the coded-subset row of the sensitivity panel for the same")
            add("   headline on these participants only, and section 0a for whether")
            add("   the coded and uncoded halves differ.")
        # A gap between coded and total items has two very different causes and
        # only one of them is a problem. Participants outside the cap were never
        # meant to be coded; a *coded* participant with unscored tasks is an
        # unfinished or unadjudicated coding, and that one silently shrinks the
        # model. Warning on the total would cry wolf on every capped run, which
        # is how a real warning gets ignored.
        inside = agent_tasks[agent_tasks["participant_code"].isin(
            analyze.coded_participants(results["task_table"]))]
        holes = int(inside["correctness"].isna().sum())
        if coded == 0:
            add("   *** No correctness coding present. Everything in this section")
            add("       is empty until data/correctness.csv exists — see README. ***")
        elif holes:
            add(f"   *** INCOMPLETE — {holes} task(s) belonging to coded")
            add("       participants have no score. Rows with a blank score are")
            add("       dropped, so the model below is fitted on a subset. Finish")
            add("       adjudication before reporting these numbers. ***")
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

    add("   Per-task effort model (H3b) — effort ~ condition * task_type +")
    add("   baseline_effort + (1 | participant); localises the load result:")
    em = results["effort_model"]
    add(_fmt(em.table, index=True))
    for n in em.notes:
        add(f"  note: {n}")
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

    add("5. RQ2 — RELIANCE AND THE DIVISION OF LABOUR")
    add("-" * 60)
    add("   Agent funnel:")
    add(_fmt(results["agent_funnel"], "%.0f"))
    add("   Delegation ratio:")
    dl = results["delegation"]
    add(_fmt(dl))
    if "kruskal_p" in getattr(dl, "attrs", {}):
        add(f"  Across conditions: Kruskal-Wallis H = {dl.attrs['kruskal_H']:.3f}, "
            f"p = {dl.attrs['kruskal_p']:.4f}")
    add("")
    add("   Rates and traversals (the funnel counts events; these compare):")
    add(_fmt(results["reliance_detail"], "%.2f"))
    add("")
    add("   H4 calibration — do users override where the agent is wrong?")
    cal = results["calibration"]
    if cal.empty:
        add("   (no data — needs agent_errors.csv, or correctness coding for the")
        add("    weaker outcome-based fallback. See README.)")
    else:
        add(f"   Ground truth: {cal.attrs.get('source', 'unknown')}")
        add(_fmt(cal, "%.1f"))
        if "gap_ci_low" in cal.attrs:
            add(f"   Gap (wrong - right) = {cal.attrs['gap_signed_pct']:.1f} pp, "
                f"95% CI [{cal.attrs['gap_ci_low']:.1f}, {cal.attrs['gap_ci_high']:.1f}]; "
                f"Fisher exact p = {cal.attrs['fisher_p']:.4f}")
            add(f"   Good calibration was pre-set at >= {analyze.CALIBRATED_GAP_PCT:.0f} pp "
                "(analyze.CALIBRATED_GAP_PCT).")
            if cal.attrs.get("gap_uninformative"):
                add("  *** The interval contains both no difference and a")
                add("      well-calibrated one. Per the plan, this is UNINFORMATIVE")
                add("      about H4 — report it as such. H4 predicts a small gap, so")
                add("      it is a hypothesis a null would satisfy; do not write an")
                add("      underpowered interval up as confirmation. ***")
            else:
                add("   The interval is informative: it excludes either no difference")
                add("   or a well-calibrated one. Say which, and in those terms.")
        elif "fisher_p" in cal.attrs:
            add(f"   Pooled gap = {cal.attrs['gap_pct_points']:.1f} percentage points; "
                f"Fisher exact p = {cal.attrs['fisher_p']:.4f}")
        if str(cal.attrs.get("source", "")).startswith("FALLBACK"):
            add("  *** Using outcome correctness, not agent-error coding. A task can")
            add("      end correct because the participant fixed a wrong suggestion —")
            add("      exactly H4's case — so this under-detects miscalibration.")
            add("      Report it as the weaker measure it is. ***")
    ae = results["agent_error_agreement"]
    if not ae.empty and pd.notna(ae.iloc[0]["kappa"]):
        add("")
        add("   Agent-error coder agreement (pre-adjudication):")
        add(_fmt(ae))
    add("")
    add("   Open-ended themes:")
    th = results["themes"]
    if th.empty:
        add("   (no themes.csv — the qualitative coding has not been done. The")
        add("    paper's Qualitative Findings section cannot be written without it.)")
    else:
        add(_fmt(th, "%.0f"))
        add("   Theme coder agreement (pre-adjudication):")
        add(_fmt(results["theme_agreement"]))
    add("")

    add("6. RQ3 — PERCEPTION vs MEASUREMENT")
    add("-" * 60)
    alpha = results["trust_alpha"]
    add(f"   Trust composite Cronbach's alpha = {alpha:.3f}")
    composite_ok = not (alpha == alpha and alpha < 0.70)
    if not composite_ok:
        add("   *** alpha < 0.70 — pre-committed fallback in force: every")
        add("       `trust_composite` row below is DESCRIPTIVE ONLY. Report the")
        add("       three items individually; do not quote the composite as a")
        add("       scale. ***")
    add("")
    pc = results["perception_correlations"]
    n_expl = getattr(pc, "attrs", {}).get("n_exploratory_tests", 0)
    add(f"   One pre-specified primary correlation (primary=True), reported")
    add(f"   unadjusted. The other {n_expl} are exploratory and carry a")
    add("   Benjamini-Hochberg adjusted p (`p_fdr`) over that family — at this")
    add("   many tests, ~5 would clear p<.05 with no effect present at all.")
    add(_fmt(pc))
    if not pc.empty and "p_fdr" in pc.columns:
        survives = int((pc["p_fdr"] < 0.05).sum())
        raw = int((pc.loc[~pc["primary"], "p"] < 0.05).sum())
        add(f"   Exploratory rows with raw p < .05: {raw}; surviving FDR: {survives}.")
    add("")
    add("   Perceived context-switching vs the behavioural view-switch rate:")
    add(_fmt(results["switching_perception"]))
    add("")
    add("   Integration-perception items (no hypothesis rides on these; they say")
    add("   whether participants noticed the thing the paper is about):")
    add(_fmt(results["integration_perception"], "%.2f"))
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
