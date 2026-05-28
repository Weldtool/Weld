use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::list::resolve_repo_and_token;
use crate::errors::ApiError;
use crate::state::GithubConfig;

/// Candidate locations GitHub recognises for a pull-request template.
const PR_TEMPLATE_PATHS: &[&str] = &[
    ".github/PULL_REQUEST_TEMPLATE.md",
    ".github/pull_request_template.md",
    "PULL_REQUEST_TEMPLATE.md",
    "pull_request_template.md",
    "docs/PULL_REQUEST_TEMPLATE.md",
    "docs/pull_request_template.md",
];

/// A single issue template discovered under `.github/ISSUE_TEMPLATE/`.
#[derive(Debug, Serialize)]
pub struct IssueTemplate {
    /// Human-facing name from the YAML frontmatter, falling back to the filename.
    pub name: String,
    /// Short description from the frontmatter `about:` field, if present.
    pub about: Option<String>,
    /// Template body with the frontmatter block stripped.
    pub body: String,
}

/// Returns the repository's pull-request template body, or `None` if the repo
/// defines no template. Requires Viewer+ on the owning team.
pub async fn get_pr_template(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Option<String>, ApiError> {
    let (repo, token) = resolve_repo_and_token(pool, github, project_id, caller_id).await?;
    for path in PR_TEMPLATE_PATHS {
        if let Some(content) = fetch_raw_file(&token, &repo, path).await? {
            return Ok(Some(content));
        }
    }
    Ok(None)
}

/// Lists the repository's issue templates. Reads the `.github/ISSUE_TEMPLATE/`
/// directory (one template per markdown file, with optional YAML frontmatter)
/// and falls back to a single `.github/ISSUE_TEMPLATE.md`. Requires Viewer+.
pub async fn list_issue_templates(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<IssueTemplate>, ApiError> {
    let (repo, token) = resolve_repo_and_token(pool, github, project_id, caller_id).await?;

    let mut templates = Vec::new();

    // Directory form: .github/ISSUE_TEMPLATE/*.md
    if let Some(entries) = fetch_dir_listing(&token, &repo, ".github/ISSUE_TEMPLATE").await? {
        for entry in entries {
            let name = entry["name"].as_str().unwrap_or("");
            let is_file = entry["type"].as_str() == Some("file");
            let is_markdown = name.ends_with(".md") || name.ends_with(".markdown");
            if !is_file || !is_markdown {
                continue; // skip config.yml and non-markdown helpers
            }
            let path = entry["path"].as_str().unwrap_or("");
            if let Some(raw) = fetch_raw_file(&token, &repo, path).await? {
                templates.push(parse_issue_template(name, &raw));
            }
        }
    }

    // Single-file fallback when no directory templates exist.
    if templates.is_empty() {
        for path in [".github/ISSUE_TEMPLATE.md", ".github/issue_template.md"] {
            if let Some(raw) = fetch_raw_file(&token, &repo, path).await? {
                templates.push(parse_issue_template("Default", &raw));
                break;
            }
        }
    }

    Ok(templates)
}

/// Lists branch names in the connected repository plus the default branch.
/// Requires Viewer+.
pub async fn list_branches(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<RepoBranches, ApiError> {
    let (repo, token) = resolve_repo_and_token(pool, github, project_id, caller_id).await?;

    let repo_meta =
        github_get_json(&token, &format!("https://api.github.com/repos/{repo}")).await?;
    let default_branch = repo_meta["default_branch"]
        .as_str()
        .unwrap_or("main")
        .to_string();

    let branches_json = github_get_json(
        &token,
        &format!("https://api.github.com/repos/{repo}/branches?per_page=100"),
    )
    .await?;

    let branches = branches_json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|b| b["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    Ok(RepoBranches {
        default_branch,
        branches,
    })
}

#[derive(Debug, Serialize)]
pub struct RepoBranches {
    pub default_branch: String,
    pub branches: Vec<String>,
}

// ── helpers ────────────────────────────────────────────────────────────────

/// Fetches a file's raw text via the contents API. Returns `None` on 404 so
/// callers can probe several candidate paths.
async fn fetch_raw_file(token: &str, repo: &str, path: &str) -> Result<Option<String>, ApiError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "https://api.github.com/repos/{repo}/contents/{path}"
        ))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.raw+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub contents request failed ({status}): {body}"
        )));
    }

    Ok(Some(resp.text().await.unwrap_or_default()))
}

/// Lists a directory's entries via the contents API. Returns `None` on 404.
async fn fetch_dir_listing(
    token: &str,
    repo: &str,
    path: &str,
) -> Result<Option<Vec<serde_json::Value>>, ApiError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "https://api.github.com/repos/{repo}/contents/{path}"
        ))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub contents listing failed ({status}): {body}"
        )));
    }

    let json = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))?;
    Ok(json.as_array().cloned())
}

async fn github_get_json(token: &str, url: &str) -> Result<serde_json::Value, ApiError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub request failed ({status}): {body}"
        )));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))
}

/// Splits a `name: value` line out of YAML frontmatter. Minimal parser — GitHub
/// issue templates only use simple scalar `name`/`about` fields up top.
fn parse_issue_template(filename: &str, raw: &str) -> IssueTemplate {
    let mut name: Option<String> = None;
    let mut about: Option<String> = None;
    let mut body = raw;

    if let Some(rest) = raw.strip_prefix("---")
        && let Some(end) = rest.find("\n---")
    {
        let frontmatter = &rest[..end];
        for line in frontmatter.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim().trim_matches(['"', '\'']).to_string();
                match key.trim() {
                    "name" if !value.is_empty() => name = Some(value),
                    "about" if !value.is_empty() => about = Some(value),
                    _ => {}
                }
            }
        }
        // Skip past the closing delimiter line.
        let after = &rest[end + 4..];
        body = after.trim_start_matches(['\r', '\n']);
    }

    IssueTemplate {
        name: name.unwrap_or_else(|| {
            filename
                .trim_end_matches(".md")
                .trim_end_matches(".markdown")
                .to_string()
        }),
        about,
        body: body.to_string(),
    }
}
