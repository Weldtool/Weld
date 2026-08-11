-- Latest CI conclusion for the pull request linked to a task.
--
-- Whether the build is green is the thing people most want to know at a glance
-- about a PR, and the board previously had no way to show it. Populated by the
-- `check_suite` webhook; NULL means "no CI result seen for this card".
-- Stores GitHub's own vocabulary (success, failure, neutral, cancelled,
-- timed_out, action_required, stale) plus "pending" while a suite is running.
ALTER TABLE tasks ADD COLUMN github_check_status TEXT;
