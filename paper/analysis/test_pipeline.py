"""
Regression tests for the analysis pipeline.

These encode the invariants that were previously checked by eye after every
run -- the ones in README "Sanity checks that should hold", plus the structural
facts about the study design that fake data must not contradict.

    pytest -q

Most tests share one synthetic run (session-scoped fixture) because generating
and analysing 24 participants takes a few seconds and nothing here mutates it.

What these do NOT do: assert anything about effect sizes or p-values. The
synthetic effects are arbitrary and reseeding changes them; pinning a p-value
would produce a test that fails for no reason. They assert *structure* --
shapes, zeros that must be zero, columns that must exist, and the invariants the
paper's claims rest on.
"""

from __future__ import annotations

from pathlib import Path

import pandas as pd
import pytest

import analyze
import derive
import sensitivity
import synthesize
from load import load

N_PARTICIPANTS = 24


@pytest.fixture(scope="session")
def synthetic(tmp_path_factory) -> Path:
    d = tmp_path_factory.mktemp("synthetic")
    synthesize.generate(d, N_PARTICIPANTS, seed=7)
    return d


@pytest.fixture(scope="session")
def data(synthetic):
    return load(synthetic)


@pytest.fixture(scope="session")
def results(data):
    return analyze.run_all(data)


# --- the export contract ----------------------------------------------------


def test_synthetic_exports_match_the_loader_contract(data):
    """load.py asserts the server's column sets; the synthesizer must satisfy
    the same ones, or testing against it proves nothing about real data."""
    assert len(data.participants) == N_PARTICIPANTS
    assert not data.events.empty
    assert not data.survey.empty
    assert data.events["payload"].map(lambda p: isinstance(p, dict)).all()


# --- README "sanity checks that should hold" --------------------------------


def test_scoping_never_leaks(results):
    """C1 sees no board and no GitHub; C2 sees no GitHub. This is the paper's
    central claim about the apparatus -- if it fails, nothing else matters."""
    mc = results["manipulation_check"].set_index("agent_condition")
    assert mc["violations"].sum() == 0
    assert mc.loc["C1", "pct_board"] == 0.0
    assert mc.loc["C1", "pct_github"] == 0.0
    assert mc.loc["C2", "pct_github"] == 0.0
    # ...and the conditions that *should* carry context do.
    assert mc.loc["C2", "pct_board"] == 100.0
    assert mc.loc["C3", "pct_github"] == 100.0


def test_c1_delegates_nothing(results):
    """C1 is advise-only: `allowed_tool_categories(C1)` is empty in the server,
    so there is no mechanism by which it could delegate."""
    d = results["delegation"].set_index("agent_condition")
    assert d.loc["C1", "mean"] == 0.0
    p = results["participant_table"]
    assert p.loc[p["agent_condition"] == "C1", "chat_drafts"].sum() == 0


def test_c1_funnel_is_structurally_empty_past_invocation(results):
    """Regression: the synthesizer used to emit these for C1.

    `agent_suggestion_{accepted,rejected}` and `agent_output_edited` are emitted
    only by MessageDraft.svelte, which renders drafts from the `chat_post` tool;
    `agent_action_undone` fires only when a user overrides a card the agent
    touched via `board_write`; `agent_tool_rejected` needs a proposed tool call.
    C1 has no tools, so a real C1 run produces none of them. Fake data that does
    would hide that fact.
    """
    f = results["agent_funnel"].set_index("agent_condition")
    for col in ("tool_attempted", "tool_executed", "tool_blocked",
                "suggestion_accepted", "suggestion_rejected",
                "output_edited", "action_undone", "tool_rejected"):
        assert f.loc["C1", col] == 0, f"C1 cannot produce {col}"
    # C1 still invokes the agent and can still error.
    assert f.loc["C1", "invoked"] > 0


def test_only_c2_can_have_blocked_tool_calls(results):
    """The gate's enforcement signal. C1 has no tools to block and C3 has every
    scope, so a block is only reachable in C2."""
    f = results["agent_funnel"].set_index("agent_condition")
    assert f.loc["C1", "tool_blocked"] == 0
    assert f.loc["C3", "tool_blocked"] == 0
    assert f.loc["C2", "tool_blocked"] > 0, (
        "C2 never attempted an out-of-scope tool; the gate's enforcement path "
        "is untested by this run"
    )


