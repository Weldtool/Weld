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
"""

from __future__ import annotations

import argparse
import random
import re
import sys
from pathlib import Path

import pandas as pd

from load import load

#: Task order as administered. Kept explicit rather than derived from the data so
#: a participant who skipped a task still produces a row the coders can score 0.
TASK_IDS = [f"B{i}" for i in range(1, 7)] + [f"T{i}" for i in range(1, 7)]

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


def build_packet(data_dir: Path, out_dir: Path, seed: int) -> int:
    data = load(data_dir)
    anchors = load_anchors()

    user_ids = {}
    if "user_id" in data.participants.columns:
        user_ids = dict(
            zip(data.participants["participant_code"], data.participants["user_id"].astype(str))
        )

    events = data.events
    finals = events[events["event_type"] == "task_final_state"]
    reviews = events[events["event_type"] == "pr_review_submitted"]

    items = []
    for code in sorted(data.participants["participant_code"]):
        for task_id in TASK_IDS:
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

    (out_dir / "CODING_PACKET.md").write_text("\n".join(lines), encoding="utf-8")

    template = pd.DataFrame(
        [{"participant_code": i["participant_code"], "task_id": i["task_id"], "score": ""}
         for i in items]
    ).sort_values(["participant_code", "task_id"])
    for coder in ("coder_1", "coder_2"):
        template.to_csv(out_dir / f"{coder}.csv", index=False)

    print(f"Wrote {out_dir/'CODING_PACKET.md'} — {len(items)} items")
    print(f"Wrote {out_dir/'coder_1.csv'} and {out_dir/'coder_2.csv'} (blank `score` column)")
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


def merge_coders(coder_1: Path, coder_2: Path, out: Path) -> None:
    """Combine two coders' scores into `correctness.csv`.

    Where they agree, that value is the consensus score. Where they don't, the
    `score` cell is left **empty** on purpose: the plan resolves disagreements by
    discussion, not by averaging or by picking a coder. Filling those cells is a
    manual step, and the file is not finished until none are blank.
    """
    a = pd.read_csv(coder_1)
    b = pd.read_csv(coder_2)
    key = ["participant_code", "task_id"]
    for name, df in (("coder_1", a), ("coder_2", b)):
        missing = set(key) - set(df.columns)
        if missing:
            raise SystemExit(f"{name} is missing column(s): {sorted(missing)}")
        if "score" not in df.columns:
            raise SystemExit(f"{name} has no `score` column")

    merged = a[key + ["score"]].merge(
        b[key + ["score"]], on=key, how="outer", suffixes=("_1", "_2")
    )
    merged = merged.rename(columns={"score_1": "coder_1", "score_2": "coder_2"})
    for c in ("coder_1", "coder_2"):
        merged[c] = pd.to_numeric(merged[c], errors="coerce")

    blank = merged[merged["coder_1"].isna() | merged["coder_2"].isna()]
    agree = merged["coder_1"] == merged["coder_2"]
    merged["score"] = merged["coder_1"].where(agree)

    out.parent.mkdir(parents=True, exist_ok=True)
    merged[key + ["score", "coder_1", "coder_2"]].to_csv(out, index=False)

    disputed = merged[~agree & merged["coder_1"].notna() & merged["coder_2"].notna()]
    total = len(merged)
    print(f"Wrote {out} — {total} items")
    print(f"  agreed outright : {int(agree.sum())} ({100.0 * agree.mean():.1f}%)")
    print(f"  need adjudication: {len(disputed)}")
    if len(blank):
        print(f"  *** {len(blank)} item(s) unscored by one or both coders — chase these first")
    if len(disputed):
        print("\nDisagreements (fill the blank `score` cell after you agree a value):")
        print(disputed[key + ["coder_1", "coder_2"]].to_string(index=False))
    print("\nThe file is not finished while any `score` cell is empty —")
    print("run_all.py drops unscored rows and the correctness model shrinks silently.")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--data", type=Path, help="directory of exports (packet mode)")
    ap.add_argument("--out", type=Path, required=True,
                    help="output directory (packet mode) or correctness.csv path (merge mode)")
    ap.add_argument("--merge", nargs=2, type=Path, metavar=("CODER_1", "CODER_2"),
                    help="merge two filled coder CSVs instead of building a packet")
    ap.add_argument("--seed", type=int, default=20260812,
                    help="shuffle seed for item order (default: %(default)s)")
    args = ap.parse_args()

    if args.merge:
        merge_coders(args.merge[0], args.merge[1], args.out)
    elif args.data:
        build_packet(args.data, args.out, args.seed)
        # Verify before anyone can send it, and fail loudly if it leaks: the
        # packet is useless the moment a coder learns the condition, and that
        # is not recoverable after the fact.
        packet = args.out / "CODING_PACKET.md"
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
        print("Send each coder CODING_PACKET.md and ONE of coder_1.csv / coder_2.csv.")
    else:
        ap.error("give --data (to build a packet) or --merge (to combine coder scores)")


if __name__ == "__main__":
    main()
