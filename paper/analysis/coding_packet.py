"""
Build the blinded packet the correctness coders work from, and merge their
scores back into `correctness.csv`.

Blinding is *mandatory* under the plan (`../WELD_ANALYSIS_PLAN.tex`, "Scoring
procedure and reliability"): a coder who knows a participant was in C3 while
judging their output is a direct route to confirming H1. The plan calls blinding
cheap because "the export makes it so" — but the export does the opposite. Every
row of `study_events.csv` carries `agent_condition`, plus timestamps and the
agent's own transcript. Handing a coder that file unblinds them on line one.

So this script produces what the plan actually describes: final state only, with
condition, timing and agent transcript stripped.

    # 1. researcher generates the packet
    python coding_packet.py --data data --out coding

    # 2. two coders independently fill in coding/coder_1.csv and coder_2.csv

    # 3. researcher merges; disagreements are reported for adjudication
    python coding_packet.py --merge coding/coder_1.csv coding/coder_2.csv \\
        --out data/correctness.csv

Kappa is computed pre-adjudication from the two coder columns, so the merge
keeps both even after you settle the consensus score.

**The coded set is capped, and the cap is a pre-registered rule.** Recruitment is
self-serve and unproctored, so collecting a participant costs nothing and every
instrumented measure -- time, TLX, the survey, the whole telemetry spine -- scales
to any N for free. Hand coding does not: it is three codings, all double-coded,
all linear in N, and it is the entire researcher cost of the study. So the
*collected* sample stays open and the *coded* sample is capped at the first
`--cap` participants per condition in enrolment order (`coded_set` below). See
`../WELD_ANALYSIS_PLAN.tex`, "The coded set is capped by enrolment order".
"""

from __future__ import annotations

import argparse
import json
import random
import re
import sys
from pathlib import Path

import numpy as np
import pandas as pd

from load import load

#: Task order as administered. Kept explicit rather than derived from the data so
#: a participant who skipped a task still produces a row the coders can score 0.
BASELINE_TASK_IDS = [f"B{i}" for i in range(1, 7)]
AGENT_TASK_IDS = [f"T{i}" for i in range(1, 7)]
TASK_IDS = BASELINE_TASK_IDS + AGENT_TASK_IDS

#: Which tasks a correctness packet covers.
#:
#: `agent` exists because the correctness *models* only ever fit on agent-phase
#: tasks -- the baseline scores feed the balance check and the kappa sample, and
#: nothing else. Coding agent tasks alone halves the largest of the three codings
#: (12N items to 6N), which matters when two people are carrying all three.
#:
#: `baseline` is the other half, so that decision stays reversible: code the
#: agent tasks now, and if a reviewer asks about manual-task accuracy later,
#: build the baseline packet then and merge it into the same file. The export
#: retains `task_final_state` for all twelve tasks, so nothing is lost by
#: deferring -- only by never doing it.
PHASE_TASKS = {
    "all": TASK_IDS,
    "agent": AGENT_TASK_IDS,
    "baseline": BASELINE_TASK_IDS,
}

#: Participants per condition that enter the human-coded set. Pre-registered, and
#: chosen so the coded set lands at the top of the design's own target range
#: (20--30) whatever recruitment delivers above it. `0` disables the cap.
DEFAULT_CAP = 10

#: The file that records who the cap selected. Named for what it is: it carries
#: `agent_condition`, so handing it to a coder unblinds the whole round.
MANIFEST_NAME = "RESEARCHER_ONLY_CODED_SET.csv"

#: The rubric doc, parsed at generation time rather than copied here — a second
#: copy of the anchors is a second thing to forget to update.
RUBRIC_DOC = Path(__file__).resolve().parent.parent / "replication" / "WELD_AGENT_TASK_SCRIPTS.md"

SCALE = """\
| Score | Meaning |
|---|---|
| 0 | Not attempted / no meaningful progress |
| 1 | Attempted, wrong outcome (e.g. moved the wrong card, posted irrelevant text) |
| 2 | Partially correct (right idea, incomplete) |
| 3 | Fully correct |
"""


def load_anchors() -> dict[str, str]:
    """Pull the per-task "what a 3 requires" anchors out of the rubric doc."""
    if not RUBRIC_DOC.exists():
        print(f"warning: rubric not found at {RUBRIC_DOC}; packet will omit anchors",
              file=sys.stderr)
        return {}
    pattern = re.compile(r"^\s*-\s*\*\*(B[1-6]|T[1-6]):\*\*\s*(.+?)\s*$", re.MULTILINE)
    anchors = dict(pattern.findall(RUBRIC_DOC.read_text(encoding="utf-8")))
    missing = [t for t in TASK_IDS if t not in anchors]
    if missing:
        print(f"warning: no rubric anchor parsed for {missing}", file=sys.stderr)
    return anchors


def coded_set(data, cap: int = DEFAULT_CAP) -> pd.DataFrame:
    """Which participants enter the human-coded set: first `cap` per condition.

    The problem this solves is that recruitment scales and coding does not.
    Sessions are self-serve, so the marginal cost of one more participant is
    zero and every instrumented measure -- including the *primary* outcome,
    baseline-adjusted task time -- gets better for free. The three human codings
    cost roughly an hour and a half of two people's time per participant and buy
    precision only as sqrt(N). Capping recruitment to protect the coding budget
    would throw away the free half of the study to save the expensive half.

    So the cap goes on the coded set, and the selection rule has to be one that
    cannot be steered. Three properties make this one defensible:

    * **Enrolment order is outcome-independent.** It is fixed at provisioning,
      before the participant has done anything, so nothing about how a session
      went can move someone in or out of the coded set. Selecting on completion,
      on data quality, or on anything measured after the fact would all fail
      this and would all be indistinguishable from choosing the sample after
      seeing it.
    * **It is balanced by construction.** The cap is applied *within* condition,
      so the coded set has equal cells even if recruitment does not. And because
      condition is assigned by adaptive minimisation -- each participant goes to
      the least-filled arm (`provision.rs::pick_agent_condition`) -- enrolment
      order is already deliberately decorrelated from condition, so taking a
      prefix of it cannot load one arm with early or late enrollees.
    * **What it costs is stated, not hidden.** Early and late enrollees may
      differ (keener, or reached through a different channel), so the coded set
      is representative of the collected set only up to that drift. The drift is
      the same in all three arms, so it does not bias the between-condition
      contrasts -- and `analyze.coded_set_comparison` tests it directly on the
      instrumented measures rather than asserting it.

    Eligibility is having produced at least one agent-phase final state --
    presence of data, not quality of it -- so the rule stays outcome-independent
    while not spending cap slots on people who consented and never started. It
    is deliberately the *same* eligibility for all three codings, so correctness,
    agent errors and themes cover one set of people rather than three.

    Returns the full manifest, coded and uncoded alike, so the decision is
    auditable after the fact rather than implicit in which files exist.
    """
    participants = data.participants
    finals = data.events[data.events["event_type"] == "task_final_state"]
    with_data = set(
        finals[finals["current_task_id"].isin(AGENT_TASK_IDS)]["participant_code"]
    )

    m = participants[["participant_code", "agent_condition", "status",
                      "provisioned_at"]].copy()
    m["eligible"] = m["participant_code"].isin(with_data)
    # Tie-break on the code so two participants provisioned in the same second
    # order identically on every machine and every re-run.
    m = m.sort_values(["provisioned_at", "participant_code"], kind="mergesort")
    m["rank_in_condition"] = (
        m[m["eligible"]].groupby("agent_condition").cumcount() + 1
    )
    m["coded"] = m["eligible"] & (
        True if cap <= 0 else m["rank_in_condition"] <= cap
    )
    m["reason"] = np.where(
        ~m["eligible"], "no agent-phase data",
        np.where(m["coded"], "within cap", "past cap"),
    )
    return m.reset_index(drop=True)


