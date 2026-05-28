-- projects: column mapping for PR-driven board automation.
-- review_column_id      -> moving a task here prompts a PR creation
-- in_progress_column_id -> a linked PR closed without merging sends the task back here
ALTER TABLE projects ADD COLUMN review_column_id      UUID REFERENCES columns(id) ON DELETE SET NULL;
ALTER TABLE projects ADD COLUMN in_progress_column_id UUID REFERENCES columns(id) ON DELETE SET NULL;
