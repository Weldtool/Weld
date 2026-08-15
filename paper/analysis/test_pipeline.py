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
from load import StudyData, load

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


# --- the analyses added to close the plan-vs-pipeline gaps ------------------


def test_confirmatory_contrast_comes_from_the_model_not_the_ratio(results):
    """The plan argues at length against pre-dividing by the baseline anchor,
    then the confirmatory test was computed on exactly that. The inferential
    number must come from the fitted ANCOVA; the ratio version stays as the
    descriptive companion, and both must be present so they can be compared."""
    mc = results["model_contrasts"]
    assert not mc.empty, "no model-based contrasts were produced"
    conf = mc[mc["confirmatory"]]
    assert len(conf) == 1, "exactly one contrast is pre-specified as confirmatory"
    assert conf.iloc[0]["contrast"] == "C3 - C1"
    # It is a real interval, not a point estimate dressed up as one.
    assert conf.iloc[0]["ratio_ci_low"] < conf.iloc[0]["time_ratio"] < conf.iloc[0]["ratio_ci_high"]
    # And the descriptive companion still exists.
    assert not results["focused_contrast"].empty


def test_c3_minus_c2_uses_the_covariance_not_two_independent_ses(results):
    """Var(a-b) = Var(a) + Var(b) - 2Cov(a,b). Differencing two independently
    quoted intervals ignores the covariance and overstates the width, which on
    the one contrast that isolates GitHub scope is the difference between an
    honest estimate and a needlessly hopeless one."""
    mc = results["model_contrasts"].set_index("contrast")
    if "C3 - C2" not in mc.index:
        pytest.skip("C3 - C2 not estimable in this run")
    naive = (mc.loc["C3 - C1", "se"] ** 2 + mc.loc["C2 - C1", "se"] ** 2) ** 0.5
    assert mc.loc["C3 - C2", "se"] < naive


def test_every_pre_specified_sensitivity_variant_is_fitted(results):
    """The plan pre-commits to reporting the headline under the idle rule, with
    interruptions dropped, and as-used. Deciding which to show after seeing them
    is how a pre-specified sensitivity becomes a post-hoc choice, so all of them
    are computed on every run."""
    panel = results["sensitivity_panel"]
    assert set(panel["variant"]) == set(analyze.SENSITIVITY_VARIANTS)


def test_as_used_analysis_actually_drops_tasks(results):
    """If the synthetic data has 100% uptake, the as-used row is identical to
    ITT and the comparison the plan pre-specifies is never exercised."""
    panel = results["sensitivity_panel"].set_index("variant")
    itt = panel.loc["primary (ITT, active time)", "n_obs"]
    as_used = panel.loc["as-used (>=1 invocation)", "n_obs"]
    assert as_used < itt, "synthetic uptake is 100%; the as-used path is untested"


def test_timing_validity_reports_the_numbers_the_plan_promises(results):
    tv = results["timing_validity"]
    for col in ("idle_rule_fired", "pct_interrupted", "idle_minutes_removed"):
        assert col in tv.columns
    allrow = tv[tv["agent_condition"] == "ALL"].iloc[0]
    assert allrow["idle_rule_fired"] > 0, "the idle rule never fired; branch untested"
    assert allrow["interrupted"] > 0, "no interrupted tasks; sensitivity untested"


def test_participant_flow_accounts_for_everyone(results, data):
    flow = results["participant_flow"]
    allrow = flow[flow["agent_condition"] == "ALL"].iloc[0]
    assert allrow["provisioned"] == len(data.participants)
    per_cond = flow[flow["agent_condition"] != "ALL"]["provisioned"].sum()
    assert per_cond == allrow["provisioned"], "conditions do not sum to the total"


def test_not_executed_rate_counts_drafts_as_having_landed(results):
    """Two different 'executed' counts exist here. The delegation ratio excludes
    drafted messages; 'did the call land' must not, or a gate blocking ~7% of
    calls is reported as a ~34% failure rate."""
    rd = results["reliance_detail"].set_index("agent_condition")
    row = rd.loc["ALL"]
    assert row["executed_total"] == row["executed_actions"] + row["executed_drafts"]
    assert row["pct_not_executed"] < 25.0