def _apply_cap(data, out_dir: Path, cap: int) -> set[str]:
    """Resolve the coded set, write the manifest, and report what it dropped.

    Every packet builder routes through here so the three codings cannot drift
    onto different participants -- which would make the agent-error and theme
    results describe a different sample from the correctness results, for no
    reason a reader could see.
    """
    manifest = coded_set(data, cap)
    out_dir.mkdir(parents=True, exist_ok=True)
    manifest.to_csv(out_dir / MANIFEST_NAME, index=False)

    codes = set(manifest[manifest["coded"]]["participant_code"])
    dropped = int((~manifest["coded"] & manifest["eligible"]).sum())
    per_cond = manifest[manifest["coded"]].groupby("agent_condition").size()
    print(f"Coded set: {len(codes)} participant(s) "
          f"({', '.join(f'{c} {n}' for c, n in per_cond.items()) or 'none'})"
          + (f"; {dropped} eligible participant(s) past the cap of {cap}/condition"
             if dropped else ""))
    print(f"Wrote {out_dir/MANIFEST_NAME} — RESEARCHER ONLY. It names each")
    print("  participant's condition; do not send it to a coder.")
    return codes


def _render_board(board: dict) -> str:
    """Cards grouped by column, with everything an anchor can turn on."""
    if not isinstance(board, dict):
        return "  (no board snapshot)\n"
    columns = [c.get("name", "?") for c in board.get("columns", [])]
    tasks = board.get("tasks", [])
    out = []
    for col in columns:
        in_col = [t for t in tasks if t.get("column") == col]
        out.append(f"  **{col}** ({len(in_col)})")
        for t in sorted(in_col, key=lambda x: str(x.get("short_id", ""))):
            bits = [f"priority: {t.get('priority') or 'None'}"]
            if t.get("github_pr_id"):
                bits.append(f"PR #{t['github_pr_id']}")
            if t.get("github_issue_id"):
                bits.append(f"issue #{t['github_issue_id']}")
            if t.get("labels"):
                bits.append(f"{len(t['labels'])} label(s)")
            out.append(f"    - [{t.get('short_id')}] {t.get('title')} — {', '.join(bits)}")
        if not in_col:
            out.append("    - (empty)")
    return "\n".join(out) + "\n"


CARD_REF = re.compile(r"\[\[card:([0-9a-fA-F-]{36})\]\]")


def _card_titles(board: object) -> dict[str, str]:
    """Card id → the label a coder can actually match against the rubric."""
    if not isinstance(board, dict):
        return {}
    return {
        str(t.get("id")): f"[{t.get('short_id')}] {t.get('title')}"
        for t in board.get("tasks", [])
        if t.get("id")
    }


def _resolve_card_refs(text: str, titles: dict[str, str]) -> str:
    """Turn `[[card:uuid]]` into the card's title.

    The app renders these as clickable chips, so a participant reading the chat
    saw a card name. Left raw, a coder sees a UUID — and several anchors turn on
    *which* card a teammate named ("the specific card Priya named"), which makes
    the reference the whole point of the item.
    """
    return CARD_REF.sub(
        lambda m: f"«{titles.get(m.group(1), 'unknown card ' + m.group(1)[:8])}»", text
    )


def _render_chat(chat: object, participant_user_id: str | None,
                 card_titles: dict[str, str] | None = None) -> str:
    """Channel transcripts, with the participant's own messages marked.

    Several anchors turn on whether the *participant* posted something, so that
    distinction has to survive; the sender's identity beyond that does not, and
    timestamps are dropped outright — they are timing data.
    """
    if not isinstance(chat, list):
        return "  (no chat snapshot)\n"
    out = []
    for channel in chat:
        msgs = channel.get("messages", [])
        out.append(f"  **#{channel.get('channel', '?')}** ({len(msgs)} messages)")
        for m in msgs:
            who = "PARTICIPANT" if (
                participant_user_id and m.get("sender_id") == participant_user_id
            ) else "teammate"
            content = " ".join(str(m.get("content", "")).split())
            content = _resolve_card_refs(content, card_titles or {})
            if len(content) > 400:
                content = content[:400] + " […]"
            out.append(f"    - [{who}] {content}")
        if not msgs:
            out.append("    - (empty)")
    return "\n".join(out) + "\n"


