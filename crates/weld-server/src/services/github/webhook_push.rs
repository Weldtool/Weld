use sqlx::PgPool;

use crate::errors::ApiError;

use super::sync;

/// Handles `push` webhook events.
///
/// Scans commit messages for `[WD-<id>]` task references.  For any matched
/// short IDs, sets `github_repo` on the task (auto-link to repo) if not
/// already set.  Full PR/issue linking happens via the pull_request handler.
pub async fn handle(db: &PgPool, payload: &serde_json::Value) -> Result<(), ApiError> {
    let repo = payload["repository"]["full_name"].as_str().unwrap_or("");
    let commits = payload["commits"].as_array();

    let Some(commits) = commits else {
        return Ok(());
    };

    let mut short_ids: Vec<i64> = Vec::new();

    for commit in commits {
        let sha = commit["id"].as_str().unwrap_or("");
        let message = commit["message"].as_str().unwrap_or("");
        let refs = extract_task_refs(message);
        if !refs.is_empty() {
            tracing::info!(repo, sha, refs = ?refs, "Commit references Weld tasks");
            short_ids.extend(
                refs.iter()
                    .filter_map(|r| r.strip_prefix("WD-").and_then(|n| n.parse::<i64>().ok())),
            );
        }
    }

    short_ids.dedup();

    if !short_ids.is_empty() {
        sync::autolink_from_push(db, repo, &short_ids).await?;
    }

    Ok(())
}

/// Extracts `WD-<id>` references from a commit message.
fn extract_task_refs(message: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut rest = message;
    while let Some(start) = rest.find("[WD-") {
        rest = &rest[start + 1..]; // skip '['
        if let Some(end) = rest.find(']') {
            refs.push(rest[..end].to_string());
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::extract_task_refs;

    #[test]
    fn parses_single_ref() {
        assert_eq!(extract_task_refs("fix bug [WD-42]"), vec!["WD-42"]);
    }

    #[test]
    fn parses_multiple_refs() {
        let refs = extract_task_refs("[WD-1] and [WD-2] done");
        assert_eq!(refs, vec!["WD-1", "WD-2"]);
    }

    #[test]
    fn no_refs_returns_empty() {
        assert!(extract_task_refs("normal commit message").is_empty());
    }

    #[test]
    fn partial_bracket_ignored() {
        assert!(extract_task_refs("[WD-no-close").is_empty());
    }
}
