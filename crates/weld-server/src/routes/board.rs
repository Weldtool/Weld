use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::routes::columns::ColumnResponse;
use crate::routes::labels::LabelResponse;
use crate::services::board;
use crate::services::task::TaskWithLabels;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct BoardResponse {
    pub columns: Vec<ColumnResponse>,
    pub tasks: Vec<crate::routes::tasks::TaskResponse>,
    pub labels: Vec<LabelResponse>,
}

pub async fn get_board(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<BoardResponse>, ApiError> {
    let state_data = board::get_board_state(&state.db, project_id, auth.user_id).await?;

    let mut tasks = Vec::with_capacity(state_data.tasks.len());
    for task in state_data.tasks {
        let labels = state_data
            .task_labels
            .get(&task.id)
            .cloned()
            .unwrap_or_default();
        tasks.push(crate::routes::tasks::TaskResponse::from(TaskWithLabels {
            task,
            labels,
        }));
    }

    Ok(Json(BoardResponse {
        columns: state_data.columns.into_iter().map(Into::into).collect(),
        tasks,
        labels: state_data.labels.into_iter().map(Into::into).collect(),
    }))
}
