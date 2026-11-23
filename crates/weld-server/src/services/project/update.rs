use super::helpers::{fetch_active_project, require_project_team_role};
use crate::errors::ApiError;
use crate::services::column::helpers::fetch_column;
use crate::services::github::helpers::validate_repo_slug;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

#[derive(Default)]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub github_repo: Option<Option<String>>,
    pub done_column_id: Option<Option<Uuid>>,
    pub in_progress_column_id: Option<Option<Uuid>>,
    pub review_column_id: Option<Option<Uuid>>,
    pub auto_close_on_issue_closed: Option<bool>,
    pub github_comment_on_move: Option<bool>,
}

/// Update a project. Caller must be Admin or Owner of the owning team.
pub async fn update_project(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
    input: UpdateProjectInput,
) -> Result<Project, ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Admin).await?;

    if let Some(name) = &input.name
        && name.trim().is_empty()
    {
        return Err(ApiError::Validation("Project name cannot be empty".into()));
    }
    if let Some(name) = &input.name
        && name.trim().chars().count() > 100
    {
        return Err(ApiError::Validation(
            "Project name must be 1-100 characters".into(),
        ));
    }

    if let Some(Some(repo)) = &input.github_repo {
        validate_repo_slug(repo)?;
    }

    // The role columns drive webhook automation ("move the card to Done when
    // the PR merges"). One pointing at another project's column would move
    // cards onto a board they don't belong to.
    for column_id in [
        input.done_column_id,
        input.in_progress_column_id,
        input.review_column_id,
    ]
    .into_iter()
    .flatten()
    .flatten()
    {
        if fetch_column(pool, column_id).await?.project_id != project_id {
            return Err(ApiError::Validation(
                "Column does not belong to this project".into(),
            ));
        }
    }

    let new_name = input
        .name
        .as_ref()
        .map(|n| n.trim().to_string())
        .unwrap_or_else(|| project.name.clone());
    let new_description = match &input.description {
        Some(v) => v.clone(),
        None => project.description.clone(),
    };
    let new_github_repo = match &input.github_repo {
        Some(v) => v.clone(),
        None => project.github_repo.clone(),
    };
    let new_done_column_id = match input.done_column_id {
        Some(v) => v,
        None => project.done_column_id,
    };
    let new_in_progress_column_id = match input.in_progress_column_id {
        Some(v) => v,
        None => project.in_progress_column_id,
    };
    let new_review_column_id = match input.review_column_id {
        Some(v) => v,
        None => project.review_column_id,
    };
    let new_auto_close = input
        .auto_close_on_issue_closed
        .unwrap_or(project.auto_close_on_issue_closed);
    let new_comment_on_move = input
        .github_comment_on_move
        .unwrap_or(project.github_comment_on_move);

    let updated = sqlx::query_as!(
        Project,
        r#"
        UPDATE projects
        SET name = $1,
            description = $2,
            github_repo = $3,
            done_column_id = $4,
            auto_close_on_issue_closed = $5,
            in_progress_column_id = $7,
            review_column_id = $8,
            github_comment_on_move = $9,
            updated_at = NOW()
        WHERE id = $6 AND deleted_at IS NULL
        RETURNING id, team_id, name, slug, description, github_repo, done_column_id,
                  in_progress_column_id, review_column_id,
                  auto_close_on_issue_closed, github_comment_on_move, created_at, updated_at, deleted_at
        "#,
        new_name,
        new_description,
        new_github_repo,
        new_done_column_id,
        new_auto_close,
        project_id,
        new_in_progress_column_id,
        new_review_column_id,
        new_comment_on_move,
    )
    .fetch_one(pool)
    .await?;

    Ok(updated)
}
