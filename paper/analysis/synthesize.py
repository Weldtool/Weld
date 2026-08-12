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


def generate(out_dir: Path, n: int, seed: int = 7) -> None:
    rng = np.random.default_rng(seed)
    random.seed(seed)
    out_dir.mkdir(parents=True, exist_ok=True)

    p_rows, e_rows, s_rows, c_rows = [], [], [], []
    t0 = datetime(2026, 8, 20, 14, 0, tzinfo=timezone.utc)
    event_id = 0

    for i in range(n):
        code = f"P{i + 1:04d}"
        cond = CONDITIONS[i % 3]
        skill = rng.normal(0, 0.35)          # person-level speed, log scale
        clock = t0 + timedelta(days=i // 6, minutes=17 * (i % 6))

        p_rows.append([
            code, cond, "completed", "v1.2",
            clock.isoformat(), clock.isoformat(),
            (clock + timedelta(minutes=62)).isoformat(),
            f"00000000-0000-0000-0000-{i:012d}",
            f"10000000-0000-0000-0000-{i:012d}",
            f"20000000-0000-0000-0000-{i:012d}",
        ])

        def emit(etype: str, task: str | None, payload: dict) -> None:
            nonlocal event_id, clock
            event_id += 1
            clock += timedelta(seconds=float(rng.uniform(2, 20)))
            e_rows.append([code, cond, event_id, etype, task or "",
                           clock.isoformat(), clock.isoformat(),
                           json.dumps(payload)])

        emit("session_start", None, {})
        emit("consent_given", None, {"version": "v1.2"})

        for phase, tasks in (("baseline", BASELINE_TASKS), ("agent", AGENT_TASKS)):
            emit("phase_started", None, {"phase": phase})
            for task in tasks:
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
                emit("view_changed", task, {"to": random.choice(["board", "chat"])})

                if phase == "agent":
                    emit("agent_panel_opened", task, {})
                    for _ in range(int(rng.integers(1, 4))):
                        emit("agent_invoked", task, {})
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
                    if rng.random() < 0.3:
                        emit("agent_suggestion_accepted", task, {})
                    if rng.random() < 0.15:
                        emit("agent_action_undone", task, {})

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
        for item, text in (
            ("open_useful", "The assistant found the right card quickly."),
            ("open_frustrating", "I had to re-check what it changed."),
            ("open_change", "Show me what it is about to do first."),
            ("open_integration", "Yes, the chat-to-card links helped."),
        ):
            s_rows.append([code, cond, item, text, clock.isoformat()])

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

    print(f"Wrote synthetic exports for {n} participants to {out_dir}/")
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
    a = ap.parse_args()
    generate(a.out, a.n, a.seed)
