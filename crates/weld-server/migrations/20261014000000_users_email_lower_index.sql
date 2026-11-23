-- Login, registration and invites look users up with LOWER(email), which the
-- plain UNIQUE(email) index cannot serve, so each of those was a full scan.
--
-- Deliberately not UNIQUE: existing rows that differ only by case would make
-- this migration fail, and the server runs migrations on startup. New accounts
-- are stored lowercased, and registration rejects case-only duplicates.
CREATE INDEX IF NOT EXISTS users_email_lower_idx ON users (LOWER(email));
