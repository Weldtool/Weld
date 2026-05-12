use sqlx::PgPool;

use crate::errors::ApiError;

/// Handles `push` webhook events.
///
/// Scans commit messages for `[WD-<id>]` task references and logs them.
/// Automatic task-to-commit linking is wired up in issue #29.
pub async fn handle(_db: &PgPool, payload: &serde_json::Value) -> Result<(), ApiError> {
    let repo = payload["repository"]["full_name"].as_str().unwrap_or("");
    let commits = payload["commits"].as_array();

    let Some(commits) = commits else {
        return Ok(());
    };

    for commit in commits {
        let sha = commit["id"].as_str().unwrap_or("");
        let message = commit["message"].as_str().unwrap_or("");
        let refs = extract_task_refs(message);
        if !refs.is_empty() {
            tracing::info!(
                repo,
                sha,
                refs = ?refs,
                "Commit references Weld tasks (auto-link pending #29)"
            );
        }
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
