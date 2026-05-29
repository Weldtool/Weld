-- github_connections.repos was a cached array of repo slugs accessible via the
-- team's GitHub App installation. It was misleading at the data level (implied
-- the team "owns" certain repos) and at the UI level (a team-wide picker that
-- competed with the per-project `projects.github_repo` binding). The
-- accessible-repos list is now fetched live from GitHub per request.
ALTER TABLE github_connections DROP COLUMN repos;
