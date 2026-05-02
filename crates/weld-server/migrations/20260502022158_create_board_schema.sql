CREATE TYPE priority AS ENUM ('critical', 'high', 'medium', 'low', 'none');

CREATE TABLE columns (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
  project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  position DOUBLE PRECISION NOT NULL,
  wip_limit INTEGER,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
);

CREATE INDEX columns_project_id_position_idx ON columns (project_id, position);

CREATE TABLE labels (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
  project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  color TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  UNIQUE (project_id, name)
);

CREATE TABLE tasks (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
  project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  column_id UUID NOT NULL REFERENCES columns (id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT,
  assignee_id UUID REFERENCES users (id) ON DELETE SET NULL,
  reporter_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  priority priority NOT NULL DEFAULT 'none',
  position DOUBLE PRECISION NOT NULL,
  due_date TIMESTAMPTZ,
  estimate_hours REAL,
  github_issue_id BIGINT,
  github_pr_id BIGINT,
  deleted_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
);

CREATE INDEX tasks_project_column_position_idx ON tasks (project_id, column_id, position);

CREATE INDEX tasks_active_project_idx ON tasks (project_id) WHERE deleted_at IS NULL;

CREATE INDEX tasks_assignee_idx ON tasks (assignee_id);

CREATE TABLE task_labels (
  task_id UUID NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  label_id UUID NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, label_id)
);
