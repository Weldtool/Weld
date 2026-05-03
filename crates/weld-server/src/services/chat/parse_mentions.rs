use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

/// Extract `@token` mentions from markdown content, ignoring tokens inside
/// inline code spans (`` `like this` ``) and fenced code blocks (```` ``` ````).
///
/// Tokens are `[A-Za-z0-9_.-]+`, lowercased, deduplicated in insertion order.
pub fn extract_mention_tokens(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut in_fence = false;

    for raw_line in content.split('\n') {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        let bytes = raw_line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'`' {
                // skip until matching backtick
                i += 1;
                while i < bytes.len() && bytes[i] != b'`' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                continue;
            }
            if b == b'@' {
                // mentions must be at start-of-line or preceded by whitespace/punct
                let prev_ok = i == 0 || {
                    let p = bytes[i - 1];
                    !(p.is_ascii_alphanumeric() || p == b'_')
                };
                if !prev_ok {
                    i += 1;
                    continue;
                }
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() {
                    let c = bytes[j];
                    if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'-' {
                        j += 1;
                    } else {
                        break;
                    }
                }
                if j > start {
                    let token = raw_line[start..j].to_ascii_lowercase();
                    if seen.insert(token.clone()) {
                        out.push(token);
                    }
                }
                i = j.max(i + 1);
                continue;
            }
            i += 1;
        }
    }

    out
}

/// Resolve mention tokens to user ids of team members. A token matches if
/// either the user's email local part or their display_name (case-insensitive,
/// spaces stripped) equals the token. Order is preserved by token; duplicate
/// user ids are dropped.
pub async fn resolve_mentions(
    pool: &PgPool,
    team_id: Uuid,
    content: &str,
) -> Result<Vec<Uuid>, ApiError> {
    let tokens = extract_mention_tokens(content);
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.display_name
        FROM users u
        JOIN team_memberships tm ON tm.user_id = u.id
        WHERE tm.team_id = $1
        "#,
        team_id
    )
    .fetch_all(pool)
    .await?;

    let mut out: Vec<Uuid> = Vec::new();
    let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    for token in &tokens {
        for r in &rows {
            let local = r.email.split('@').next().unwrap_or("").to_ascii_lowercase();
            let dn = r
                .display_name
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase();
            if (&local == token || &dn == token) && seen.insert(r.id) {
                out.push(r.id);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::extract_mention_tokens;

    #[test]
    fn extracts_simple_mentions() {
        let r = extract_mention_tokens("hi @alice and @bob");
        assert_eq!(r, vec!["alice", "bob"]);
    }

    #[test]
    fn dedups_case_insensitive() {
        let r = extract_mention_tokens("@Alice @alice @ALICE");
        assert_eq!(r, vec!["alice"]);
    }

    #[test]
    fn ignores_inline_code() {
        let r = extract_mention_tokens("`@alice` @bob");
        assert_eq!(r, vec!["bob"]);
    }

    #[test]
    fn ignores_fenced_code() {
        let r = extract_mention_tokens("```\n@alice\n```\n@bob");
        assert_eq!(r, vec!["bob"]);
    }

    #[test]
    fn ignores_email_like() {
        let r = extract_mention_tokens("contact me at foo@bar.com");
        assert!(r.is_empty(), "got {r:?}");
    }
}