def test_task_counts_are_complete_or_explained(results):
    """Every participant has 12 tasks unless they withdrew."""
    tasks = results["task_table"]
    p = results["participant_table"].set_index("participant_code")
    counts = tasks.groupby("participant_code").size()
    for code, n in counts.items():
        if p.loc[code, "status"] == "completed":
            assert n == 12, f"{code} completed but has {n} tasks"
        else:
            assert n < 12, f"{code} is {p.loc[code,'status']} but has all 12"


# --- paths that were silently untested before -------------------------------


def test_idle_rule_actually_fires(results):
    """The analysis plan offers "fitted both ways" as a robustness guarantee.

    With no >=180s gaps anywhere, both fits are of identical data and the
    guarantee is vacuous -- which is exactly what was happening.
    """
    tasks = results["task_table"]
    assert tasks["idle_rule_fired"].sum() > 0, (
        "no task contains an idle gap; the idle-adjusted model is a duplicate "
        "of the primary one and tests nothing"
    )
    fired = tasks[tasks["idle_rule_fired"]]
    assert (fired["idle_ms"] > 0).all()
    assert (fired["task_time_ms_idle_adj"] < fired["task_time_ms"]).all()


def test_idle_adjusted_model_differs_from_primary(results):
    primary = results["primary_time_model"].table["coef"]
    adjusted = results["primary_time_model_idle_adjusted"].table["coef"]
    assert not primary.round(6).equals(adjusted.round(6)), (
        "idle-adjusted refit is identical to the primary model"
    )


def test_funnel_right_hand_side_is_exercised(results):
    """These columns carry the reliance story and were all structurally zero."""
    f = results["agent_funnel"].set_index("agent_condition")
    for col in ("suggestion_accepted", "suggestion_rejected", "output_edited",
                "action_undone", "tool_rejected"):
        assert f.loc[["C2", "C3"], col].sum() > 0, f"{col} is zero everywhere"


def test_a_participant_withdraws(results):
    """The exclusion path needs a dry run before it meets real data."""
    p = results["participant_table"]
    assert (p["status"] != "completed").sum() >= 1


# --- outputs the paper names by name ----------------------------------------


def test_t6_contrast_is_reported_separately(results):
    """focused_contrast() pools T5+T6; the paper names T6 alone."""
    t6 = results["t6_contrast"]
    assert not t6.empty
    assert (t6["task_id"] == "T6").all()
    headline = t6[t6["headline"]]
    assert len(headline) == 1
    assert headline.iloc[0]["contrast"] == "C3 - C2"


def test_confirmatory_contrast_is_flagged_exactly_once(results):
    fc = results["focused_contrast"]
    assert fc["confirmatory"].sum() == 1
    assert fc.loc[fc["confirmatory"], "contrast"].iloc[0] == "C3 - C1"


def test_correlations_carry_intervals(results):
    c = results["perception_correlations"]
    assert {"ci_low", "ci_high"}.issubset(c.columns)
    sub = c.dropna(subset=["ci_low", "ci_high"])
    assert len(sub) > 0
    assert (sub["ci_low"] <= sub["spearman_rho"]).all()
    assert (sub["spearman_rho"] <= sub["ci_high"]).all()


def test_tlx_reports_every_subscale(results):
    sub = results["tlx_subscales"]
    assert set(sub["subscale"]) == set(derive.TLX_SUBSCALES)
    # The one item authored against the workload direction must stay flagged.
    perf = sub[sub["subscale"] == "performance"]
    assert perf["reverse_scored"].all()
    assert not sub[sub["subscale"] != "performance"]["reverse_scored"].any()


def test_sus_is_compared_against_the_published_norm(results):
    sus = results["sus"]
    assert "ALL" in set(sus["agent_condition"])
    assert analyze.SUS_NORM == 68.0
    assert (sus["norm"] == analyze.SUS_NORM).all()
    row = sus[sus["agent_condition"] == "ALL"].iloc[0]
    assert row["n"] == N_PARTICIPANTS
    assert row["vs_norm"] == pytest.approx(row["mean"] - 68.0)


