-- Synthetic github-bot user for system messages
INSERT INTO users (id, email, display_name, password_hash, role)
VALUES (
    '00000000-0000-0000-0000-000000000002',
    'github-bot@weld.internal',
    'GitHub',
    '',
    'member'
)
ON CONFLICT (id) DO NOTHING;

-- Per-channel GitHub subscription config
ALTER TABLE channels
    ADD COLUMN github_repos TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN github_event_filter JSONB NOT NULL DEFAULT '{"pr": true, "issue": true, "review": true, "push": false}';

-- Structured event metadata on messages (empty object for normal messages)
ALTER TABLE messages
    ADD COLUMN metadata JSONB NOT NULL DEFAULT '{}';