def build_packet(data_dir: Path, out_dir: Path, seed: int, phase: str = "agent",
                 cap: int = DEFAULT_CAP) -> int:
    """Build the blinded correctness packet for `phase` (agent / baseline / all).

    Splitting by phase lets the agent tasks -- the only ones any correctness
    model reads -- be coded first, and the baselines added later or not at all.
    That is why `agent` is the default: the baseline scores feed the balance
    check and the kappa sample and nothing else, so coding all twelve tasks
    doubles the largest job to strengthen a check that also runs on instrumented
    time and effort. Files are suffixed by phase so a second round cannot
    overwrite the first round's coder CSVs, and `merge_coders` combines rounds
    into one `correctness.csv` by replacing the keys it covers and leaving the
    rest alone.
    """
    if phase not in PHASE_TASKS:
        raise SystemExit(f"unknown phase '{phase}' (expected {'/'.join(PHASE_TASKS)})")
    task_ids = PHASE_TASKS[phase]
    suffix = "" if phase == "all" else f"_{phase}"
    data = load(data_dir)
    anchors = load_anchors()
    codes = _apply_cap(data, out_dir, cap)

    user_ids = {}
    if "user_id" in data.participants.columns:
        user_ids = dict(
            zip(data.participants["participant_code"], data.participants["user_id"].astype(str))
        )

    events = data.events
    finals = events[events["event_type"] == "task_final_state"]
    reviews = events[events["event_type"] == "pr_review_submitted"]

    items = []
    for code in sorted(codes):
        for task_id in task_ids:
            rows = finals[
                (finals["participant_code"] == code) & (finals["current_task_id"] == task_id)
            ]
            # Last snapshot wins: a task re-entered after a resume emits more
            # than one, and the final one is the state they finished on.
            payload = rows.iloc[-1]["payload"] if len(rows) else {}
            rv = reviews[
                (reviews["participant_code"] == code) & (reviews["current_task_id"] == task_id)
            ]
            items.append({
                "participant_code": code,
                "task_id": task_id,
                "payload": payload,
                "reviews": [r for r in rv["payload"]],
                "missing": len(rows) == 0,
            })

    # Shuffle presentation so coders don't work through one participant's whole
    # session in order, which invites carrying an impression of "this person" from
    # task to task. Seeded, so the packet is reproducible.
    random.Random(seed).shuffle(items)

    out_dir.mkdir(parents=True, exist_ok=True)
    lines: list[str] = []
    add = lines.append
    add("# Correctness coding packet")
    add("")
    if phase != "all":
        # Named, not hidden. A coder who knows this round is "the agent ones"
        # learns nothing about any individual item -- every item is still shown
        # without its condition -- but a round that silently covered half the
        # tasks would be impossible to reconcile against the plan later.
        which = "agent-phase" if phase == "agent" else "manual baseline"
        add(f"**This round covers the {which} tasks only** "
            f"({len(task_ids)} per participant).")
        add("")
    add("Score each item **0–3** on the final state the participant produced. It does")
    add("not matter whether the participant or an assistant produced it — score the")
    add("state, not the route to it.")
    add("")
    add("You are deliberately not told which condition anyone was in, how long")
    add("anything took, or what any assistant said. That is the point: knowing it")
    add("would bias the scores toward the study's own hypothesis. Please don't go")
    add("looking for it.")
    add("")
    add("Work alone — the second coder is scoring the same items independently, and")
    add("the agreement between you is a reported result. Disagreements get resolved")
    add("afterwards, together.")
    add("")
    add("Put your scores in the `score` column of your own CSV")
    add("(`coder_1.csv` or `coder_2.csv`), matching on participant + task.")
    add("")
    add(SCALE)
    add(f"Items are in a shuffled order (seed {seed}); item numbers are just labels.")
    add("")
    add("---")
    add("")

    for n, item in enumerate(items, start=1):
        add(f"## Item {n:03d} — participant {item['participant_code']}, task {item['task_id']}")
        add("")
        if item["task_id"] in anchors:
            add(f"**A 3 requires:** {anchors[item['task_id']]}")
            add("")
        if item["missing"]:
            add("> **No final-state snapshot was recorded for this task.** The task was")
            add("> skipped or abandoned. Score 0 unless the board or chat below shows")
            add("> otherwise.")
            add("")
        payload = item["payload"] if isinstance(item["payload"], dict) else {}
        add("**Board**")
        add("")
        add(_render_board(payload.get("board", {})))
        add("**Chat**")
        add("")
        add(_render_chat(payload.get("chat", []), user_ids.get(item["participant_code"]),
                         _card_titles(payload.get("board", {}))))
        if item["reviews"]:
            add("**Pull-request review submitted in-app**")
            add("")
            for r in item["reviews"]:
                comment = " ".join(str(r.get("comment") or "").split()) or "(no comment)"
                add(f"  - PR #{r.get('number')} — verdict: {r.get('verdict')} — {comment}")
            add("")
        add("---")
        add("")

    packet_path = out_dir / f"CODING_PACKET{suffix}.md"
    packet_path.write_text("\n".join(lines), encoding="utf-8")

    template = pd.DataFrame(
        [{"participant_code": i["participant_code"], "task_id": i["task_id"], "score": ""}
         for i in items]
    ).sort_values(["participant_code", "task_id"])
    for coder in ("coder_1", "coder_2"):
        template.to_csv(out_dir / f"{coder}{suffix}.csv", index=False)

    print(f"Wrote {packet_path} — {len(items)} items ({phase} phase)")
    print(f"Wrote {out_dir/f'coder_1{suffix}.csv'} and "
          f"{out_dir/f'coder_2{suffix}.csv'} (blank `score` column)")
    if phase == "agent":
        print("\nThis is the half the correctness models actually read. To add the")
        print("baselines later: --phase baseline, then merge into the same")
        print("correctness.csv — the merge keeps rows it does not cover.")
    return len(items)


# --- Agent-error coding (H4) ------------------------------------------------

#: The seed every participant's workspace is cloned from. It is small and fixed
#: -- 12 cards, 18 messages, 3 issues, 3 pull requests -- which is what makes
#: most agent errors checkable rather than a matter of taste.
SEED_DOC = (
    Path(__file__).resolve().parents[2]
    / "crates" / "weld-server" / "study" / "pettracker_seed.json"
)

AGENT_ERROR_SCALE = """\
Two independent yes/no judgements per item. An item can be neither, either, or
both.

| Column | Ask yourself |
|---|---|
| `factual_error` | Did the assistant state something the workspace contradicts? Check it against `ANSWER_KEY.md`. 1 = yes, 0 = no. |
| `judgement_error` | Setting facts aside, was the advice or action wrong for the task — unhelpful, irrelevant, or the wrong move? 1 = yes, 0 = no. |

Leave **both blank** if the assistant said nothing substantive, or declined, or
said it could not see something. Declining to answer is not an error.

`factual_error` is the one that carries the analysis, and it is the one you can
settle by looking something up rather than by forming an opinion. When in doubt
about `judgement_error`, code 0 — it is the softer measure and is reported
separately.
"""