def test_calibration_prefers_agent_error_coding_over_the_fallback(results):
    """H4 is about whether the *agent* was right. Outcome correctness is the
    documented fallback and is weaker -- a task can end correct because the
    participant fixed a wrong suggestion, which is H4's own case."""
    cal = results["calibration"]
    assert not cal.empty
    assert cal.attrs["source"].startswith("agent-error coding")
    assert "factual_error" in cal.attrs["source"], (
        "factual_error is the pre-specified stratifier — it is the half a\n"
        "coder can settle by lookup rather than by opinion"
    )
    assert set(cal["stratum"]) == {"agent correct", "agent wrong"}


def test_calibration_falls_back_cleanly_when_agent_errors_are_missing(data):
    """Most of the study's life, agent_errors.csv will not exist. The analysis
    must degrade to the outcome-based measure and say so, not vanish."""
    stripped = StudyData(
        data.participants, data.events, data.survey, data.correctness,
        data.agent_turns, pd.DataFrame(), data.themes,
    )
    tasks = derive.task_table(stripped)
    cal = analyze.calibration(stripped, tasks)
    assert not cal.empty
    assert cal.attrs["source"].startswith("FALLBACK")


def test_calibration_only_counts_tasks_where_the_agent_was_used(results, data):
    """A task nobody invoked the agent on contributes a structural zero to the
    override rate and dilutes both strata."""
    cal = results["calibration"]
    total = cal[cal["agent_condition"] == "ALL"]["tasks"].sum()
    tasks = results["task_table"]
    used = tasks[(tasks["phase"] == "agent") & (tasks["agent_invocations"] > 0)]
    assert total <= len(used)


def test_perception_table_has_one_primary_and_fdr_on_the_rest(results):
    """~100 correlations with no multiplicity statement invites the reader to
    read the significant ones as findings; five would clear p<.05 by chance."""
    pc = results["perception_correlations"]
    assert int(pc["primary"].sum()) == 1
    primary = pc[pc["primary"]].iloc[0]
    assert (primary["scope"], primary["perception"], primary["measured"]) == \
        analyze.PRIMARY_PERCEPTION
    # The primary is reported unadjusted; the exploratory family is adjusted.
    assert pd.isna(primary["p_fdr"])
    expl = pc[~pc["primary"]]
    assert expl["p_fdr"].notna().all()
    # BH can only ever raise a p-value.
    assert (expl["p_fdr"] >= expl["p"] - 1e-9).all()


def test_all_five_integration_items_reach_the_analysis(results):
    """Four of the five were collected on every participant and analysed
    nowhere -- they only ever appeared as int_switch_perceived."""
    items = set(results["integration_perception"]["item"])
    assert items == set(derive.INTEGRATION_ITEMS)


def test_switching_perception_pairs_self_report_with_behaviour(results):
    sp = results["switching_perception"]
    assert not sp.empty
    assert "pooled" in set(sp["scope"])


def test_effort_model_carries_the_baseline_effort_anchor(results):
    """H3(b) is an ANCOVA on the participant's own baseline effort, the same
    logic as the timing model. Without the anchor it is a different analysis."""
    em = results["effort_model"]
    assert not em.table.empty, f"effort model did not fit: {em.summary}"
    assert "baseline_effort" in em.table.index


def test_theme_kappa_is_nominal_not_weighted(results):
    """Themes have no ordering, so a quadratic weight would invent one."""
    ta = results["theme_agreement"]
    assert not ta.empty
    assert 0.0 < float(ta.iloc[0]["kappa"]) <= 1.0


def test_agent_turns_are_exported_and_loadable(data):
    """The transcripts were written by the server and never exported, which
    left the plan naming a coding source that could not be reached."""
    assert not data.agent_turns.empty
    assert {"role", "content", "turn_index"} <= set(data.agent_turns.columns)
    assert set(data.agent_turns["role"]) == {"user", "assistant"}