def test_rely_verify_crosstab_is_produced(results):
    rc = results["reliance_calibration"]
    ct = rc.attrs.get("crosstab")
    assert ct is not None and not ct.empty
    total = rc.attrs["over_reliant"] + rc.attrs["under_reliant"] + rc.attrs["balanced"]
    assert total == rc.attrs["n_total"] == N_PARTICIPANTS


# --- units ------------------------------------------------------------------


def test_performance_subscale_is_reverse_scored():
    """The direction of this one item corrupts both phases if wrong, silently,
    and no other check would catch it. MAX + MIN - raw on a 1-100 slider."""
    wide = pd.DataFrame({"tlx_performance": [1.0, 50.0, 100.0]})
    flipped = derive._reverse_performance(wide.copy(), "")
    assert list(flipped["tlx_performance"]) == [100.0, 51.0, 1.0]


def test_spearman_ci_brackets_rho_and_guards_small_n():
    lo, hi = analyze._spearman_ci(0.5, 24)
    assert lo < 0.5 < hi
    # Undefined below n=5 rather than silently producing a nonsense interval.
    assert all(pd.isna(v) for v in analyze._spearman_ci(0.5, 4))


def test_kappa_is_undefined_not_perfect_when_coders_never_vary():
    """A single category throughout is no evidence of agreement. Reporting 1.0
    there would overstate the coding's reliability."""
    same = pd.DataFrame({
        "participant_code": ["P1", "P2", "P3"],
        "task_id": ["T1", "T2", "T3"],
        "score": [3, 3, 3], "coder_1": [3, 3, 3], "coder_2": [3, 3, 3],
    })
    out = analyze.coder_agreement(same)
    assert pd.isna(out.iloc[0]["kappa_quadratic"])
    assert out.iloc[0]["note"]


def test_sensitivity_answers_its_own_question():
    """The n-for-20% search used to print nothing when the answer exceeded its
    ceiling, which reads as "no answer" rather than "more than you can recruit"."""
    n, target = sensitivity.n_for_reduction(20.0)
    assert n is not None and n > sensitivity.N_PER_CELL
    mde, _, _ = sensitivity.mde_log(0.5, 0.5, 0.5, n=n)
    assert mde <= target
    # One below the reported n must not suffice, or n is not minimal.
    worse, _, _ = sensitivity.mde_log(0.5, 0.5, 0.5, n=n - 1)
    assert worse > target


def test_sensitivity_headline_matches_the_quoted_design():
    h = sensitivity.headline(sim=False)
    assert h["n_per_cell"] == 8 and h["k_tasks"] == 2
    assert 0.3 < h["mde_log"] < 1.0
    assert 30 < h["pct_reduction"] < 60


def test_blinding_check_catches_a_leak_but_not_our_own_instructions(tmp_path):
    """The packet's preamble tells the coder they are *not* being shown the
    condition. Scanning it flags that sentence forever, and a check that always
    fails is a check everyone learns to ignore."""
    import coding_packet

    p = tmp_path / "CODING_PACKET.md"
    p.write_text(
        "# Correctness coding packet\n\n"
        "You are deliberately not told which condition anyone was in.\n\n"
        "---\n\n"
        "## Item 001 — participant P0001, task B1\n\n"
        "Board state: card moved to Done.\n\n"
        "## Item 002 — participant P0002, task T3\n\n"
        "agent_condition: C3 — latency_ms 1840\n",
        encoding="utf-8",
    )
    leaks = coding_packet.verify_blinding(p)
    assert len(leaks) == 1, f"expected only the item leak, got {leaks}"
    assert "agent_condition" in leaks[0][1]


def test_blinding_check_passes_on_a_clean_packet(tmp_path):
    import coding_packet

    p = tmp_path / "CODING_PACKET.md"
    p.write_text(
        "# Correctness coding packet\n\n"
        "Score each item 0-3 on the final state.\n\n"
        "## Item 001 — participant P0001, task B1\n\n"
        "Board state: card moved to Done. Chat: 'looks good to me'.\n",
        encoding="utf-8",
    )
    assert coding_packet.verify_blinding(p) == []


def test_importing_sensitivity_does_not_run_the_simulation():
    """It had no __main__ guard, so importing it ran 20k reps."""
    src = Path(__file__).with_name("sensitivity.py").read_text(encoding="utf-8")
    assert 'if __name__ == "__main__":' in src
