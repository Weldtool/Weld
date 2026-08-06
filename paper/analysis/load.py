"""
Load the three study exports into tidy DataFrames.

The exports come from the study export endpoint (see
`crates/weld-server/src/services/study/export.rs`), one CSV per dataset:

    GET /api/v1/study/export?dataset=participants  -> study_participants.csv
    GET /api/v1/study/export?dataset=events        -> study_events.csv
    GET /api/v1/study/export?dataset=survey        -> study_survey.csv

The endpoint is disabled (404) unless STUDY_EXPORT_TOKEN is set on the deploy.

Nothing here interprets the data; that is `derive.py`. This module only reads,
types, and unpacks the JSON payload column, so a schema change in the server
breaks in exactly one place.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import pandas as pd

# Column sets we depend on. If the server export changes shape, these assertions
# fire here rather than producing a silently wrong analysis downstream.
PARTICIPANT_COLS = {
    "participant_code", "agent_condition", "status", "consent_version",
    "consent_given_at", "provisioned_at", "completed_at",
}
EVENT_COLS = {
    "participant_code", "agent_condition", "event_id", "event_type",
    "current_task_id", "client_ts", "server_ts", "payload",
}
SURVEY_COLS = {"participant_code", "agent_condition", "item_id", "value", "answered_at"}


@dataclass
class StudyData:
    """Everything the analysis needs, straight off disk."""

    participants: pd.DataFrame
    events: pd.DataFrame
    survey: pd.DataFrame
    #: Manual correctness coding; empty until the coders deliver it.
    correctness: pd.DataFrame

    def __repr__(self) -> str:  # pragma: no cover - convenience only
        return (
            f"StudyData(participants={len(self.participants)}, "
            f"events={len(self.events)}, survey={len(self.survey)}, "
            f"correctness={len(self.correctness)})"
        )


def _require(df: pd.DataFrame, cols: set[str], name: str) -> None:
    missing = cols - set(df.columns)
    if missing:
        raise ValueError(
            f"{name} is missing expected column(s): {sorted(missing)}. "
            "The server export shape probably changed -- update load.py."
        )


def _parse_payload(raw: object) -> dict:
    """Payload is a JSON object serialised into one CSV cell."""
    if isinstance(raw, dict):
        return raw
    if raw is None or (isinstance(raw, float) and pd.isna(raw)):
        return {}
    try:
        parsed = json.loads(raw)
    except (TypeError, ValueError):
        return {}
    return parsed if isinstance(parsed, dict) else {}


def load(data_dir: str | Path) -> StudyData:
    """Read the exports from `data_dir`.

    `correctness.csv` is produced by the human coders, not the server. See
    `README.md` for its required shape. It is optional so the pipeline can be
    exercised on telemetry alone before coding is finished.
    """
    d = Path(data_dir)

    participants = pd.read_csv(d / "study_participants.csv")
    _require(participants, PARTICIPANT_COLS, "study_participants.csv")

    events = pd.read_csv(d / "study_events.csv")
    _require(events, EVENT_COLS, "study_events.csv")

    survey = pd.read_csv(d / "study_survey.csv")
    _require(survey, SURVEY_COLS, "study_survey.csv")

    for frame, cols in (
        (participants, ["consent_given_at", "provisioned_at", "completed_at"]),
        (events, ["client_ts", "server_ts"]),
        (survey, ["answered_at"]),
    ):
        for c in cols:
            frame[c] = pd.to_datetime(frame[c], format="ISO8601", utc=True, errors="coerce")

    events["payload"] = events["payload"].map(_parse_payload)
    # Client clock drives ordering and timing; server time is the backstop when a
    # client timestamp is missing.
    events["ts"] = events["client_ts"].fillna(events["server_ts"])
    events = events.sort_values(["participant_code", "ts", "event_id"]).reset_index(drop=True)

    correctness_path = d / "correctness.csv"
    if correctness_path.exists():
        correctness = pd.read_csv(correctness_path)
        required = {"participant_code", "task_id", "score"}
        _require(correctness, required, "correctness.csv")
    else:
        correctness = pd.DataFrame(
            columns=["participant_code", "task_id", "score", "coder_1", "coder_2"]
        )

    return StudyData(participants, events, survey, correctness)


def payload_field(events: pd.DataFrame, key: str) -> pd.Series:
    """Pull one key out of every event's payload, NaN where absent."""
    return events["payload"].map(lambda p: p.get(key))


def of_type(events: pd.DataFrame, *types: str) -> pd.DataFrame:
    """Subset to one or more event types."""
    return events[events["event_type"].isin(types)]