def test_theme_merge_leaves_disagreements_blank(tmp_path):
    """Themes cannot be averaged, which is the clearest case for the rule the
    plan applies to every coding: disagreements are resolved by discussion."""
    import coding_packet

    a = tmp_path / "a.csv"
    b = tmp_path / "b.csv"
    a.write_text("participant_code,item_id,theme\nP1,open_useful,agent_helpful\n"
                 "P2,open_change,ui_usability\n", encoding="utf-8")
    b.write_text("participant_code,item_id,theme\nP1,open_useful,agent_helpful\n"
                 "P2,open_change,agent_trust\n", encoding="utf-8")
    out = tmp_path / "themes.csv"
    coding_packet.merge_coders(a, b, out, mode="themes")
    merged = pd.read_csv(out)
    agreed = merged[merged["participant_code"] == "P1"].iloc[0]
    disputed = merged[merged["participant_code"] == "P2"].iloc[0]
    assert agreed["theme"] == "agent_helpful"
    assert pd.isna(disputed["theme"])
    assert disputed["coder_1"] == "ui_usability"


# --- the H4 coding instrument ----------------------------------------------


def test_agent_error_packet_hides_the_participants_reaction():
    """The blinding that matters for H4 is not condition — condition is not the
    contrast in the calibration test. It is the *outcome*: a coder who reads
    "no, that's wrong, undo it" has inferred the stratifying variable from the
    behaviour the test predicts, and H4 confirms itself out of nothing."""
    import coding_packet

    turns = [
        ["user", "Which PR closes this card?"],
        ["assistant", "PR #22."],
        ["user", "No, that's wrong — undoing that."],
    ]
    kept = coding_packet._turns_without_the_reaction(turns)
    assert kept == turns[:2], "the trailing participant reaction must be dropped"
    # A conversation already ending on the assistant is untouched.
    assert coding_packet._turns_without_the_reaction(turns[:2]) == turns[:2]
    # All-participant (agent never replied) yields nothing to code, not a crash.
    assert coding_packet._turns_without_the_reaction([turns[0]]) == []


def test_answer_key_carries_the_facts_t6_turns_on():
    """T6 is the task where the correct PR is identifiable only from GitHub
    context. If the key does not disambiguate the three PRs, `factual_error` is
    back to being a memory test and the coders will not agree."""
    import coding_packet

    key = coding_packet._render_answer_key()
    assert key, "no answer key generated — is the seed still at SEED_DOC?"
    for pr in ("#22", "#27", "#34"):
        assert pr in key
    assert "vaccination" in key.lower()
    # The repo object must render as a name, not a dumped dict.
    assert "readme_excerpt" not in key
    # Seeded card references resolve to titles a coder can match on.
    assert "[[card:" not in key


def test_dispute_file_does_not_show_the_adjudicator_what_the_coders_chose(tmp_path):
    """Independent adjudication means independent. Showing a third coder a 1 and
    a 0 anchors the judgement and turns the third pass into a vote — which is
    what the discussion route already was."""
    import coding_packet

    a, b = tmp_path / "a.csv", tmp_path / "b.csv"
    a.write_text("participant_code,task_id,factual_error,judgement_error\n"
                 "P1,T5,1,0\n", encoding="utf-8")
    b.write_text("participant_code,task_id,factual_error,judgement_error\n"
                 "P1,T5,0,0\n", encoding="utf-8")
    out = tmp_path / "agent_errors.csv"
    coding_packet.merge_coders(a, b, out, mode="agent-errors")

    disputes = pd.read_csv(tmp_path / "agent_errors_disputes.csv")
    assert list(disputes.columns) == [
        "participant_code", "task_id", "factual_error", "judgement_error"
    ]
    assert disputes[["factual_error", "judgement_error"]].isna().all().all()


