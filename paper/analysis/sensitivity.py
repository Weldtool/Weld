"""
Sensitivity analysis for the WELD agent-scope study (Section 5.3 of WELD_TOSEM_PAPER.tex).

Design: C3 vs C1 on cross-surface agent tasks (T5, T6).
  n participants per cell, k=2 cross-surface tasks each.
  log(time)_ij = mu + beta*cond_i + gamma*log(baseline_i) + u_i + e_ij
  u_i ~ N(0, s_u^2)   between-participant
  e_ij ~ N(0, s_e^2)  within-participant (task-to-task)

The per-person baseline enters as a covariate; it absorbs a fraction r^2 of the
BETWEEN-participant variance (r = correlation of baseline log-time with agent
log-time at the participant level).

Analytic MDE, then a simulation check.
"""

import numpy as np
from scipy import stats

N_PER_CELL = 8  # 20-30 total => ~7-10 per cell; 8 is the midpoint
K_TASKS = 2  # T5, T6
ALPHA = 0.05
POWER = 0.80


def mde_log(sigma_total, icc, r, n=N_PER_CELL, k=K_TASKS, alpha=ALPHA, power=POWER):
    """Minimum detectable difference in log(time) between two cells."""
    s_u2 = icc * sigma_total**2
    s_e2 = (1 - icc) * sigma_total**2
    # variance of a participant's mean outcome, after the baseline covariate
    # absorbs r^2 of the between-participant component
    var_pmean = s_u2 * (1 - r**2) + s_e2 / k
    se_diff = np.sqrt(2 * var_pmean / n)
    df = 2 * n - 3  # two groups, one covariate
    tcrit = stats.t.ppf(1 - alpha / 2, df)
    tpow = stats.t.ppf(power, df)
    return (tcrit + tpow) * se_diff, se_diff, np.sqrt(var_pmean)


def simulate(sigma_total, icc, r, n=N_PER_CELL, k=K_TASKS, reps=20000, seed=0):
    """Empirical power at the analytic MDE, via the participant-mean t-test
    (algebraically the balanced-design equivalent of the LMM contrast)."""
    rng = np.random.default_rng(seed)
    delta, _, sd_pmean = mde_log(sigma_total, icc, r, n, k)
    s_u = np.sqrt(icc) * sigma_total
    s_e = np.sqrt(1 - icc) * sigma_total
    hits = 0
    for _ in range(reps):
        out = []
        for grp, shift in ((0, 0.0), (1, delta)):
            u = rng.normal(0, s_u, n)
            base = r * u + rng.normal(0, s_u * np.sqrt(max(1 - r**2, 0)), n)
            e = rng.normal(0, s_e, (n, k))
            y = shift + u + e.mean(axis=1)
            # residualise on the baseline covariate (what the model does)
            if r > 0:
                b = np.polyfit(base, y, 1)[0]
                y = y - b * (base - base.mean())
            out.append(y)
        t, p = stats.ttest_ind(out[1], out[0])
        hits += p < ALPHA
    return hits / reps


#: The assumption the paper quotes in Section 5.3.
CENTRAL = dict(sigma=0.5, icc=0.5, r=0.5)


def headline(sim: bool = True) -> dict[str, float]:
    """The numbers Section 5.3 quotes, as data rather than as printed text.

    `run_all.py` folds these into the report so the reported MDE cannot drift
    from the one this script computes. `sim=False` skips the 20k-rep simulation,
    which is the only slow part.
    """
    m, _, _ = mde_log(CENTRAL["sigma"], CENTRAL["icc"], CENTRAL["r"])
    out = {
        "mde_log": float(m),
        "pct_reduction": float((1 - np.exp(-m)) * 100),
        "cohens_d": float(m / CENTRAL["sigma"]),
        "n_per_cell": N_PER_CELL,
        "k_tasks": K_TASKS,
        **{f"assumed_{k}": v for k, v in CENTRAL.items()},
    }
    if sim:
        out["power_sim"] = float(
            simulate(CENTRAL["sigma"], CENTRAL["icc"], CENTRAL["r"])
        )
    return out


def n_for_reduction(pct: float = 20.0, max_n: int = 400) -> tuple[int | None, float]:
    """Smallest n per cell that could detect a `pct` reduction, or None.

    Returns None rather than printing nothing when even `max_n` is too small --
    the honest answer to "what would it take" is sometimes "more than you are
    ever going to recruit", and that is worth saying out loud in a paper whose
    whole sensitivity story is about being underpowered for small effects.
    """
    target = -np.log(1 - pct / 100)
    for n in range(6, max_n + 1):
        m, _, _ = mde_log(CENTRAL["sigma"], CENTRAL["icc"], CENTRAL["r"], n=n)
        if m <= target:
            return n, target
    return None, target


def main() -> None:
    print(f"Design: {N_PER_CELL}/cell, k={K_TASKS} cross-surface tasks, "
          f"alpha={ALPHA}, power={POWER}\n")
    hdr = f"{'sigma':>6} {'ICC':>5} {'r':>5} | {'MDE(log)':>9} {'% faster':>9} {'d':>6}"
    print(hdr)
    print("-" * len(hdr))
    for sigma in (0.4, 0.5, 0.6):
        for icc in (0.4, 0.6):
            for r in (0.0, 0.5, 0.7):
                m, _, _ = mde_log(sigma, icc, r)
                pct = (1 - np.exp(-m)) * 100
                print(f"{sigma:>6.1f} {icc:>5.1f} {r:>5.1f} | "
                      f"{m:>9.3f} {pct:>8.1f}% {m / sigma:>6.2f}")

    print("\nSimulation check (empirical power at the analytic MDE, 20k reps):")
    for sigma, icc, r in ((0.5, 0.4, 0.0), (0.5, 0.4, 0.5), (0.5, 0.6, 0.5), (0.6, 0.6, 0.7)):
        print(f"  sigma={sigma} ICC={icc} r={r}: power={simulate(sigma, icc, r):.3f}")

    h = headline()
    print(f"\nHeadline (central assumption sigma={CENTRAL['sigma']}, "
          f"ICC={CENTRAL['icc']}, r={CENTRAL['r']}):")
    print(f"  MDE = {h['mde_log']:.3f} log units = {h['pct_reduction']:.0f}% "
          f"time reduction, d = {h['cohens_d']:.2f}")
    print(f"  power at that MDE (sim) = {h['power_sim']:.3f}")

    pct = 20.0
    n, target = n_for_reduction(pct)
    print(f"\nWhat N would be needed for a {pct:.0f}% reduction "
          f"(sigma={CENTRAL['sigma']}, ICC={CENTRAL['icc']}, r={CENTRAL['r']})?")
    if n is None:
        print(f"  Not reachable below 400/cell (needs MDE <= {target:.3f} log units).")
        print("  A 20% effect is simply out of range for this design; say so "
              "rather than implying a feasible N exists.")
    else:
        m, _, _ = mde_log(CENTRAL["sigma"], CENTRAL["icc"], CENTRAL["r"], n=n)
        print(f"  n = {n} per cell (MDE {m:.3f} <= {target:.3f})")


if __name__ == "__main__":
    main()
