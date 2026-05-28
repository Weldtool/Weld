-- Personal teams: a per-user private team so every user has a personal
-- workspace that is separate from shared teams. Personal projects/board/chat
-- reuse all existing team-scoped machinery.
ALTER TABLE teams ADD COLUMN is_personal BOOLEAN NOT NULL DEFAULT FALSE;

-- Project-level membership. Being added to a team no longer implies access to
-- every project in it: Members/Viewers must be explicitly added to a project
-- to see it. Team Owners/Admins keep implicit access to all team projects
-- (enforced in application code), so they are not required to have a row here.
CREATE TABLE project_memberships (
  project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  added_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  PRIMARY KEY (project_id, user_id)
);

CREATE INDEX idx_project_memberships_user ON project_memberships (user_id);

-- Backfill to preserve existing access: every current team member becomes a
-- member of every existing (non-deleted) project in their team.
INSERT INTO project_memberships (project_id, user_id)
SELECT p.id, tm.user_id
FROM projects p
JOIN team_memberships tm ON tm.team_id = p.team_id
WHERE p.deleted_at IS NULL
ON CONFLICT DO NOTHING;

-- Backfill a personal team for every existing user that doesn't have one.
INSERT INTO teams (name, slug, owner_id, is_personal)
SELECT 'Personal', 'personal-' || u.id::text, u.id, TRUE
FROM users u
WHERE NOT EXISTS (
  SELECT 1 FROM teams t WHERE t.owner_id = u.id AND t.is_personal = TRUE
);

-- Add each user as Owner of their personal team.
INSERT INTO team_memberships (team_id, user_id, role)
SELECT t.id, t.owner_id, 'owner'
FROM teams t
WHERE t.is_personal = TRUE
ON CONFLICT (user_id, team_id) DO NOTHING;
