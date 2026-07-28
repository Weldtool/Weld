-- Per-project control over Weld → GitHub comments.
--
-- Weld comments on a linked issue when its card enters the In Progress or
-- Review column, so people watching the issue on GitHub can see the work start
-- without opening Weld. Useful by default, but teams that treat the issue thread
-- as human-only conversation can switch it off here.
ALTER TABLE projects
    ADD COLUMN github_comment_on_move BOOLEAN NOT NULL DEFAULT TRUE;

-- GitHub App user-to-server tokens.
--
-- Acting through the App installation attributed every review, merge, comment
-- and issue to the Weld bot. These columns let us hold a per-user token instead,
-- so GitHub records who actually did what.
--
-- Unlike OAuth App tokens these expire (8 hours) and carry a refresh token,
-- which is exactly why they are the safer choice: a leaked token is short-lived,
-- and it can never reach a repository the App was not installed on.
ALTER TABLE github_user_links
    ADD COLUMN refresh_token_encrypted BYTEA,
    ADD COLUMN refresh_nonce           BYTEA,
    ADD COLUMN token_expires_at        TIMESTAMPTZ,
    ADD COLUMN refresh_expires_at      TIMESTAMPTZ;

-- Tokens issued before this migration were minted with the read-only `read:user`
-- scope and cannot write. Clearing them forces a re-authorisation on next use
-- rather than failing every write with a confusing 403.
DELETE FROM github_user_links;
