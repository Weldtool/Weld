use super::helpers::{fetch_active_channel, require_channel_access};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::message::Channel;

pub async fn get_channel(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
) -> Result<Channel, ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    require_channel_access(pool, &channel, caller_id).await?;
    Ok(channel)
}