def test_adjudicator_resolves_disputes_and_leaves_agreements_alone(tmp_path):
    import coding_packet

    a, b = tmp_path / "a.csv", tmp_path / "b.csv"
    a.write_text("participant_code,task_id,factual_error,judgement_error\n"
                 "P1,T5,0,0\nP2,T6,1,0\n", encoding="utf-8")
    b.write_text("participant_code,task_id,factual_error,judgement_error\n"
                 "P1,T5,0,0\nP2,T6,0,0\n", encoding="utf-8")
    adj = tmp_path / "adj.csv"
    adj.write_text("participant_code,task_id,factual_error,judgement_error\n"
                   "P2,T6,1,0\n", encoding="utf-8")
    out = tmp_path / "agent_errors.csv"
    coding_packet.merge_coders(a, b, out, mode="agent-errors", adjudicator=adj)

    merged = pd.read_csv(out).set_index("participant_code")
    assert merged.loc["P1", "factual_error"] == 0      # agreement stands
    assert merged.loc["P2", "factual_error"] == 1      # dispute resolved
    # Pre-adjudication coder columns survive, or kappa becomes meaningless.
    assert merged.loc["P2", "factual_error_1"] == 1
    assert merged.loc["P2", "factual_error_2"] == 0


def test_agent_error_is_derived_as_either_kind_of_error(tmp_path):
    """`calibration()` and load.py read one `agent_error`; asking coders for a
    third judgement they'd have to keep consistent with the other two is how
    coding files drift."""
    import coding_packet

    a, b = tmp_path / "a.csv", tmp_path / "b.csv"
    body = ("participant_code,task_id,factual_error,judgement_error\n"
            "P1,T5,0,0\nP2,T5,1,0\nP3,T5,0,1\nP4,T5,1,1\n")
    a.write_text(body, encoding="utf-8")
    b.write_text(body, encoding="utf-8")
    out = tmp_path / "agent_errors.csv"
    coding_packet.merge_coders(a, b, out, mode="agent-errors")

    merged = pd.read_csv(out).set_index("participant_code")
    assert list(merged["agent_error"]) == [0, 1, 1, 1]


def test_single_value_codings_keep_their_documented_column_names(tmp_path):
    """correctness.csv is documented as score/coder_1/coder_2 and
    `coder_agreement` reads those names. Renaming them would empty kappa
    silently rather than fail."""
    import coding_packet

    a, b = tmp_path / "a.csv", tmp_path / "b.csv"
    a.write_text("participant_code,task_id,score\nP1,B1,3\n", encoding="utf-8")
    b.write_text("participant_code,task_id,score\nP1,B1,3\n", encoding="utf-8")
    out = tmp_path / "correctness.csv"
    coding_packet.merge_coders(a, b, out, mode="correctness")
    assert list(pd.read_csv(out).columns) == [
        "participant_code", "task_id", "score", "coder_1", "coder_2"
    ]


def test_agent_error_kappa_is_reported_for_both_halves(results):
    """factual and judgement are separate judgements with different expected
    reliability — factual is a lookup, judgement is an opinion. One pooled kappa
    would hide that."""
    ae = results["agent_error_agreement"]
    assert set(ae["coding"]) >= {"factual_error", "judgement_error"}


# --- baseline balance -------------------------------------------------------


def test_baseline_balance_checks_the_arms_before_the_agent_appears(results):
    """Minimisation equalises cell *sizes*, not ability. If C3 were simply
    faster to begin with, every later contrast inherits the head start — and the
    baseline correctness coding has no other consumer, so without this the six
    baseline scores per participant are collected and never used."""
    bb = results["baseline_balance"]
    assert not bb.empty
    assert "baseline correctness" in set(bb["measure"]), (
        "baseline correctness is coded by hand; it must be used for something"
    )
    assert {"baseline time (ms)", "baseline effort"} <= set(bb["measure"])
    assert bb["kruskal_p"].notna().all()


def test_calibration_reports_an_interval_not_just_a_point_estimate(results):
    """H4 predicts a *small* gap, so it is satisfied by a null. At n~8/cell the
    default outcome is a smallish gap with a wide interval, which reads as
    confirmation unless the interval is on the page."""
    cal = results["calibration"]
    for k in ("gap_signed_pct", "gap_ci_low", "gap_ci_high", "gap_uninformative"):
        assert k in cal.attrs, f"calibration is missing `{k}`"
    assert cal.attrs["gap_ci_low"] < cal.attrs["gap_signed_pct"] < cal.attrs["gap_ci_high"]


