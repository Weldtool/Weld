CREATE TABLE github_connections (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    team_id         UUID NOT NULL UNIQUE REFERENCES teams(id) ON DELETE CASCADE,
    installation_id BIGINT NOT NULL,
    account_login   TEXT NOT NULL,
    repos           TEXT[] NOT NULL DEFAULT '{}',
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_github_connections_team_id ON github_connections(team_id);

CREATE TABLE github_user_links (
    user_id                 UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    github_user_id          BIGINT NOT NULL,
    github_login            TEXT NOT NULL,
    access_token_encrypted  BYTEA NOT NULL,
    nonce                   BYTEA NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_github_user_links_github_user_id ON github_user_links(github_user_id);
