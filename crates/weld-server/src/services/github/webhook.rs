use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;
use tracing::Instrument;

use crate::errors::ApiError;

use super::{webhook_installation, webhook_issues, webhook_pull_request, webhook_push};

type HmacSha256 = Hmac<Sha256>;

/// Verifies the `X-Hub-Signature-256` header against the raw request body.
/// Uses the `hmac` crate's built-in constant-time comparison.
pub fn verify_signature(secret: &str, body: &[u8], signature_header: &str) -> Result<(), ApiError> {
    let hex_str = signature_header
        .strip_prefix("sha256=")
        .ok_or_else(|| ApiError::Unauthorized("Invalid signature format".into()))?;

    let expected = hex::decode(hex_str)
        .map_err(|_| ApiError::Unauthorized("Malformed signature hex".into()))?;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    mac.verify_slice(&expected)
        .map_err(|_| ApiError::Unauthorized("Webhook signature mismatch".into()))
}

/// Processes an inbound GitHub webhook delivery.
///
/// Idempotent: if `delivery_id` has already been successfully processed the
/// function returns immediately without re-running the handler.  On handler
/// failure the error text is persisted to `github_webhook_deliveries` before
/// the error is propagated so that GitHub will retry (5xx response).
#[tracing::instrument(skip(db, body), fields(delivery_id, event))]
pub async fn handle_webhook(
    db: &PgPool,
    event: &str,
    delivery_id: &str,
    body: &[u8],
) -> Result<(), ApiError> {
    // Idempotency check
    let existing = sqlx::query!(
        "SELECT processed_at FROM github_webhook_deliveries WHERE delivery_id = $1",
        delivery_id
    )
    .fetch_optional(db)
    .await?;

    if let Some(row) = existing {
        if row.processed_at.is_some() {
            tracing::info!("Duplicate delivery — skipping");
            return Ok(());
        }
        // Previously failed: fall through to retry
    } else {
        sqlx::query!(
            "INSERT INTO github_webhook_deliveries (delivery_id, event) VALUES ($1, $2)",
            delivery_id,
            event,
        )
        .execute(db)
        .await?;
    }

    let payload: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| ApiError::Validation("Webhook payload is not valid JSON".into()))?;

    let action = payload["action"].as_str().unwrap_or("").to_string();
    let repo = payload["repository"]["full_name"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let span = tracing::info_span!(
        "github_webhook_dispatch",
        %delivery_id,
        %event,
        %action,
        %repo,
    );

    let result = dispatch(db, event, &payload).instrument(span).await;

    match result {
        Ok(()) => {
            sqlx::query!(
                "UPDATE github_webhook_deliveries SET processed_at = NOW() WHERE delivery_id = $1",
                delivery_id
            )
            .execute(db)
            .await?;
            Ok(())
        }
        Err(e) => {
            let error_text = e.to_string();
            let _ = sqlx::query!(
                "UPDATE github_webhook_deliveries SET error = $2 WHERE delivery_id = $1",
                delivery_id,
                error_text,
            )
            .execute(db)
            .await;
            Err(e)
        }
    }
}

async fn dispatch(db: &PgPool, event: &str, payload: &serde_json::Value) -> Result<(), ApiError> {
    match event {
        "installation" | "installation_repositories" => {
            webhook_installation::handle(db, event, payload).await
        }
        "issues" | "issue_comment" => webhook_issues::handle(db, event, payload).await,
        "pull_request" | "pull_request_review" => {
            webhook_pull_request::handle(db, event, payload).await
        }
        "push" => webhook_push::handle(db, payload).await,
        _ => {
            tracing::debug!(event, "Unhandled GitHub event type");
            Ok(())
        }
    }
}