def test_a_wide_gap_interval_is_flagged_uninformative_not_supportive():
    """The failure mode this exists to prevent: an interval spanning both no
    difference and good calibration cannot tell them apart, and must not be
    written up as evidence of miscalibration."""
    # ~7pp gap on 15 tasks a side — the realistic shape of a thin cell here.
    lo, hi = analyze._newcombe_diff_ci(5, 15, 4, 15)
    assert lo < 0 < hi
    assert hi > analyze.CALIBRATED_GAP_PCT / 100.0, (
        "this interval spans both hypotheses and must be reported as uninformative"
    )
    # And the converse: a narrow interval that clears the threshold is a real
    # finding, not something the guard should suppress.
    lo2, hi2 = analyze._newcombe_diff_ci(4, 30, 3, 30)
    assert hi2 < analyze.CALIBRATED_GAP_PCT / 100.0


def test_wilson_interval_stays_inside_zero_and_one():
    """Override rates sit near 0 and 1, where the normal approximation escapes
    the unit interval."""
    for k, n in ((0, 12), (12, 12), (1, 40)):
        lo, hi = analyze._wilson(k, n)
        assert 0.0 <= lo <= hi <= 1.0


# --- phased correctness coding ---------------------------------------------


@pytest.mark.parametrize(
    "phase,prefix,n_per_participant", [("agent", "T", 6), ("baseline", "B", 6)]
)
def test_packet_phase_covers_only_that_half(synthetic, tmp_path, phase, prefix,
                                            n_per_participant):
    """Agent tasks are the only ones any correctness model reads, so coding them
    alone halves the largest job. Baselines stay available as a second round
    rather than being discarded."""
    import coding_packet

    out = tmp_path / phase
    n = coding_packet.build_packet(synthetic, out, seed=1, phase=phase)
    assert n == n_per_participant * N_PARTICIPANTS

    template = pd.read_csv(out / f"coder_1_{phase}.csv")
    assert template["task_id"].str.startswith(prefix).all()
    # Suffixed, so a second round cannot overwrite the first round's files.
    assert (out / f"CODING_PACKET_{phase}.md").exists()
    assert not (out / "coder_1.csv").exists()


def test_agent_round_is_the_default_and_all_is_still_reachable(synthetic, tmp_path):
    """The agent round is the committed default: it is the half every
    correctness model reads, and coding all twelve doubles the largest job to
    strengthen a balance check that also runs on instrumented time and effort.
    `all` stays reachable with its documented filenames, because the decision is
    recorded in the plan rather than welded into the code."""
    import coding_packet

    default = tmp_path / "default"
    n = coding_packet.build_packet(synthetic, default, seed=1, cap=0)
    assert n == 6 * N_PARTICIPANTS
    assert (default / "CODING_PACKET_agent.md").exists()

    every = tmp_path / "all"
    n = coding_packet.build_packet(synthetic, every, seed=1, phase="all", cap=0)
    assert n == 12 * N_PARTICIPANTS
    assert (every / "CODING_PACKET.md").exists()
    assert (every / "coder_1.csv").exists()


# --- the coding cap ---------------------------------------------------------


@pytest.fixture(scope="session")
def oversubscribed(tmp_path_factory) -> Path:
    """A run that recruited past the coding budget — 12 per arm against a cap of
    10. Below the cap the cap is a no-op and tests nothing."""
    d = tmp_path_factory.mktemp("oversubscribed")
    synthesize.generate(d, 36, seed=11)
    return d


def test_cap_takes_the_first_n_per_condition_in_enrolment_order(oversubscribed,
                                                                tmp_path):
    """The whole defence of the cap is that the rule cannot be steered: it is
    fixed at provisioning, before the participant has done anything, and applied
    within condition so it cannot load one arm."""
    import coding_packet

    data = load(oversubscribed)
    manifest = coding_packet.coded_set(data, cap=10)
    coded = manifest[manifest["coded"]]

    assert set(coded.groupby("agent_condition").size()) == {10}, \
        "the cap must be applied within condition, not to the pooled sample"

    # Nobody coded was enrolled after somebody uncoded in the same arm.
    for cond, g in manifest[manifest["eligible"]].groupby("agent_condition"):
        latest_in = g[g["coded"]]["provisioned_at"].max()
        earliest_out = g[~g["coded"]]["provisioned_at"].min()
        assert latest_in <= earliest_out, f"{cond} coded set is not a prefix"


