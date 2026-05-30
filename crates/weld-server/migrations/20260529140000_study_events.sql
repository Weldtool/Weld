-- Behavioral telemetry for the research study (Category A in
-- research-paper/CLAUDE_STUDY.md §5A). One row per logged event, joinable to a
-- participant for analysis/export. The §5 "MUST NOT LOG" exclusions
-- (keystrokes, mouse movement, screen/camera, other tabs, non-PetTracker data)
-- are enforced at the ingest layer — only the allowed event schema is accepted.
CREATE TABLE study_events (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  participant_id UUID NOT NULL REFERENCES study_participants (id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,
  -- Task config id active when the event fired (e.g. 'p1_crossref'); null
  -- outside any task (e.g. free exploration, session lifecycle).
  current_task_id TEXT,
  payload JSONB NOT NULL DEFAULT '{}',
  -- Client-reported event time (for accurate ordering/timing) and server
  -- ingest time (authoritative, monotonic-ish backstop).
  client_ts TIMESTAMPTZ,
  server_ts TIMESTAMPTZ NOT NULL DEFAULT NOW ()
);

CREATE INDEX study_events_participant_idx ON study_events (participant_id, server_ts);
CREATE INDEX study_events_type_idx ON study_events (event_type);
