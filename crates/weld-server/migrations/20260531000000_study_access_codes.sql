-- Single-use access codes for study participation (see research-paper/).
-- Each recruited participant receives exactly one code; a code provisions one
-- participant and is then spent, so the same person can't run the study twice
-- (across devices/incognito where localStorage doesn't help). Enforced only
-- when STUDY_REQUIRE_ACCESS_CODE is set; otherwise provisioning ignores codes
-- so local testing stays frictionless. Safe to apply regardless of study mode.

CREATE TABLE study_access_codes (
  -- The human-handed code, e.g. 'WELD-AB12-CD34'. Unique, case-sensitive.
  code TEXT PRIMARY KEY,
  -- Optional label for batch/recruitment tracking (e.g. 'pilot', 'wave-2').
  note TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
  -- NULL until the code is spent. Set atomically inside provisioning.
  used_at TIMESTAMPTZ,
  -- The participant this code provisioned, once spent.
  participant_id UUID REFERENCES study_participants (id) ON DELETE SET NULL
);

-- Fast lookup of still-valid codes.
CREATE INDEX study_access_codes_unused_idx ON study_access_codes (code)
  WHERE used_at IS NULL;