def test_cap_is_deterministic_across_runs(oversubscribed):
    """A selection that moved between runs would be indistinguishable from one
    chosen after seeing the data."""
    import coding_packet

    data = load(oversubscribed)
    first = coding_packet.coded_set(data, cap=10)
    second = coding_packet.coded_set(load(oversubscribed), cap=10)
    assert list(first[first["coded"]]["participant_code"]) == \
           list(second[second["coded"]]["participant_code"])


def test_all_three_codings_cover_the_same_people(oversubscribed, tmp_path):
    """Three codings on three different subsets would make the agent-error and
    theme results describe a different sample from correctness, for no reason a
    reader could see."""
    import coding_packet

    out = tmp_path / "packets"
    coding_packet.build_packet(oversubscribed, out, seed=1, cap=10)
    correctness = set(pd.read_csv(out / "coder_1_agent.csv")["participant_code"])

    coding_packet.build_qualitative_packet(oversubscribed, out, seed=1, cap=10)
    themes = set(pd.read_csv(out / "theme_coder_1.csv")["participant_code"])

    coding_packet.build_agent_error_packet(oversubscribed, out, seed=1, cap=10)
    errors = set(pd.read_csv(out / "agent_error_coder_1.csv")["participant_code"])

    assert len(correctness) == 30
    assert themes <= correctness and errors <= correctness


def test_cap_manifest_is_written_but_never_shipped(oversubscribed, tmp_path):
    """The manifest carries `agent_condition` — it is the one file in `coding/`
    that would unblind the whole round."""
    import coding_packet

    out = tmp_path / "packets"
    coding_packet.build_packet(oversubscribed, out, seed=1, cap=10)
    manifest = out / coding_packet.MANIFEST_NAME
    assert manifest.exists()
    assert "agent_condition" in pd.read_csv(manifest).columns
    assert "RESEARCHER_ONLY" in manifest.name

    # And what the coders do get still passes the blinding check.
    assert coding_packet.verify_blinding(out / "CODING_PACKET_agent.md") == []


def test_cap_records_everyone_not_just_the_selected(oversubscribed):
    """The decision has to be auditable after the fact rather than implicit in
    which files happen to exist."""
    import coding_packet

    data = load(oversubscribed)
    manifest = coding_packet.coded_set(data, cap=10)
    assert len(manifest) == 36
    assert set(manifest["reason"]) >= {"within cap", "past cap"}


def test_uncapped_codes_everyone_with_data(oversubscribed):
    """`--cap 0` has to stay available: the cap is a budget decision, and a
    reviewer asking for the full set must be answerable without a code change."""
    import coding_packet

    manifest = coding_packet.coded_set(load(oversubscribed), cap=0)
    assert manifest["coded"].sum() == manifest["eligible"].sum()


def test_coded_subset_refit_is_reported_beside_the_headline(oversubscribed):
    """Correctness is always reported beside time. With recruitment uncapped and
    coding capped they rest on different numbers of people, so the pairing only
    holds if the headline is also shown on the coded set."""
    data = load(oversubscribed)
    tasks = derive.task_table(data)
    coded = analyze.coded_participants(tasks)
    assert coded, "synthetic data should carry correctness scores"

    full = analyze.primary_time_model(tasks)
    subset = analyze.primary_time_model(tasks, coded_subset=True)
    assert subset.n_participants <= full.n_participants
    assert any("coded" in n.lower() for n in subset.notes)

    panel = analyze.sensitivity_panel(tasks)
    assert any("coded subset" in v for v in panel["variant"])


def test_coded_subset_refit_degrades_when_nothing_is_coded_yet(data):
    """Coding lands weeks after collection, so the refit must say so rather than
    crash the run on every export before then."""
    tasks = derive.task_table(data).copy()
    tasks["correctness"] = float("nan")
    res = analyze.primary_time_model(tasks, coded_subset=True)
    assert res.table.empty
    assert "no correctness coding" in res.summary