def _render_answer_key() -> str:
    """The ground truth a coder checks factual claims against.

    Every participant's workspace is a clone of one fixed seed, so "PR #34 is the
    vaccination-reminder one" is a fact with an answer, not an impression. Giving
    coders the key turns most of `factual_error` into a lookup, which is both
    faster and far more reliable than asking two people to recall a 12-card board
    from memory across a hundred-odd items.
    """
    if not SEED_DOC.exists():
        print(f"warning: seed not found at {SEED_DOC}; packet will omit the answer key",
              file=sys.stderr)
        return ""
    seed = json.loads(SEED_DOC.read_text(encoding="utf-8"))
    columns = {c["id"]: c["name"] for c in seed.get("columns", [])}
    people = {t["id"]: t.get("display_name", t["id"]) for t in seed.get("teammates", [])}
    channels = {c["id"]: c["name"] for c in seed.get("channels", [])}

    out = ["# Answer key — the workspace every participant started from", ""]
    out.append("Each participant got an identical copy of this. Use it to settle")
    out.append("`factual_error`: if the assistant said something this contradicts,")
    out.append("that is a factual error, whatever else was going on.")
    out.append("")
    out.append("Participants changed things during their session, so a card being")
    out.append("somewhere else by the end is not itself a contradiction — judge")
    out.append("claims about what *was there*, names, numbers, and who said what.")
    out.append("")

    out.append("## People")
    out.append("")
    for t in seed.get("teammates", []):
        out.append(f"- **{t.get('display_name')}** — {t.get('role')}. {t.get('blurb', '')}")
    out.append("")

    out.append("## Board — starting state")
    out.append("")
    for col in seed.get("columns", []):
        cards = [c for c in seed.get("cards", []) if c.get("column_id") == col["id"]]
        out.append(f"**{col['name']}** ({len(cards)})")
        for c in cards:
            desc = " ".join(str(c.get("description", "")).split())
            if len(desc) > 180:
                desc = desc[:180] + " […]"
            out.append(f"- *{c.get('title')}* — {desc}")
        out.append("")

    # Seeded messages reference cards as `[[card:seed_id]]`. The app renders those
    # as the card's name, so that is what a participant read and what the
    # assistant's claims are about; leaving the raw id makes several messages
    # unusable as ground truth.
    seed_card_titles = {c["id"]: c.get("title", c["id"]) for c in seed.get("cards", [])}
    seed_ref = re.compile(r"\[\[card:([A-Za-z0-9_-]+)\]\]")

    out.append("## Channels and who is in the conversation")
    out.append("")
    for ch in seed.get("channels", []):
        msgs = [m for m in seed.get("messages", []) if m.get("channel_id") == ch["id"]]
        out.append(f"**#{ch['name']}** ({len(msgs)} seeded messages)")
        for m in msgs:
            who = people.get(m.get("sender_id"), m.get("sender_id"))
            content = " ".join(str(m.get("content", "")).split())
            content = seed_ref.sub(
                lambda mo: f"«{seed_card_titles.get(mo.group(1), mo.group(1))}»", content
            )
            if len(content) > 260:
                content = content[:260] + " […]"
            out.append(f"- {who}: {content}")
        out.append("")

    gh = seed.get("github", {})
    # `repo` is an object in the seed, not a name; printing it raw dumps the
    # README excerpt into the heading.
    repo = gh.get("repo") or {}
    repo_name = repo.get("full_name", "repository") if isinstance(repo, dict) else str(repo)
    out.append(f"## GitHub — `{repo_name}`")
    out.append("")
    if isinstance(repo, dict) and repo.get("description"):
        out.append(f"{repo['description']}")
        out.append("")
    out.append("### Issues")
    for i in gh.get("issues", []):
        out.append(f"- **#{i.get('number')}** ({i.get('state')}) {i.get('title')}")
    out.append("")
    out.append("### Pull requests")
    for pr in gh.get("pull_requests", []):
        body = " ".join(str(pr.get("body", "")).split())
        if len(body) > 260:
            body = body[:260] + " […]"
        files = ", ".join(f.get("filename", "?") for f in pr.get("files", []))
        out.append(f"- **#{pr.get('number')}** ({pr.get('state')}) *{pr.get('title')}*")
        out.append(f"  - by {people.get(pr.get('author'), pr.get('author'))}; touches {files or '(no files listed)'}")
        out.append(f"  - {body}")
    out.append("")
    _ = columns, channels  # named above for readability; rendering uses them inline
    return "\n".join(out)


def _turns_without_the_reaction(rows: list[list[object]]) -> list[list[object]]:
    """Drop participant turns that come after the last assistant turn.

    This is the load-bearing blinding rule for this packet, and it is not the one
    a first pass would guess. Condition leakage does not matter much here --
    condition is not the contrast in the calibration test, which compares override
    rates *between* strata of agent correctness. What must not leak is the
    **outcome**: whether the participant overrode the agent. A coder who reads
    "no, that's the wrong PR, undo it" and concludes the agent was wrong has
    inferred the stratifying variable from the very behaviour the test predicts,
    and the result becomes circular -- H4 would confirm itself out of nothing.

    Trailing participant turns are where that reaction almost always lands, and
    dropping them costs no context: nothing after the last assistant turn informs
    whether the assistant was right. Residual, stated rather than solved: an
    intermediate participant turn can also react to an earlier assistant turn.
    The packet tells coders to judge assertions against the answer key and not
    against how the participant responded, which is a weaker guarantee than
    removal, and the limitation is reported.
    """
    last_assistant = max(
        (i for i, r in enumerate(rows) if r[0] == "assistant"), default=None
    )
    # No assistant turn means the participant asked and got nothing back: there
    # is no assistant output to judge, so the item is dropped rather than shipped
    # as a page of participant text with nothing to code against it.
    return [] if last_assistant is None else rows[: last_assistant + 1]


