use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::task::Label;

use crate::errors::ApiError;
use crate::services::github::helpers::{create_installation_token, gh_client, github_error};
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use crate::state::GithubConfig;

#[derive(Deserialize)]
struct GhLabel {
    name: String,
    color: String,
}

/// Pulls the repo's labels from GitHub and upserts them into the project's
/// label table, matching on lowercase name. Returns the project's full label
/// list after sync. Requires Member+ on the project's team.
pub async fn sync_labels_from_github(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<Label>, ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    let repo = project.github_repo.clone().ok_or_else(|| {
        ApiError::Validation("Project has no GitHub repository configured".into())
    })?;

    let conn = sqlx::query!(
        "SELECT installation_id FROM github_connections WHERE team_id = $1",
        project.team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Validation("Team has no GitHub connection".into()))?;

    let token = create_installation_token(github, conn.installation_id).await?;

    let resp = gh_client()
        .get(format!(
            "https://api.github.com/repos/{repo}/labels?per_page=100"
        ))
        .bearer_auth(&token)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("list labels", resp).await);
    }

    let gh_labels: Vec<GhLabel> = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))?;

    // Snapshot existing project labels so we can match by lowercase name
    // without round-tripping per label.
    let existing: Vec<Label> = sqlx::query_as!(
        Label,
        r#"
        SELECT id, project_id, name, color, created_at
        FROM labels
        WHERE project_id = $1
        "#,
        project_id,
    )
    .fetch_all(pool)
    .await?;

    let lookup: std::collections::HashMap<String, ()> = existing
        .iter()
        .map(|l| (l.name.to_lowercase(), ()))
        .collect();

    for gh in &gh_labels {
        let trimmed = gh.name.trim();
        if trimmed.is_empty() || lookup.contains_key(&trimmed.to_lowercase()) {
            continue;
        }
        let color = format!("#{}", gh.color);
        // ON CONFLICT swallows the duplicate case if a race or unique-index
        // ambiguity (e.g., different casing inserted concurrently) trips.
        sqlx::query!(
            r#"
            INSERT INTO labels (project_id, name, color)
            VALUES ($1, $2, $3)
            ON CONFLICT (project_id, name) DO NOTHING
            "#,
            project_id,
            trimmed,
            color,
        )
        .execute(pool)
        .await?;
    }

    let labels = sqlx::query_as!(
        Label,
        r#"
        SELECT id, project_id, name, color, created_at
        FROM labels
        WHERE project_id = $1
        ORDER BY name
        "#,
        project_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(labels)
}
