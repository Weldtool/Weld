ALTER TABLE teams
    ADD COLUMN deleted_at TIMESTAMPTZ;

ALTER TABLE projects
    ADD COLUMN deleted_at TIMESTAMPTZ;

-- Replace global UNIQUE constraints with partial unique indexes scoped to
-- active (non-deleted) rows so soft-deleted slugs can be reused.
ALTER TABLE teams DROP CONSTRAINT teams_slug_key;
ALTER TABLE projects DROP CONSTRAINT projects_slug_key;

CREATE UNIQUE INDEX teams_slug_active_key ON teams (slug) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX projects_slug_active_key ON projects (slug) WHERE deleted_at IS NULL;

CREATE INDEX projects_team_id_active_idx ON projects (team_id) WHERE deleted_at IS NULL;
CREATE INDEX teams_active_idx ON teams (id) WHERE deleted_at IS NULL;