def build_agent_error_packet(data_dir: Path, out_dir: Path, seed: int,
                             cap: int = DEFAULT_CAP) -> int:
    """The packet for coding whether *the assistant* was right, per task.

    This is a different judgement from correctness, and H4 needs it. Correctness
    scores the state the participant finished on; a task can end fully correct
    because the participant caught and fixed a wrong suggestion, which is exactly
    the case H4 is about. Only a judgement about the assistant's own output
    separates "they overrode because it was wrong" from "they overrode anyway".

    **This packet cannot be condition-blind, and we do not pretend otherwise.**
    The transcript is the evidence, and the transcript gives the condition away:
    a C1 assistant never acts, a C3 one talks about pull requests. Stripping that
    would remove the thing being coded. Two consequences, both stated in the
    packet itself and in the paper: the correctness measure stays in its own
    separate blinded packet and is coded first, so the load-bearing measure is
    never contaminated by this one; and the agent-error coding is reported as
    what it is -- an unblinded secondary coding supporting an exploratory
    hypothesis, not a confirmatory result.
    """
    data = load(data_dir)
    turns = data.agent_turns
    if turns.empty:
        raise SystemExit(
            "No study_agent_turns.csv in "
            f"{data_dir}. Pull it with `make export` (dataset=agent_turns); "
            "without it H4's calibration test falls back to outcome correctness."
        )
    anchors = load_anchors()
    codes = _apply_cap(data, out_dir, cap)

    items = []
    for (code, task_id), grp in turns.groupby(["participant_code", "task_id"]):
        if task_id not in TASK_IDS or code not in codes:
            continue
        ordered = grp.sort_values("turn_index")
        rows = _turns_without_the_reaction(ordered[["role", "content"]].values.tolist())
        if not rows:
            continue
        items.append({"participant_code": code, "task_id": task_id, "turns": rows})
    random.Random(seed).shuffle(items)

    out_dir.mkdir(parents=True, exist_ok=True)
    lines: list[str] = []
    add = lines.append
    add("# Agent-error coding packet")
    add("")
    add("For each item, judge **whether the assistant was right** — not whether the")
    add("participant ended up right. Those come apart, and the difference is the")
    add("whole point of this coding: someone who spots a wrong suggestion and fixes")
    add("it produces a correct outcome from a wrong assistant.")
    add("")
    add("Judge only what the assistant asserted or did. If it declined, said it")
    add("could not see something, or offered nothing substantive, leave the codes")
    add("blank rather than scoring it wrong — refusing to answer is not an error.")
    add("")
    add("**Check claims against `ANSWER_KEY.md`, not against the participant.**")
    add("Every participant started from the same workspace and the key describes it,")
    add("so most factual questions have a lookup answer. Where a participant's own")
    add("message appears to disagree with the assistant, that is not evidence: they")
    add("could be mistaken, and reading their reaction as the answer would make this")
    add("coding circular with the analysis it feeds.")
    add("")
    add("Conversations here end on the assistant's last turn — anything the")
    add("participant said afterwards has been removed on purpose, for the reason")
    add("just given.")
    add("")
    add("Unlike the correctness packet, this one **does** show you the assistant's")
    add("side of the conversation, so you may be able to infer how much of the")
    add("workspace it could see. That is unavoidable — the transcript is the")
    add("evidence — and it matters less here than it would there. Please don't")
    add("reason from it anyway.")
    add("")
    add("Work alone; the second coder is coding the same items independently.")
    add("")
    add(AGENT_ERROR_SCALE)
    add(f"Items are in a shuffled order (seed {seed}); item numbers are just labels.")
    add("")
    add("---")
    add("")
    for n, item in enumerate(items, start=1):
        add(f"## Item {n:03d} — participant {item['participant_code']}, "
            f"task {item['task_id']}")
        add("")
        if item["task_id"] in anchors:
            add(f"**The task required:** {anchors[item['task_id']]}")
            add("")
        for role, content in item["turns"]:
            who = "PARTICIPANT" if role == "user" else "ASSISTANT"
            text = " ".join(str(content).split())
            if len(text) > 1200:
                text = text[:1200] + " […]"
            add(f"- **{who}:** {text}")
        add("")
        add("---")
        add("")
    (out_dir / "AGENT_ERROR_PACKET.md").write_text("\n".join(lines), encoding="utf-8")

    key = _render_answer_key()
    if key:
        (out_dir / "ANSWER_KEY.md").write_text(key, encoding="utf-8")

    template = pd.DataFrame(
        [{"participant_code": i["participant_code"], "task_id": i["task_id"],
          "factual_error": "", "judgement_error": ""} for i in items]
    ).sort_values(["participant_code", "task_id"])
    for coder in ("coder_1", "coder_2"):
        template.to_csv(out_dir / f"agent_error_{coder}.csv", index=False)

    print(f"Wrote {out_dir/'AGENT_ERROR_PACKET.md'} — {len(items)} items")
    if key:
        print(f"Wrote {out_dir/'ANSWER_KEY.md'} — send it with the packet")
    print(f"Wrote {out_dir/'agent_error_coder_1.csv'} and "
          f"{out_dir/'agent_error_coder_2.csv'} "
          "(blank `factual_error` / `judgement_error` columns)")
    print("\nNOTE: this packet is deliberately NOT condition-blind (see the module")
    print("docstring). Code the correctness packet first and separately.")
    return len(items)


# --- Qualitative coding (RQ2 open-ended items) ------------------------------

#: The four free-text items on the exit survey (apps/web/src/lib/survey-config.ts).
OPEN_ITEMS = ["open_useful", "open_frustrating", "open_change", "open_integration"]

STARTER_CODEBOOK = """\
# Theme codebook — open-ended items

A starting point, not a finished scheme. Thematic analysis builds its categories
from the data: **one coder reads all responses first and revises this list**
(merging, splitting, renaming, adding), then both coders code every response
against the revised list. Kappa is only meaningful once the list is fixed, so do
not change it after coding starts.

Assign **one primary theme** per response — the thing the answer is mostly
about. That constraint is what makes Cohen's kappa well-defined; a secondary
impression goes in `notes`, which is not scored.

| Theme | Use when the response is mostly about |
|---|---|
| `agent_helpful` | The assistant saved work, found something, or did a step well |
| `agent_wrong` | The assistant was mistaken, misunderstood, or acted wrongly |
| `agent_limited` | The assistant could not see or do something it was asked to |
| `agent_trust` | Whether they trusted it, checked it, or felt they had to |
| `integration_noticed` | Board / chat / GitHub being connected, positively or not |
| `context_switching` | Moving between surfaces, losing or keeping their place |
| `ui_usability` | Layout, discoverability, controls — the workspace itself |
| `task_artificial` | The study tasks or scenario feeling contrived |
| `other` | Anything that fits nothing above (add a note) |
| `none` | Blank, "n/a", or no substantive content |
"""


def _reliability_subset(items: list[dict], frac: float, seed: int) -> list[dict]:
    """The share of responses the second coder also codes, stratified by item.

    Stratified rather than a flat random draw because the four questions provoke
    different kinds of answer -- "what frustrated you" and "did you notice the
    integration" do not share a vocabulary -- and a subset that happened to miss
    one of them would report an agreement figure for a codebook that was never
    tested on a quarter of its material.
    """
    rng = random.Random(seed)
    chosen: list[dict] = []
    for item_id in OPEN_ITEMS:
        pool = [i for i in items if i["item_id"] == item_id]
        if not pool:
            continue
        # At least one per item, so no question goes untested for agreement.
        k = max(1, round(frac * len(pool)))
        chosen.extend(rng.sample(pool, min(k, len(pool))))
    return chosen


