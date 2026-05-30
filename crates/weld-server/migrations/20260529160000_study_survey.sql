-- Exit-survey responses (Category B, research-paper/WELD_EXIT_SURVEY.md). One
-- row per (participant, item) so the analysis export pivots cleanly by item_id.
CREATE TABLE study_survey_responses (
  participant_id UUID NOT NULL REFERENCES study_participants (id) ON DELETE CASCADE,
  item_id TEXT NOT NULL,
  value TEXT NOT NULL,
  answered_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  PRIMARY KEY (participant_id, item_id)
);

-- Optional follow-up contact, kept in a SEPARATE access-restricted table
-- (WELD_EXIT_SURVEY.md §6): excluded from the analysis dataset and deleted
-- after the follow-up window. Never join this into analysis exports.
CREATE TABLE study_followup_contacts (
  participant_id UUID PRIMARY KEY REFERENCES study_participants (id) ON DELETE CASCADE,
  email TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
);
