"""
Generate synthetic exports with the exact shape of the real ones.

The point is to exercise the whole pipeline before a single participant runs, so
that on the day real data lands nothing about the analysis is still unknown.

THIS PRODUCES FAKE DATA. It is a plumbing test, not a preview of results. The
effects baked in below are arbitrary and chosen only to make every code path
execute. Never put a number from a synthetic run anywhere near the paper.

    python synthesize.py --out data_synthetic --n 24
"""

from __future__ import annotations

import argparse
import json
import random
from datetime import datetime, timedelta, timezone
from pathlib import Path

import numpy as np

CONDITIONS = ["C1", "C2", "C3"]
BASELINE_TASKS = ["B1", "B2", "B3", "B4", "B5", "B6"]
AGENT_TASKS = ["T1", "T2", "T3", "T4", "T5", "T6"]
TASK_TYPE = {
    "B1": "single_surface", "B2": "cross_surface", "B3": "cross_surface",
    "B4": "cross_surface", "B5": "cross_surface", "B6": "comprehension",
    "T1": "comprehension", "T2": "comprehension", "T3": "single_surface",
    "T4": "single_surface", "T5": "cross_surface", "T6": "cross_surface",
}

# Arbitrary multiplicative effects, purely so the plots have something to draw.
CONDITION_EFFECT = {
    "comprehension":  {"C1": 1.00, "C2": 0.98, "C3": 0.97},
    "single_surface": {"C1": 1.00, "C2": 0.93, "C3": 0.92},
    "cross_surface":  {"C1": 1.00, "C2": 0.78, "C3": 0.62},
}

TLX_SUBSCALES = ["mental", "physical", "temporal", "performance", "effort", "frustration"]
TRUST_ITEMS = ["trust_reliable", "trust_predictable", "trust_competent", "trust_rely",
               "trust_context", "trust_again", "trust_verify", "trust_speed_perceived"]
INT_ITEMS = ["int_findacross", "int_noticed", "int_vs_separate", "int_reuse",
             "int_switch_perceived"]

#: Candidate open-ended answers, each paired with the theme a coder would give
#: it. Several per item so the theme table has structure to aggregate; the theme
#: labels match `coding_packet.STARTER_CODEBOOK`.
OPEN_RESPONSES = {
    "open_useful": [
        ("The assistant found the right card quickly.", "agent_helpful"),
        ("Having the chat and the board in one place saved me hunting.",
         "integration_noticed"),
        ("Honestly the search was the useful part, not the assistant.", "ui_usability"),
    ],
    "open_frustrating": [
        ("I had to re-check everything it changed.", "agent_trust"),
        ("It confidently pointed me at the wrong pull request.", "agent_wrong"),
        ("It kept saying it could not see the repository.", "agent_limited"),
    ],
    "open_change": [
        ("Show me what it is about to do before it does it.", "agent_trust"),
        ("Fewer clicks to get between a card and its discussion.",
         "context_switching"),
        ("The board columns were not obvious at first.", "ui_usability"),
    ],
    "open_integration": [
        ("Yes, the chat-to-card links helped.", "integration_noticed"),
        ("I noticed it but it did not change how I worked.", "integration_noticed"),
        ("Not really — the tasks felt made up so it was hard to tell.",
         "task_artificial"),
    ],
}


def _csv_cell(v: object) -> str:
    s = "" if v is None else str(v)
    if any(c in s for c in ',"\n'):
        return '"' + s.replace('"', '""') + '"'
    return s


def _write_csv(path: Path, header: list[str], rows: list[list[object]]) -> None:
    with path.open("w", encoding="utf-8") as fh:
        fh.write(",".join(header) + "\n")
        for r in rows:
            fh.write(",".join(_csv_cell(c) for c in r) + "\n")


#: Participants per condition that get hand-coded, mirroring
#: `coding_packet.DEFAULT_CAP`. The synthetic exports have to have the *shape* of
#: the real ones, and the defining shape of this study's data is that telemetry
#: covers everybody and hand coding covers a capped subset. Emitting coding for
#: all n would mean the coverage table, the coded-vs-uncoded comparison and the
#: coded-subset refit were never exercised until run day.
CODING_CAP = 10