def build_qualitative_packet(data_dir: Path, out_dir: Path, seed: int,
                             cap: int = DEFAULT_CAP,
                             reliability_frac: float = 0.25) -> int:
    """The packet for thematically coding the four open-ended survey items.

    The plan, the paper's Methods and the paper's Results all promise this and
    none of them could be honoured: unlike correctness, there was no packet, no
    codebook and no agreement measure. This is the missing instrument.

    Responses are shuffled across participants and items so a coder does not read
    one person's four answers in a row. Condition is not shown. It cannot be
    fully hidden -- someone may write "it linked the PR for me", which only a
    C3 assistant could do -- and that residual is stated rather than papered
    over; the qualitative findings are illustrative, not confirmatory.

    **This is the one coding that is not fully double-coded, and that follows
    from the sentence above.** Coder 1 codes every response; coder 2 codes a
    stratified `reliability_frac` of them, and kappa is computed on that overlap.
    Correctness and agent error stay fully double-coded because they feed
    confirmatory and pre-specified tests; the themes are labelled illustrative in
    the plan and in the paper, and a reliability subset with a reported kappa is
    the standard instrument for exactly that status. It is also where the saving
    is largest per unit of evidence lost: four responses per participant is the
    coding that grows fastest with N and the only one whose result is not a
    number in a model.

    What must not happen is the saving being taken silently. The overlap size is
    written into the packet, `merge_coders` reports coverage, and
    `analyze.nominal_agreement` computes kappa on the double-coded rows alone --
    filling the single-coded rows from coder 1 and then computing agreement over
    all of them would report a kappa inflated by rows only one person ever saw.
    """
    data = load(data_dir)
    survey = data.survey
    if survey.empty:
        raise SystemExit(f"No survey rows in {data_dir}.")
    codes = _apply_cap(data, out_dir, cap)
    open_rows = survey[survey["item_id"].isin(OPEN_ITEMS)].copy()
    open_rows = open_rows[open_rows["participant_code"].isin(codes)]
    open_rows["value"] = open_rows["value"].fillna("").astype(str)
    open_rows = open_rows[open_rows["value"].str.strip() != ""]
    if open_rows.empty:
        raise SystemExit(
            "No non-empty open-ended responses found. The four items are optional, "
            "so this is possible -- but check the export before concluding it."
        )

    items = [
        {"participant_code": r["participant_code"], "item_id": r["item_id"],
         "text": " ".join(str(r["value"]).split())}
        for _, r in open_rows.iterrows()
    ]
    random.Random(seed).shuffle(items)
    subset = _reliability_subset(items, reliability_frac, seed)

    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "THEME_CODEBOOK.md").write_text(STARTER_CODEBOOK, encoding="utf-8")

    prompts = {
        "open_useful": "What was the most useful moment of working in Weld today?",
        "open_frustrating": "What was the most frustrating moment?",
        "open_change": "If you could change one thing about Weld, what would it be?",
        "open_integration": "Did you notice the board, chat, and GitHub being "
                            "connected? If so, was that helpful, distracting, or neither?",
    }

    def render(rows: list[dict], role: str) -> str:
        lines: list[str] = []
        add = lines.append
        add("# Qualitative coding packet — open-ended responses")
        add("")
        add(role)
        add("")
        add("Assign **one primary theme** to each response, from `THEME_CODEBOOK.md`.")
        add("Put it in the `theme` column of your own CSV, matching on participant +")
        add("item. Anything else you noticed goes in `notes`, which is not scored.")
        add("")
        add("Read the codebook first. If it does not fit the data, say so before you")
        add("start coding — it is meant to be revised once, then frozen.")
        add("")
        add("You are not told which condition anyone was in. Some responses may still")
        add("hint at it by describing what the assistant could do. Please don't reason")
        add("from that; code what the person said about their experience.")
        add("")
        add(f"Responses are shuffled across participants and items (seed {seed}).")
        add("")
        add("---")
        add("")
        for n, item in enumerate(rows, start=1):
            add(f"## Item {n:03d} — participant {item['participant_code']}, "
                f"`{item['item_id']}`")
            add("")
            add(f"> **Question asked:** {prompts.get(item['item_id'], item['item_id'])}")
            add("")
            add(f"{item['text']}")
            add("")
            add("---")
            add("")
        return "\n".join(lines)

    # Coder 1 gets the corpus; coder 2 gets the reliability subset. Both are told
    # which they hold, because a coder who believes they are the only reader
    # codes differently from one who knows their work is being compared -- and
    # the second coder in particular must not think the missing items were an
    # oversight and go looking for them.
    (out_dir / "QUALITATIVE_PACKET.md").write_text(
        render(items,
               f"**You are coder 1: you code all {len(items)} responses.** A second "
               f"coder independently codes {len(subset)} of them, drawn across all "
               "four questions, and the agreement between you on those is reported "
               "as the reliability of this coding."),
        encoding="utf-8")
    (out_dir / "QUALITATIVE_PACKET_reliability.md").write_text(
        render(subset,
               f"**You are coder 2: you code this {len(subset)}-response reliability "
               f"sample.** Coder 1 is independently coding these and "
               f"{len(items) - len(subset)} more. Your agreement with them on these "
               "items is the reported reliability of the whole coding, so code them "
               "exactly as you would code the full set — do not go easier or harder "
               "because the sample is small."),
        encoding="utf-8")

    def template(rows: list[dict]) -> pd.DataFrame:
        return pd.DataFrame(
            [{"participant_code": i["participant_code"], "item_id": i["item_id"],
              "theme": "", "notes": ""} for i in rows]
        ).sort_values(["participant_code", "item_id"])

    template(items).to_csv(out_dir / "theme_coder_1.csv", index=False)
    template(subset).to_csv(out_dir / "theme_coder_2.csv", index=False)

    pct = 100.0 * len(subset) / len(items)
    print(f"Wrote {out_dir/'QUALITATIVE_PACKET.md'} — {len(items)} responses (coder 1)")
    print(f"Wrote {out_dir/'QUALITATIVE_PACKET_reliability.md'} — {len(subset)} "
          f"responses ({pct:.0f}%, coder 2)")
    print(f"Wrote {out_dir/'THEME_CODEBOOK.md'} — revise it before coding starts")
    print(f"Wrote {out_dir/'theme_coder_1.csv'} and {out_dir/'theme_coder_2.csv'}")
    print("\nThemes are the one single-coded coding: they are illustrative rather")
    print("than confirmatory, so a double-coded reliability sample carries the")
    print("agreement claim. Kappa is computed on the overlap only.")
    return len(items)


#: Anything here in the packet would tell a coder which condition they are
#: looking at, or how long it took. Deliberately broad -- a false positive costs
#: one look, an unblinded coder costs the correctness measure.
LEAK_PATTERN = re.compile(
    r"\bC[123]\b|agent_condition|condition|latency|prompt_version|_ms\b",
    re.IGNORECASE,
)


def verify_blinding(packet: Path) -> list[tuple[int, str]]:
    """Lines in the packet that could unblind a coder. Empty list means clean.

    Only the item bodies are scanned. The preamble above the first item is our
    own fixed instructions, which legitimately mention conditions -- it tells
    the coder they are deliberately not being shown one. Scanning it flags that
    sentence forever, and a check that always fails gets ignored, which is worse
    than no check. A leak can only come from the generated items anyway, since
    those are the only part built from participant data.
    """
    lines = packet.read_text(encoding="utf-8").splitlines()
    first_item = next(
        (i for i, line in enumerate(lines) if line.startswith("## Item ")), None
    )
    if first_item is None:  # no items: nothing generated, nothing to leak
        return []
    return [
        (n, line.strip())
        for n, line in enumerate(lines[first_item:], first_item + 1)
        if LEAK_PATTERN.search(line)
    ]