def test_coding_coverage_puts_the_gap_in_a_table(oversubscribed):
    """The gap between collected and coded belongs in a number the paper can
    quote, not a sentence in Limitations."""
    data = load(oversubscribed)
    tasks = derive.task_table(data)
    cov = analyze.coding_coverage(data, tasks)
    row = cov[cov["agent_condition"] == "ALL"].iloc[0]
    assert row["collected"] == 36
    assert row["correctness_coded"] <= row["collected"]
    assert {"agent_items", "agent_items_scored", "pct_items_scored"} <= set(cov.columns)


def test_coded_and_uncoded_participants_are_compared_not_assumed(oversubscribed):
    """The cap's defence is testable, and the test is what makes it a defence:
    every measure here is instrumented, so it exists for uncoded participants
    too."""
    data = load(oversubscribed)
    tasks = derive.task_table(data)
    # Only some participants coded, so there is an uncoded half to compare with.
    coded = sorted(tasks["participant_code"].unique())[:18]
    tasks.loc[~tasks["participant_code"].isin(coded), "correctness"] = float("nan")

    cmp = analyze.coded_set_comparison(tasks)
    assert not cmp.empty
    assert (cmp["n_uncoded"] > 0).all()
    assert {"mean_coded", "mean_uncoded", "p"} <= set(cmp.columns)


# --- themes: single-coded with a double-coded reliability sample ------------


def test_theme_reliability_subset_covers_every_question(oversubscribed, tmp_path):
    """The four questions provoke different vocabularies; a subset that missed
    one would report agreement for a codebook never tested on a quarter of its
    material."""
    import coding_packet

    out = tmp_path / "themes"
    total = coding_packet.build_qualitative_packet(
        oversubscribed, out, seed=1, cap=0, reliability_frac=0.25
    )
    full = pd.read_csv(out / "theme_coder_1.csv")
    subset = pd.read_csv(out / "theme_coder_2.csv")

    assert len(full) == total
    assert 0 < len(subset) < len(full)
    assert set(subset["item_id"]) == set(coding_packet.OPEN_ITEMS)
    # Coder 2's items are a subset of coder 1's, or the overlap kappa is computed
    # on rows the two never both saw.
    keys = lambda df: set(zip(df["participant_code"], df["item_id"]))  # noqa: E731
    assert keys(subset) <= keys(full)


def test_each_theme_coder_gets_their_own_packet(oversubscribed, tmp_path):
    """Coder 2 must not think the missing items were an oversight and go looking
    for them, and coder 1 must know their work is being compared."""
    import coding_packet

    out = tmp_path / "themes"
    coding_packet.build_qualitative_packet(oversubscribed, out, seed=1, cap=0)
    full = (out / "QUALITATIVE_PACKET.md").read_text()
    subset = (out / "QUALITATIVE_PACKET_reliability.md").read_text()

    assert "you are coder 1" in full.lower()
    assert "you are coder 2" in subset.lower()
    assert full.count("## Item ") > subset.count("## Item ")
    assert coding_packet.verify_blinding(out / "QUALITATIVE_PACKET_reliability.md") == []


def test_single_coded_themes_are_resolved_not_left_blank(tmp_path):
    """Rows coder 2 was deliberately never given have no disagreement to resolve,
    so coder 1's theme stands — otherwise the whole single-coded majority would
    be reported as unfinished and dropped by the pipeline."""
    import coding_packet

    a = tmp_path / "t1.csv"
    b = tmp_path / "t2.csv"
    a.write_text("participant_code,item_id,theme\n"
                 "P1,open_useful,agent_helpful\n"
                 "P2,open_useful,ui_usability\n"
                 "P3,open_change,agent_wrong\n", encoding="utf-8")
    # Coder 2 saw only P1 (agreeing) and P3 (disagreeing).
    b.write_text("participant_code,item_id,theme\n"
                 "P1,open_useful,agent_helpful\n"
                 "P3,open_change,agent_limited\n", encoding="utf-8")

    out = tmp_path / "themes.csv"
    coding_packet.merge_coders(a, b, out, "themes")
    merged = pd.read_csv(out).set_index("participant_code")

    assert merged.loc["P1", "theme"] == "agent_helpful"   # agreed
    assert merged.loc["P2", "theme"] == "ui_usability"    # single-coded, stands
    assert pd.isna(merged.loc["P3", "theme"])             # disputed, unresolved


