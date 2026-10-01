use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::realtime::broadcast::publish_channel_event;
use crate::services::chat::{
    self, CreateChannelInput, MessagePage, PostMessageInput, ThreadView, UpdateChannelInput,
};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use weld_core::models::message::{Channel, ChatMessage};

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub is_direct: bool,
    #[serde(default)]
    pub members: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub topic: Option<Option<String>>,
    pub github_repos: Option<Vec<String>>,
    pub github_event_filter: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct PostMessageRequest {
    pub content: String,
    #[serde(default)]
    pub thread_parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EditMessageRequest {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ListMessagesQuery {
    #[serde(default)]
    pub before: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<i64>,
}

fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Debug, Serialize)]
pub struct ChannelResponse {
    pub id: Uuid,
    pub team_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub topic: Option<String>,
    pub is_direct: bool,
    pub is_private: bool,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub github_repos: Vec<String>,
    pub github_event_filter: serde_json::Value,
}

impl From<Channel> for ChannelResponse {
    fn from(c: Channel) -> Self {
        Self {
            id: c.id,
            team_id: c.team_id,
            project_id: c.project_id,
            name: c.name,
            topic: c.topic,
            is_direct: c.is_direct,
            is_private: c.is_private,
            created_by: c.created_by,
            created_at: c.created_at,
            updated_at: c.updated_at,
            github_repos: c.github_repos,
            github_event_filter: c.github_event_filter,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub thread_parent_id: Option<Uuid>,
    pub mentions: Vec<Uuid>,
    pub task_refs: Vec<Uuid>,
    pub metadata: serde_json::Value,
    pub edited_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// Number of thread replies. Only set when listing a channel's messages;
    /// absent from realtime events and single-message responses, which don't
    /// compute it, so a client never mistakes "not computed" for zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<i64>,
}

impl From<ChatMessage> for MessageResponse {
    fn from(m: ChatMessage) -> Self {
        Self {
            id: m.id,
            channel_id: m.channel_id,
            sender_id: m.sender_id,
            content: m.content,
            thread_parent_id: m.thread_parent_id,
            mentions: m.mentions,
            task_refs: m.task_refs,
            metadata: m.metadata,
            edited_at: m.edited_at,
            created_at: m.created_at,
            reply_count: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MessagePageResponse {
    pub messages: Vec<MessageResponse>,
    pub next_before: Option<Uuid>,
}

impl From<MessagePage> for MessagePageResponse {
    fn from(p: MessagePage) -> Self {
        Self {
            messages: p
                .messages
                .into_iter()
                .map(|m| {
                    let reply_count = p.reply_counts.get(&m.id).copied().unwrap_or(0);
                    MessageResponse {
                        reply_count: Some(reply_count),
                        ..m.into()
                    }
                })
                .collect(),
            next_before: p.next_before,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ThreadResponse {
    pub parent: MessageResponse,
    pub replies: Vec<MessageResponse>,
}

impl From<ThreadView> for ThreadResponse {
    fn from(t: ThreadView) -> Self {
        Self {
            parent: t.parent.into(),
            replies: t.replies.into_iter().map(Into::into).collect(),
        }
    }
}

pub async fn list_channels(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<ChannelResponse>>, ApiError> {
    let channels = chat::list_channels(&state.db, team_id, auth.user_id).await?;
    Ok(Json(channels.into_iter().map(Into::into).collect()))
}

pub async fn create_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Json(input): Json<CreateChannelRequest>,
) -> Result<(StatusCode, Json<ChannelResponse>), ApiError> {
    let channel = chat::create_channel(
        &state.db,
        auth.user_id,
        CreateChannelInput {
            team_id,
            project_id: input.project_id,
            name: input.name,
            topic: input.topic,
            is_private: input.is_private,
            is_direct: input.is_direct,
            initial_members: input.members,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(channel.into())))
}

pub async fn get_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let channel = chat::get_channel(&state.db, id, auth.user_id).await?;
    Ok(Json(channel.into()))
}

pub async fn update_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateChannelRequest>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let channel = chat::update_channel(
        &state.db,
        id,
        auth.user_id,
        UpdateChannelInput {
            name: input.name,
            topic: input.topic,
            github_repos: input.github_repos,
            github_event_filter: input.github_event_filter,
        },
    )
    .await?;
    Ok(Json(channel.into()))
}

pub async fn archive_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    chat::archive_channel(&state.db, id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<AddMemberRequest>,
) -> Result<StatusCode, ApiError> {
    chat::add_channel_member(&state.db, id, auth.user_id, input.user_id).await?;
    Ok(StatusCode::CREATED)
}

pub async fn remove_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    chat::remove_channel_member(&state.db, channel_id, auth.user_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn mark_read(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    chat::mark_channel_read(&state.db, id, auth.user_id).await?;
    publish_channel_event(
        &state.hub,
        id,
        "chat.read",
        &serde_json::json!({
            "channel_id": id,
            "user_id": auth.user_id,
            "at": chrono::Utc::now(),
        }),
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_messages(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<ListMessagesQuery>,
) -> Result<Json<MessagePageResponse>, ApiError> {
    let page = chat::list_messages(&state.db, id, auth.user_id, q.before, q.limit).await?;
    Ok(Json(page.into()))
}

pub async fn post_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<PostMessageRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), ApiError> {
    let msg = chat::post_message(
        &state.db,
        id,
        auth.user_id,
        PostMessageInput {
            content: input.content,
            thread_parent_id: input.thread_parent_id,
        },
    )
    .await?;
    let resp: MessageResponse = msg.into();
    publish_channel_event(&state.hub, id, "chat.message.new", &resp);
    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn get_thread(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ThreadResponse>, ApiError> {
    let thread = chat::get_thread(&state.db, id, auth.user_id).await?;
    Ok(Json(thread.into()))
}

pub async fn edit_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<EditMessageRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let msg = chat::edit_message(&state.db, id, auth.user_id, input.content).await?;
    let resp: MessageResponse = msg.into();
    publish_channel_event(&state.hub, resp.channel_id, "chat.message.edited", &resp);
    Ok(Json(resp))
}

pub async fn delete_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = chat::delete_message(&state.db, id, auth.user_id).await?;
    publish_channel_event(
        &state.hub,
        deleted.channel_id,
        "chat.message.deleted",
        &serde_json::json!({
            "id": deleted.id,
            "channel_id": deleted.channel_id,
            "thread_parent_id": deleted.thread_parent_id,
        }),
    );
    Ok(StatusCode::NO_CONTENT)
}
