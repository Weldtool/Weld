-- tasks: per-row repo pointer and sequential short ID for commit message references
ALTER TABLE tasks ADD COLUMN github_repo TEXT;
ALTER TABLE tasks ADD COLUMN short_id BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE;

-- projects: auto-close configuration for inbound webhook sync
ALTER TABLE projects ADD COLUMN done_column_id     UUID    REFERENCES columns(id) ON DELETE SET NULL;
ALTER TABLE projects ADD COLUMN auto_close_on_issue_closed BOOLEAN NOT NULL DEFAULT FALSE;
