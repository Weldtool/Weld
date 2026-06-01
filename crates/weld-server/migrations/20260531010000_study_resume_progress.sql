-- Server-side study progress so a participant can resume on any device by
-- re-entering their single-use access code (their work already lives in the DB;
-- only their place in the flow was previously kept in browser localStorage).
-- Updated as the participant advances; read back on code-resume. Study-only.

ALTER TABLE study_participants
  ADD COLUMN resume_stage TEXT,
  ADD COLUMN resume_task_index INTEGER NOT NULL DEFAULT 0;
