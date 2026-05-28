use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

/// Extract `#<short_id>` task references from markdown content, ignoring tokens
/// inside inline code spans (`` `like this` ``) and fenced code blocks
/// (```` ``` ````).
///
/// Tokens are runs of ASCII digits, deduplicated in insertion order. A `#` only
/// starts a reference at the start of a line or when preceded by whitespace or
/// punctuation, so `abc#3` and hex-like `#fff` are ignored.
pub fn extract_task_ref_ids(content: &str) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
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
            if b == b'#' {
                // references must be at start-of-line or preceded by whitespace/punct
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
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                // require the digit run to end at a word boundary so `#fff` and
                // `#12ab` don't parse as a reference
                let boundary_ok = j == bytes.len() || {
                    let n = bytes[j];
                    !(n.is_ascii_alphanumeric() || n == b'_')
                };
                if j > start
                    && boundary_ok
                    && let Ok(id) = raw_line[start..j].parse::<i64>()
                    && seen.insert(id)
                {
                    out.push(id);
                }
                i = j.max(i + 1);
                continue;
            }
            i += 1;
        }
    }

    out
}

/// Resolve `#<short_id>` references to task ids. A reference matches an active
/// task whose `short_id` equals the number and whose project belongs to
/// `team_id`. Order is preserved by reference; duplicates are dropped.
pub async fn resolve_task_refs(
    pool: &PgPool,
    team_id: Uuid,
    content: &str,
) -> Result<Vec<Uuid>, ApiError> {
    let short_ids = extract_task_ref_ids(content);
    if short_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.short_id
        FROM tasks t
        JOIN projects p ON p.id = t.project_id
        WHERE p.team_id = $1
          AND p.deleted_at IS NULL
          AND t.deleted_at IS NULL
          AND t.short_id = ANY($2)
        "#,
        team_id,
        &short_ids,
    )
    .fetch_all(pool)
    .await?;

    let mut out: Vec<Uuid> = Vec::new();
    let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    // Preserve the order references appeared in the message.
    for short_id in &short_ids {
        if let Some(r) = rows.iter().find(|r| r.short_id == *short_id)
            && seen.insert(r.id)
        {
            out.push(r.id);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::extract_task_ref_ids;

    #[test]
    fn extracts_simple_refs() {
        assert_eq!(extract_task_ref_ids("see #12 and #34"), vec![12, 34]);
    }

    #[test]
    fn dedups_in_order() {
        assert_eq!(extract_task_ref_ids("#7 #7 #7"), vec![7]);
    }

    #[test]
    fn ignores_inline_code() {
        assert_eq!(extract_task_ref_ids("`#12` #34"), vec![34]);
    }

    #[test]
    fn ignores_fenced_code() {
        assert_eq!(extract_task_ref_ids("```\n#12\n```\n#34"), vec![34]);
    }

    #[test]
    fn ignores_non_boundary_and_hex() {
        assert!(extract_task_ref_ids("color #fff and id#5").is_empty());
    }
}
