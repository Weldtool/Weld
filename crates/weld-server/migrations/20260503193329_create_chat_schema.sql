CREATE TABLE channels (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
  team_id UUID NOT NULL REFERENCES teams (id) ON DELETE CASCADE,
  project_id UUID REFERENCES projects (id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  topic TEXT,
  is_direct BOOLEAN NOT NULL DEFAULT FALSE,
  is_private BOOLEAN NOT NULL DEFAULT FALSE,
  created_by UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX channels_team_name_active_key ON channels (team_id, LOWER(name)) WHERE deleted_at IS NULL;

CREATE INDEX channels_team_active_idx ON channels (team_id) WHERE deleted_at IS NULL;

CREATE INDEX channels_project_active_idx ON channels (project_id) WHERE deleted_at IS NULL AND project_id IS NOT NULL;

CREATE TABLE channel_memberships (
  channel_id UUID NOT NULL REFERENCES channels (id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  last_read_at TIMESTAMPTZ,
  PRIMARY KEY (channel_id, user_id)
);

CREATE INDEX channel_memberships_user_idx ON channel_memberships (user_id);

CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
  channel_id UUID NOT NULL REFERENCES channels (id) ON DELETE CASCADE,
  sender_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  content TEXT NOT NULL,
  thread_parent_id UUID REFERENCES messages (id) ON DELETE CASCADE,
  mentions UUID[] NOT NULL DEFAULT '{}',
  edited_at TIMESTAMPTZ,
  deleted_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
);

CREATE INDEX messages_channel_created_idx ON messages (channel_id, created_at DESC);

CREATE INDEX messages_thread_parent_idx ON messages (thread_parent_id) WHERE thread_parent_id IS NOT NULL;

CREATE INDEX messages_mentions_idx ON messages USING GIN (mentions);