#: What each merge mode produces: the key columns, the coders' column, and
#: whether that column is numeric. Adding a coding here is all it takes to give
#: it the same adjudication discipline as correctness.
MERGE_MODES = {
    "correctness": {
        "key": ["participant_code", "task_id"],
        "values": ["score"], "numeric": True,
    },
    "agent-errors": {
        "key": ["participant_code", "task_id"],
        # Two independent binaries. `factual_error` is checkable against the
        # answer key and is what the calibration test stratifies on;
        # `judgement_error` is the softer measure and is reported separately.
        # `agent_error` is derived from both so the analysis contract is unchanged.
        "values": ["factual_error", "judgement_error"], "numeric": True,
    },
    "themes": {
        "key": ["participant_code", "item_id"],
        "values": ["theme"], "numeric": False,
        # The one coding where coder 2 covers a subset rather than the corpus
        # (see `build_qualitative_packet`). Rows only coder 1 saw take coder 1's
        # theme as the value instead of being reported as unresolved -- but they
        # are excluded from kappa, which `nominal_agreement` gets right by
        # dropping rows with a missing coder column.
        "single_coded_ok": True,
    },
}


def merge_coders(coder_1: Path, coder_2: Path, out: Path,
                 mode: str = "correctness", adjudicator: Path | None = None) -> None:
    """Combine two coders' codings into the file the pipeline reads.

    Where they agree, that value is the consensus. Where they don't, resolution
    is never by averaging or by picking a coder -- averaging is not even
    available for themes, which is the clearest case for the rule.

    Two resolution routes, and the choice is the study's, not this script's:

    * **Discussion to consensus** (default). The value cell is left empty, the
      disputed rows are printed, and the coders settle each one and fill it in.
      Cheap, but it has a known failure mode: the more confident or more senior
      coder tends to win, and if that person knows the hypotheses, bias re-enters
      at exactly the step blinding existed to protect.
    * **Independent adjudication** (`--adjudicator`). A third coder resolves the
      disputed items blind and without discussion. That closes the failure mode
      above, and it is cheap in volume because they only see disputes. Pass their
      filled CSV and it fills the value cells directly.

    The two coders' own columns are kept in the output either way, because kappa
    is computed pre-adjudication -- afterwards the coders agree by construction
    and the number would be meaningless.
    """
    spec = MERGE_MODES[mode]
    key, values, numeric = spec["key"], spec["values"], spec["numeric"]
    single_coded_ok = spec.get("single_coded_ok", False)
    a = pd.read_csv(coder_1)
    b = pd.read_csv(coder_2)
    for name, df in (("coder_1", a), ("coder_2", b)):
        missing = set(key) - set(df.columns)
        if missing:
            raise SystemExit(f"{name} is missing column(s): {sorted(missing)}")
        for value in values:
            if value not in df.columns:
                raise SystemExit(f"{name} has no `{value}` column (mode: {mode})")

    adj = None
    if adjudicator is not None:
        adj = pd.read_csv(adjudicator).set_index(key)

    merged = a[key + values].merge(
        b[key + values], on=key, how="outer", suffixes=("_1", "_2")
    )
    out_cols = list(key)
    coder_cols: list[str] = []
    disputed_mask = pd.Series(False, index=merged.index)
    blank_mask = pd.Series(False, index=merged.index)
    summary = []

    # With one value column the coder columns keep their historic names, because
    # `correctness.csv` is documented that way and `analyze.coder_agreement`
    # reads them; only the two-column agent-error file needs the value prefix to
    # keep `factual_error` and `judgement_error` apart.
    single = len(values) == 1

    for value in values:
        c1, c2 = ("coder_1", "coder_2") if single else (f"{value}_1", f"{value}_2")
        if single:
            merged = merged.rename(columns={f"{value}_1": c1, f"{value}_2": c2})
        for c in (c1, c2):
            if numeric:
                merged[c] = pd.to_numeric(merged[c], errors="coerce")
            else:
                merged[c] = merged[c].astype("string").str.strip().replace("", pd.NA)

        both = merged[c1].notna() & merged[c2].notna()
        agree = (merged[c1] == merged[c2]).fillna(False)
        disputed = ~agree & both
        merged[value] = merged[c1].where(agree)

        if single_coded_ok:
            # Rows coder 2 was deliberately never given: there is no
            # disagreement to resolve, so coder 1's code stands. Keyed on coder 2
            # being *absent* rather than on the mode alone -- a blank cell on a
            # row they were given is an unfinished coding and must stay visible
            # as one, not be quietly resolved in coder 1's favour.
            only_1 = merged[c2].isna() & merged[c1].notna()
            merged[value] = merged[value].where(~only_1, merged[c1])

        if adj is not None and value in adj.columns:
            # Only disputes are adjudicated; agreements and deliberately
            # single-coded rows already stand, and must not be overwritten by a
            # reindex that produces NaN for keys the adjudicator never saw.
            idx = pd.MultiIndex.from_frame(merged[key])
            resolved = pd.Series(adj[value].reindex(idx).values, index=merged.index)
            if numeric:
                resolved = pd.to_numeric(resolved, errors="coerce")
            merged[value] = merged[value].where(~disputed, resolved)

        blank_mask |= merged[value].isna()
        disputed_mask |= disputed
        n_both = int(both.sum())
        summary.append((value, int((agree & both).sum()), n_both, len(merged)))
        out_cols += [value, c1, c2]
        coder_cols += [c1, c2]

    # `calibration()` and load.py read a single `agent_error`; derive it rather
    # than ask coders for a third judgement they'd have to keep consistent.
    if mode == "agent-errors":
        both = merged[["factual_error", "judgement_error"]]
        merged["agent_error"] = both.max(axis=1).where(both.notna().any(axis=1))
        out_cols = key + ["agent_error"] + out_cols[len(key):]

    out.parent.mkdir(parents=True, exist_ok=True)
    final = merged[out_cols]

    # Coding can arrive in rounds -- agent tasks now, baselines later -- and the
    # second round must not delete the first. Rows whose keys this merge covers
    # are replaced (so re-running after adjudication updates them); rows it does
    # not cover are carried through untouched. Silently clobbering a completed
    # round would be unrecoverable without re-coding it.
    carried = 0
    if out.exists():
        try:
            existing = pd.read_csv(out)
        except Exception:  # noqa: BLE001 - a corrupt file must not eat the merge
            existing = pd.DataFrame()
        if not existing.empty and set(key).issubset(existing.columns):
            covered = set(map(tuple, final[key].astype(str).values))
            keep = existing[
                ~existing[key].astype(str).apply(tuple, axis=1).isin(covered)
            ]
            carried = len(keep)
            if carried:
                final = pd.concat([keep, final], ignore_index=True)
                final = final.reindex(
                    columns=list(dict.fromkeys(list(final.columns) + out_cols))
                )
    final = final.sort_values(key)
    final.to_csv(out, index=False)

    disputed = merged[disputed_mask]
    print(f"Wrote {out} — {len(merged)} items from this round ({mode})")
    if carried:
        print(f"  carried through {carried} row(s) from an earlier round, untouched")
        print(f"  file now holds {len(final)} rows in total")
    for value, n_agree, n_both, n_rows in summary:
        pct = 100.0 * n_agree / n_both if n_both else float("nan")
        print(f"  {value}: agreed outright {n_agree} of {n_both} double-coded "
              f"({pct:.1f}%)")
        if n_both < n_rows:
            print(f"    {n_rows - n_both} row(s) single-coded — kappa is computed "
                  "on the double-coded rows only")
    print(f"  need resolution: {len(disputed)}")
    if adj is not None:
        print(f"  adjudicated from: {adjudicator}")
    still_blank = int(blank_mask.sum())
    if still_blank:
        print(f"  *** {still_blank} item(s) still have an empty value — "
              "uncoded by a coder, or disputed and not yet resolved")
    if len(disputed) and adj is None:
        cols = key + coder_cols
        print("\nDisagreements. Either settle them by discussion and fill the")
        print("blank cells, or hand this list to a third coder and re-run with")
        print("--adjudicator:")
        print(disputed[cols].to_string(index=False))
        stem = out.with_name(out.stem + "_disputes.csv")
        # Key columns and blanks only. The adjudicator must not see what the two
        # coders said: the point of the third pass is an independent judgement,
        # and showing them a 1 and a 0 anchors it -- worse, it turns adjudication
        # into a vote, which is what the discussion route already was. They work
        # from the packet, matching on participant + item, exactly as the
        # original coders did.
        disputed[key].assign(**{v: "" for v in values}).to_csv(stem, index=False)
        print(f"\nWrote {stem} — {len(disputed)} disputed item(s), keys only.")
        print("The adjudicator fills the blank value column(s) from the packet,")
        print("without being shown what either coder chose, then re-run with")
        print("--adjudicator pointing at that file.")
    print("\nThe file is not finished while any value cell is empty —")
    print("run_all.py drops uncoded rows and the affected model shrinks silently.")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--data", type=Path, help="directory of exports (packet mode)")
    ap.add_argument("--out", type=Path, required=True,
                    help="output directory (packet mode) or correctness.csv path (merge mode)")
    ap.add_argument("--merge", nargs=2, type=Path, metavar=("CODER_1", "CODER_2"),
                    help="merge two filled coder CSVs instead of building a packet")
    ap.add_argument("--kind", choices=["correctness", "agent-errors", "themes"],
                    default="correctness",
                    help="which coding to build or merge (default: %(default)s)")
    ap.add_argument("--phase", choices=list(PHASE_TASKS), default="agent",
                    help="correctness packets only: which tasks to cover. "
                         "`agent` is the half the models read and halves the "
                         "coding load; `baseline` builds the remainder later. "
                         "(default: %(default)s)")
    ap.add_argument("--cap", type=int, default=DEFAULT_CAP, metavar="N",
                    help="code the first N participants per condition in "
                         "enrolment order; 0 codes everyone. The same set is "
                         "used for all three codings. (default: %(default)s)")
    ap.add_argument("--reliability-frac", type=float, default=0.25, metavar="F",
                    help="themes only: the share of responses the second coder "
                         "also codes, for the reported kappa (default: %(default)s)")
    ap.add_argument("--adjudicator", type=Path, metavar="CSV",
                    help="a third coder's resolutions for the disputed items "
                         "(merge mode); resolves blind, without discussion")
    ap.add_argument("--seed", type=int, default=20260812,
                    help="shuffle seed for item order (default: %(default)s)")
    args = ap.parse_args()

    if args.merge:
        merge_coders(args.merge[0], args.merge[1], args.out, args.kind,
                     args.adjudicator)
    elif args.data and args.kind == "agent-errors":
        build_agent_error_packet(args.data, args.out, args.seed, args.cap)
    elif args.data and args.kind == "themes":
        build_qualitative_packet(args.data, args.out, args.seed, args.cap,
                                 args.reliability_frac)
        # The qualitative packet is meant to be blind too, so it gets the same
        # check. It scans participant prose rather than rendered state, so a
        # false positive is likelier here -- which is the right trade, and the
        # same one the correctness packet makes.
        failed = False
        for name in ("QUALITATIVE_PACKET.md", "QUALITATIVE_PACKET_reliability.md"):
            packet = args.out / name
            leaks = verify_blinding(packet)
            if leaks:
                failed = True
                print(f"\n*** BLINDING CHECK FAILED — {len(leaks)} line(s) in "
                      f"{packet} could unblind a coder. Review before sending. ***")
                for n, line in leaks[:10]:
                    print(f"    line {n}: {line[:100]}")
        if failed:
            sys.exit(1)
        print("\nBlinding check passed: no condition or timing markers in either "
              "packet.")
        print("Send coder 1 QUALITATIVE_PACKET.md + theme_coder_1.csv, and coder 2 "
              "QUALITATIVE_PACKET_reliability.md + theme_coder_2.csv.")
        print(f"Send neither of them {MANIFEST_NAME}.")
    elif args.data:
        build_packet(args.data, args.out, args.seed, args.phase, args.cap)
        # Verify before anyone can send it, and fail loudly if it leaks: the
        # packet is useless the moment a coder learns the condition, and that
        # is not recoverable after the fact.
        suffix = "" if args.phase == "all" else f"_{args.phase}"
        packet = args.out / f"CODING_PACKET{suffix}.md"
        leaks = verify_blinding(packet)
        if leaks:
            print(f"\n*** BLINDING CHECK FAILED — {len(leaks)} line(s) in {packet} "
                  "could unblind a coder. DO NOT SEND. ***")
            for n, line in leaks[:10]:
                print(f"    line {n}: {line[:100]}")
            if len(leaks) > 10:
                print(f"    ... and {len(leaks) - 10} more")
            sys.exit(1)
        print(f"\nBlinding check passed: no condition, timing or transcript "
              f"markers in {packet}.")
        print(f"Send each coder {packet.name} and ONE of "
              f"coder_1{suffix}.csv / coder_2{suffix}.csv.")
        print(f"Send neither of them {MANIFEST_NAME}.")
    else:
        ap.error("give --data (to build a packet) or --merge (to combine coder scores)")


if __name__ == "__main__":
    main()