#: Share of open-ended responses the second theme coder also sees. Themes are the
#: one single-coded coding, so a synthetic themes.csv with both coder columns
#: filled everywhere would hide the case `nominal_agreement` has to get right.
RELIABILITY_FRAC = 0.25


def _capped_codes(p_rows: list[list[object]], cap: int) -> set[str]:
    """The first `cap` participants per condition in enrolment order.

    Same rule as `coding_packet.coded_set`, restated rather than imported: this
    module fabricates the exports that module reads, and a shared import would
    make the synthetic data agree with the selection code by construction
    instead of by both following the written rule.
    """
    if cap <= 0:
        return {str(r[0]) for r in p_rows}
    seen: dict[str, int] = {}
    coded = set()
    for row in p_rows:  # already in provisioning order
        code, cond = str(row[0]), str(row[1])
        seen[cond] = seen.get(cond, 0) + 1
        if seen[cond] <= cap:
            coded.add(code)
    return coded


def generate(out_dir: Path, n: int, seed: int = 7, cap: int = CODING_CAP) -> None:
    rng = np.random.default_rng(seed)
    random.seed(seed)
    out_dir.mkdir(parents=True, exist_ok=True)

    # t_rows: agent transcript. a_rows: agent-error coding. th_rows: themes.
    # The last two are human codings rather than exports, but they are
    # synthesised for the same reason correctness is -- the analyses that consume
    # them have to be exercised before real data lands, not on run day.
    p_rows, e_rows, s_rows, c_rows = [], [], [], []
    t_rows, a_rows, th_rows = [], [], []
    t0 = datetime(2026, 8, 20, 14, 0, tzinfo=timezone.utc)
    event_id = 0

    for i in range(n):
        code = f"P{i + 1:04d}"
        cond = CONDITIONS[i % 3]
        skill = rng.normal(0, 0.35)          # person-level speed, log scale
        clock = t0 + timedelta(days=i // 6, minutes=17 * (i % 6))

        # One participant withdraws mid-session. Everyone completing all twelve
        # tasks means the exclusion path, the "N excluded and why" sentence, and
        # the participant-flow figure have no dry run -- and a participant-level
        # table that silently drops someone is exactly the bug you want to find
        # before run day, not after.
        withdraws = n > 6 and i == 6
        stop_after = 8 if withdraws else None

        p_rows.append([
            code, cond, "withdrawn" if withdraws else "completed", "v1.2",
            clock.isoformat(), clock.isoformat(),
            (clock + timedelta(minutes=62)).isoformat(),
            f"00000000-0000-0000-0000-{i:012d}",
            f"10000000-0000-0000-0000-{i:012d}",
            f"20000000-0000-0000-0000-{i:012d}",
        ])

        def emit(etype: str, task: str | None, payload: dict,
                 idle_s: float = 0.0) -> None:
            nonlocal event_id, clock
            event_id += 1
            clock += timedelta(seconds=float(rng.uniform(2, 20)) + idle_s)
            e_rows.append([code, cond, event_id, etype, task or "",
                           clock.isoformat(), clock.isoformat(),
                           json.dumps(payload)])

        emit("session_start", None, {})
        emit("consent_given", None, {"version": "v1.2"})

        tasks_done = 0
        for phase, tasks in (("baseline", BASELINE_TASKS), ("agent", AGENT_TASKS)):
            if stop_after is not None and tasks_done >= stop_after:
                break
            emit("phase_started", None, {"phase": phase})
            for task in tasks:
                if stop_after is not None and tasks_done >= stop_after:
                    emit("session_abandoned", None, {"after_tasks": tasks_done})
                    break
                tasks_done += 1
                ttype = TASK_TYPE[task]
                emit("task_shown", task, {"task_id": task, "task_type": ttype})

                base_s = {"comprehension": 190, "single_surface": 210,
                          "cross_surface": 265}[ttype]
                mult = CONDITION_EFFECT[ttype][cond] if phase == "agent" else 1.0
                practice = 0.93 if phase == "agent" else 1.0
                secs = base_s * mult * practice * float(np.exp(skill + rng.normal(0, 0.22)))
                away = float(max(0, rng.normal(6, 9))) * 1000

                # A little surface activity so delegation has a denominator.
                n_user = int(rng.integers(1, 5))
                if phase == "agent" and cond != "C1":
                    n_agent = int(rng.integers(1, 4 if cond == "C2" else 6))
                else:
                    n_agent = 0
                for _ in range(n_user):
                    emit(random.choice(["card_moved", "message_sent", "card_edited"]),
                         task, {})

                # Some tasks contain a long think/walk-away gap. Without these
                # the idle rule (>= IDLE_GAP_SECONDS between consecutive events
                # in a task) never fires, and the analysis plan's "fit the
                # primary model both ways" robustness check silently becomes two
                # fits of identical data. The gap is drawn well above the
                # threshold so the branch is genuinely exercised, and `away_ms`
                # above stays small so the conservative excess-over-away
                # subtraction leaves something behind.
                # Kept modest on purpose: large enough to clear the 180 s
                # threshold and exercise the adjustment, small enough that the
                # planted condition effect stays visible. Crank it up and the
                # primary model stops converging on 8/cell, which tests nothing
                # useful about the pipeline.
                idle_s = 0.0
                if rng.random() < 0.07:
                    idle_s = float(rng.uniform(185, 240))
                    secs += idle_s
                emit("view_changed", task, {"to": random.choice(["board", "chat"])},
                     idle_s=idle_s)

                # Cross-surface traversals: following a card ref out of chat, or
                # a PR ref off a card. These are the behavioural "integration
                # usage" measure, and leaving them structurally zero meant that
                # whole RQ2 row read 0.00 in every synthetic run -- a column that
                # is always zero tests nothing.
                if ttype == "cross_surface" and rng.random() < 0.45:
                    emit("card_referenced_from_chat", task, {})
                if cond == "C3" and task == "T6" and rng.random() < 0.5:
                    emit("pr_referenced_from_card", task, {})

                # Some agent-phase tasks get done without touching the agent.
                # Task wording offers it, never instructs it (plan, "Availability,
                # not instruction"), so this is the realistic case -- and it is
                # what makes the as-used model differ from the ITT one. With
                # uptake pinned at 100% the two are identical and the pre-specified
                # comparison is untested until run day.
                used_agent = rng.random() > 0.12
                if phase == "agent" and used_agent:
                    emit("agent_panel_opened", task, {})
                    turn_index = 0
                    for _ in range(int(rng.integers(1, 4))):
                        emit("agent_invoked", task, {})
                        turn_index += 1
                        t_rows.append([code, cond, task, turn_index, "user",
                                       f"Can you help me with {task}?",
                                       clock.isoformat()])
                        turn_index += 1
                        t_rows.append([code, cond, task, turn_index, "assistant",
                                       "Here is what I found in the workspace "
                                       "and what I would do next.",
                                       clock.isoformat()])
                        emit("agent_context_sent", task, _context_snapshot(cond))
                        emit("agent_response", task, {
                            "latency_ms": int(rng.integers(900, 4200)),
                            "response_type": "message",
                            "num_tool_calls": n_agent,
                            "model": "gpt-5.4-2026-03-05", "temperature": 0.2,
                            "prompt_version": "v1", "total_tokens": int(rng.integers(700, 3000)),
                        })
                    for _ in range(n_agent):
                        # `post_message` only ever drafts -- the participant
                        # sends. derive.py keeps drafts out of the delegation
                        # numerator, so the synthetic data has to produce some
                        # or that split goes untested until real data lands.
                        tool = random.choice(["move_card", "update_card", "post_message"])
                        emit("agent_tool_call", task, {
                            "tool": tool,
                            "allowed": True, "executed": True,
                            "drafted": tool == "post_message",
                        })
                    # The gate refusing an out-of-scope tool. C2 has no GitHub
                    # tools, so a `github_link` attempt is the realistic block;
                    # this is the enforcement signal itself, and leaving it
                    # always-zero means a regression in the gate would not show
                    # up anywhere in a synthetic run.
                    if cond == "C2" and rng.random() < 0.12:
                        emit("agent_tool_call", task, {
                            "tool": "github_link", "allowed": False,
                            "executed": False, "drafted": False,
                            "reason": "out_of_scope",
                        })
                    # The right-hand side of the funnel: not every suggestion is
                    # taken, some are taken but rewritten, and some agent work
                    # gets reverted. These carry the reliance story, so they
                    # cannot be structurally zero.
                    #
                    # All four are gated on C1 having no tools, which is not an
                    # arbitrary choice -- it mirrors the real system exactly.
                    # `agent_suggestion_{accepted,rejected}` and
                    # `agent_output_edited` are emitted only by
                    # MessageDraft.svelte, which renders drafts produced by the
                    # `chat_post` tool; `agent_action_undone` fires only when a
                    # user overrides a card the agent touched, via `board_write`.
                    # `allowed_tool_categories(C1)` is empty, so a real C1 run
                    # cannot produce any of them. Emitting them here would make
                    # C1's funnel look populated and hide that fact.
                    if cond != "C1":
                        if rng.random() < 0.3:
                            emit("agent_suggestion_accepted", task, {})
                        if rng.random() < 0.22:
                            emit("agent_suggestion_rejected", task, {})
                        if rng.random() < 0.18:
                            emit("agent_output_edited", task,
                                 {"edit_distance": int(rng.integers(5, 90))})
                        if rng.random() < 0.15:
                            emit("agent_action_undone", task, {})
                        # The participant declining a proposed tool call before
                        # it runs -- also tool-dependent, so also C2/C3 only.
                        if rng.random() < 0.10:
                            emit("agent_tool_rejected", task, {"tool": "move_card"})
                    # An agent failure is condition-independent: run.rs emits it
                    # for any condition, C1 included.
                    if rng.random() < 0.06:
                        emit("agent_error", task, {"kind": "timeout"})

                if task in ("B4", "T6"):
                    emit("pr_review_opened", task, {"number": 22 if task == "B4" else 34})
                    emit("pr_review_submitted", task, {
                        "number": 22 if task == "B4" else 34,
                        "verdict": random.choice(["approve", "comment", "request_changes"]),
                        "has_comment": True, "comment_length": 60,
                    })

                emit("task_final_state", task, {"snapshot": "omitted-in-synthetic"})
                emit("task_completed", task, {
                    "task_id": task, "task_type": ttype,
                    "task_time_ms": int(secs * 1000),
                    "wall_ms": int(secs * 1000 + away),
                    "away_ms": int(away),
                    "pause_count": int(rng.integers(0, 3)),
                    "was_interrupted": bool(rng.random() < 0.1),
                })
                emit("task_rating", task, {
                    "task_id": task, "task_type": ttype, "phase": phase,
                    "effort": int(np.clip(rng.normal(4 if phase == "baseline" else 3, 1.2), 1, 7)),
                    "agent_help": None if phase == "baseline"
                    else int(np.clip(rng.normal(4, 1.4), 1, 7)),
                })

                # Correctness, as the human coders would deliver it.
                score = int(np.clip(rng.normal(2.4 if phase == "baseline" else
                                               2.2 + (0.5 if cond == "C3" and
                                                      ttype == "cross_surface" else 0),
                                               0.8), 0, 3))
                # The two coders must not agree perfectly: identical columns make
                # quadratic-weighted kappa exactly 1.000 every run, so the
                # agreement statistic is never actually exercised and neither is
                # the reporting threshold it feeds. Disagree by one point about a
                # fifth of the time, which is what adjacent-category disagreement
                # on a 0-3 rubric realistically looks like; `score` stays the
                # consensus, as it would be after adjudication.
                drift = int(rng.choice([0, 0, 0, 0, 1, -1]))
                coder_2 = int(np.clip(score + drift, 0, 3))
                c_rows.append([code, task, score, score, coder_2])

                # Agent-error coding (H4). Only exists where the agent was used,
                # which is the same restriction `calibration()` applies -- an
                # untouched task has no agent output to be right or wrong about.
                # Wider scope is made slightly *more* error-prone here on purpose:
                # if the synthetic agent were flawless the calibration strata
                # would collapse to one and the whole test would go unexercised.
                if phase == "agent" and used_agent:
                    # Two independent binaries, as the coders now deliver them.
                    # `factual_error` is checkable against the answer key, so its
                    # coders agree more often than on the judgement call -- the
                    # synthetic disagreement rates reflect that, or the reported
                    # kappas would not show the split the design predicts.
                    fact_p = {"C1": 0.20, "C2": 0.24, "C3": 0.28}[cond]
                    fact = int(rng.random() < fact_p)
                    judge = int(rng.random() < 0.18)
                    fact_2 = fact if rng.random() > 0.07 else 1 - fact
                    judge_2 = judge if rng.random() > 0.20 else 1 - judge
                    a_rows.append([code, task, max(fact, judge),
                                   fact, fact, fact_2, judge, judge, judge_2])

            if phase == "baseline":
                emit("interim_tlx_shown", None, {})
                emit("interim_tlx_submitted", None, {})
            emit("phase_ended", None, {"phase": phase})

        emit("survey_started", None, {})
        emit("survey_completed", None, {})
        emit("session_end", None, {})

        # --- survey rows -------------------------------------------------
        load_drop = {"C1": 2.0, "C2": 6.0, "C3": 9.0}[cond]
        for sub in TLX_SUBSCALES:
            b = float(np.clip(rng.normal(52, 15), 0, 100))
            s_rows.append([code, cond, f"tlx_{sub}_baseline", round(b), clock.isoformat()])
            s_rows.append([code, cond, f"tlx_{sub}",
                           round(float(np.clip(b - load_drop + rng.normal(0, 8), 0, 100))),
                           clock.isoformat()])
        for k in range(1, 11):
            s_rows.append([code, cond, f"sus_{k:02d}",
                           int(np.clip(rng.normal(3.9 if k % 2 else 2.1, 0.9), 1, 5)),
                           clock.isoformat()])
        trust_level = {"C1": 4.1, "C2": 4.9, "C3": 5.5}[cond]
        for item in TRUST_ITEMS:
            s_rows.append([code, cond, item,
                           int(np.clip(rng.normal(trust_level, 1.1), 1, 7)),
                           clock.isoformat()])
        for item in INT_ITEMS:
            s_rows.append([code, cond, item,
                           int(np.clip(rng.normal(4.6, 1.2), 1, 7)), clock.isoformat()])
        # Open-ended answers, varied per participant so the theme table has more
        # than one row per item. A single fixed string collapses every theme
        # count to n and makes kappa undefined, which exercises nothing.
        for item, choices in OPEN_RESPONSES.items():
            text, theme = choices[int(rng.integers(0, len(choices)))]
            s_rows.append([code, cond, item, text, clock.isoformat()])
            # Theme coding, as the two coders would deliver it: mostly agreeing,
            # disagreeing on a minority, with `theme` the adjudicated consensus.
            theme_2 = theme if rng.random() > 0.18 else "other"
            th_rows.append([code, item, theme, theme, theme_2])

    # Recruitment is uncapped and hand coding is not, so the three coding files
    # cover a subset of the people the telemetry covers. Filtering here rather
    # than while generating keeps the fabricated *sessions* identical whatever
    # the cap is, which is the real relationship: the cap decides who gets coded,
    # not who gets run.
    coded = _capped_codes(p_rows, cap)
    c_rows = [r for r in c_rows if r[0] in coded]
    a_rows = [r for r in a_rows if r[0] in coded]
    th_rows = [r for r in th_rows if r[0] in coded]

    # Themes: coder 2 sees only the reliability sample, so their column is blank
    # on the rest. `theme` (the consensus) stays filled throughout, because a
    # single-coded row has no disagreement to resolve.
    by_item: dict[str, list[int]] = {}
    for i, r in enumerate(th_rows):
        by_item.setdefault(str(r[1]), []).append(i)
    double_coded: set[int] = set()
    for idxs in by_item.values():
        k = max(1, round(RELIABILITY_FRAC * len(idxs)))
        double_coded.update(random.sample(idxs, min(k, len(idxs))))
    for i, r in enumerate(th_rows):
        if i not in double_coded:
            r[4] = ""

    _write_csv(out_dir / "study_participants.csv",
               ["participant_code", "agent_condition", "status", "consent_version",
                "consent_given_at", "provisioned_at", "completed_at",
                "user_id", "team_id", "project_id"], p_rows)
    _write_csv(out_dir / "study_events.csv",
               ["participant_code", "agent_condition", "event_id", "event_type",
                "current_task_id", "client_ts", "server_ts", "payload"], e_rows)
    _write_csv(out_dir / "study_survey.csv",
               ["participant_code", "agent_condition", "item_id", "value",
                "answered_at"], s_rows)
    _write_csv(out_dir / "correctness.csv",
               ["participant_code", "task_id", "score", "coder_1", "coder_2"], c_rows)
    _write_csv(out_dir / "study_agent_turns.csv",
               ["participant_code", "agent_condition", "task_id", "turn_index",
                "role", "content", "created_at"], t_rows)
    _write_csv(out_dir / "agent_errors.csv",
               ["participant_code", "task_id", "agent_error",
                "factual_error", "factual_error_1", "factual_error_2",
                "judgement_error", "judgement_error_1", "judgement_error_2"],
               a_rows)
    _write_csv(out_dir / "themes.csv",
               ["participant_code", "item_id", "theme", "coder_1", "coder_2"], th_rows)

    print(f"Wrote synthetic exports for {n} participants to {out_dir}/")
    print(f"  Hand coding covers {len(coded)} of them "
          f"(cap {cap}/condition); themes are double-coded on "
          f"{len(double_coded)} of {len(th_rows)} responses.")
    print("  THESE ARE FAKE. Plumbing test only -- never cite a number from them.")


def _context_snapshot(cond: str) -> dict:
    """Mimic the gate's scoping snapshot, including its omissions.

    Two details here are load-bearing for the manipulation check, and both were
    once missing — which is how a broken check passed on synthetic data and then
    reported a total scoping failure on the first real export.

    First, `sources_included`: the gate declares its sources explicitly and the
    check reads that field. Second, the chat entries carry real prose, and that
    prose mentions the board and GitHub — exactly as the seeded conversation
    does. A check that decides scope by scanning the payload for those words
    will now fail loudly here instead of waiting for real participants.
    """
    sources = ["chat"]
    snap: dict = {
        "condition": cond,
        "chat": [
            {"channel": "pettracker", "sender": "Alex Rivera",
             "content": "Can someone capture this decision on the board so we "
                        "don't relitigate it in three weeks?"},
            {"channel": "general", "sender": "Priya Nair",
             "content": "The GitHub PR for the feeding schedule is up for review."},
        ],
    }
    if cond in ("C2", "C3"):
        snap["board"] = {"columns": ["Backlog", "In Progress", "Done"], "cards": 12}
        sources.append("board")
    if cond == "C3":
        snap["github"] = {"pull_requests": [22, 27, 34], "issues": [3]}
        sources.append("github")
    snap["sources_included"] = sources
    return snap


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default="data_synthetic", type=Path)
    ap.add_argument("--n", default=24, type=int, help="participants (multiple of 3)")
    ap.add_argument("--seed", default=7, type=int)
    ap.add_argument("--cap", default=CODING_CAP, type=int,
                    help="hand-coded participants per condition; 0 codes all")
    a = ap.parse_args()
    generate(a.out, a.n, a.seed, a.cap)