def test_theme_kappa_excludes_the_single_coded_rows(tmp_path):
    """Filling single-coded rows from coder 1 and then computing agreement over
    all of them would report a kappa inflated by rows only one person saw."""
    import coding_packet

    a = tmp_path / "t1.csv"
    b = tmp_path / "t2.csv"
    rows = [("P1", "agent_helpful"), ("P2", "ui_usability"),
            ("P3", "agent_wrong"), ("P4", "agent_trust")]
    a.write_text("participant_code,item_id,theme\n"
                 + "".join(f"{p},open_useful,{t}\n" for p, t in rows),
                 encoding="utf-8")
    b.write_text("participant_code,item_id,theme\n"
                 "P1,open_useful,agent_helpful\n"
                 "P2,open_useful,agent_wrong\n", encoding="utf-8")

    out = tmp_path / "themes.csv"
    coding_packet.merge_coders(a, b, out, "themes")
    merged = pd.read_csv(out)

    agreement = analyze.nominal_agreement(merged, "themes")
    assert agreement.iloc[0]["n_items"] == 2, "kappa must span the overlap only"
    assert agreement.iloc[0]["n_coded"] == 4
    assert "reliability sample" in agreement.iloc[0]["note"]


def _round(tmp_path, name, rows):
    a, b = tmp_path / f"{name}_1.csv", tmp_path / f"{name}_2.csv"
    body = "participant_code,task_id,score\n" + "".join(
        f"{p},{t},{s}\n" for p, t, s in rows
    )
    a.write_text(body, encoding="utf-8")
    b.write_text(body, encoding="utf-8")
    return a, b


def test_second_coding_round_does_not_delete_the_first(tmp_path):
    """Coding arrives in rounds — agent tasks now, baselines later. Clobbering a
    completed round is unrecoverable without re-coding it."""
    import coding_packet

    out = tmp_path / "correctness.csv"
    a1, b1 = _round(tmp_path, "agent", [("P1", "T1", 3), ("P1", "T2", 2)])
    coding_packet.merge_coders(a1, b1, out)
    assert len(pd.read_csv(out)) == 2

    a2, b2 = _round(tmp_path, "base", [("P1", "B1", 1), ("P1", "B2", 0)])
    coding_packet.merge_coders(a2, b2, out)

    merged = pd.read_csv(out).set_index("task_id")
    assert len(merged) == 4
    assert merged.loc["T1", "score"] == 3        # first round survived
    assert merged.loc["B1", "score"] == 1        # second round landed


def test_re_merging_a_round_replaces_it_rather_than_duplicating(tmp_path):
    """Re-running after adjudication is the normal path, not an edge case."""
    import coding_packet

    out = tmp_path / "correctness.csv"
    a1, b1 = _round(tmp_path, "r1", [("P1", "T1", 3)])
    coding_packet.merge_coders(a1, b1, out)
    a2, b2 = _round(tmp_path, "r2", [("P1", "T1", 1)])
    coding_packet.merge_coders(a2, b2, out)

    merged = pd.read_csv(out)
    assert len(merged) == 1, "re-merge duplicated the row instead of replacing it"
    assert merged.iloc[0]["score"] == 1


def test_balance_check_survives_without_baseline_correctness(data):
    """If baselines are never coded, the randomisation check must degrade to
    time and effort — both free from telemetry — rather than disappear."""
    tasks = derive.task_table(data)
    tasks = tasks.copy()
    tasks.loc[tasks["phase"] == "baseline", "correctness"] = float("nan")

    bb = analyze.baseline_balance(tasks)
    measures = set(bb["measure"])
    assert "baseline correctness" not in measures
    assert {"baseline time (ms)", "baseline effort"} <= measures
