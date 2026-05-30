-- Research study (see research-paper/). All study state lives behind STUDY_MODE
-- and never affects the normal product. This migration is safe to apply whether
-- or not study mode is enabled — it only adds tables/columns/seed rows.

-- Mark teams created for a study session so they can be told apart from real
-- teams (and cleaned up) without scanning study_participants.
ALTER TABLE teams ADD COLUMN is_study BOOLEAN NOT NULL DEFAULT FALSE;

-- The four fixed "fake teammates" who authored the seed content. They are
-- global singletons (like the github-bot user) shared across every
-- participant's cloned project — they author cards/messages but never act, so
-- sharing them is safe and keeps provisioning cheap. Fixed UUIDs so the seed
-- loader can reference them deterministically.
INSERT INTO users (id, email, display_name, password_hash, role) VALUES
  ('00000000-0000-0000-0000-0000000000a1', 'sarah.chen@pettracker.study',  'Sarah Chen',  '', 'member'),
  ('00000000-0000-0000-0000-0000000000a2', 'alex.rivera@pettracker.study', 'Alex Rivera', '', 'member'),
  ('00000000-0000-0000-0000-0000000000a3', 'priya.nair@pettracker.study',  'Priya Nair',  '', 'member'),
  ('00000000-0000-0000-0000-0000000000a4', 'marcus.webb@pettracker.study', 'Marcus Webb', '', 'member')
ON CONFLICT (id) DO NOTHING;

CREATE TABLE study_participants (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
  -- Anonymized, human-facing id (e.g. 'P0001'). Sequential, unique.
  participant_code TEXT NOT NULL UNIQUE,
  -- The throwaway user account this participant works as for the session.
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  -- The isolated, cloned study team + PetTracker project for this participant.
  team_id UUID NOT NULL REFERENCES teams (id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  -- Between-subjects agent context condition, fixed for the whole session.
  agent_condition TEXT NOT NULL CHECK (agent_condition IN ('C1', 'C2', 'C3')),
  -- Consent + lifecycle tracking.
  consent_version TEXT,
  consent_given_at TIMESTAMPTZ,
  status TEXT NOT NULL DEFAULT 'provisioned'
    CHECK (status IN ('provisioned', 'consented', 'in_progress', 'completed', 'abandoned')),
  provisioned_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  completed_at TIMESTAMPTZ
);

-- Monotonic counter backing participant_code allocation (P0001, P0002, ...).
CREATE SEQUENCE study_participant_code_seq;

CREATE INDEX study_participants_user_idx ON study_participants (user_id);
CREATE INDEX study_participants_condition_idx ON study_participants (agent_condition);
