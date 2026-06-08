-- Collect participant email and institution so results can be linked to the
-- person. Nullable because existing rows predate this column.
ALTER TABLE study_participants
  ADD COLUMN participant_email TEXT,
  ADD COLUMN institution TEXT;
