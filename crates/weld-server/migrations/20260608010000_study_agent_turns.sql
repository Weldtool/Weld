-- Stores agent conversation turns server-side so the client never controls
-- what goes into the LLM context (prevents prompt injection via history).
CREATE TABLE study_agent_turns (
  id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
  participant_id UUID      NOT NULL REFERENCES study_participants (id) ON DELETE CASCADE,
  task_id      TEXT        NOT NULL,
  role         TEXT        NOT NULL CHECK (role IN ('user', 'assistant')),
  content      TEXT        NOT NULL,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX study_agent_turns_lookup
  ON study_agent_turns (participant_id, task_id, created_at);
