-- messages: tasks referenced via #<short_id> in the body, resolved to task ids.
-- Mirrors the `mentions` column used for @user references.
ALTER TABLE messages ADD COLUMN task_refs UUID[] NOT NULL DEFAULT '{}';
CREATE INDEX messages_task_refs_idx ON messages USING GIN (task_refs);
