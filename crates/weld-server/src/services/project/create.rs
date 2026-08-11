use super::helpers::generate_unique_project_slug;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

/// Default columns seeded into every new project. Users can rename, reorder,
/// or delete them from the board UI; this is purely a starting point so the
/// board isn't empty on first load.
const DEFAULT_COLUMNS: &[&str] = &["Backlog", "In Progress", "Review", "Completed"];

/// Create a project under the given team. Caller must be Admin or Owner of the team.
pub async fn create_project(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
    name: &str,
    description: Option<&str>,
) -> Result<Project, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation("Project name cannot be empty".into()));
    }

    require_team_role(pool, team_id, caller_id, TeamRole::Admin).await?;

    let slug = generate_unique_project_slug(pool, trimmed).await?;

    let mut tx = pool.begin().await?;

    let project = sqlx::query_as!(
        Project,
        r#"
        INSERT INTO projects (team_id, name, slug, description)
        VALUES ($1, $2, $3, $4)
        RETURNING id, team_id, name, slug, description, github_repo, done_column_id,
                  in_progress_column_id, review_column_id, auto_close_on_issue_closed,
               github_comment_on_move,
                  created_at, updated_at, deleted_at
        "#,
        team_id,
        trimmed,
        slug,
        description
    )
    .fetch_one(&mut *tx)
    .await?;

    // Seed the default board columns, remembering the ids of the ones we map to
    // the PR-automation roles (In Progress / Review / Completed).
    let mut in_progress_id: Option<Uuid> = None;
    let mut review_id: Option<Uuid> = None;
    let mut done_id: Option<Uuid> = None;
    for (idx, col_name) in DEFAULT_COLUMNS.iter().enumerate() {
        let position = (idx + 1) as f64;
        let col_id = sqlx::query_scalar!(
            "INSERT INTO columns (project_id, name, position) VALUES ($1, $2, $3) RETURNING id",
            project.id,
            col_name,
            position
        )
        .fetch_one(&mut *tx)
        .await?;
        match *col_name {
            "In Progress" => in_progress_id = Some(col_id),
            "Review" => review_id = Some(col_id),
            "Completed" => done_id = Some(col_id),
            _ => {}
        }
    }

    let project = sqlx::query_as!(
        Project,
        r#"
        UPDATE projects
        SET in_progress_column_id = $2, review_column_id = $3, done_column_id = $4
        WHERE id = $1
        RETURNING id, team_id, name, slug, description, github_repo, done_column_id,
                  in_progress_column_id, review_column_id, auto_close_on_issue_closed,
               github_comment_on_move,
                  created_at, updated_at, deleted_at
        "#,
        project.id,
        in_progress_id,
        review_id,
        done_id,
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(project)
}
