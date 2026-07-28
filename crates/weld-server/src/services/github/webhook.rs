use std::sync::Arc;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;
use tracing::Instrument;

use crate::errors::ApiError;
use crate::realtime::Hub;

use super::{
    webhook_check_suite, webhook_installation, webhook_issues, webhook_pull_request, webhook_push,
};

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
#[tracing::instrument(skip(db, hub, body), fields(delivery_id, event))]
pub async fn handle_webhook(
    db: &PgPool,
    hub: &Arc<Hub>,
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

    let result = dispatch(db, hub, event, &payload).instrument(span).await;

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

/// Resolves the payload's `installation.id` to the Weld teams that installed it.
///
/// Every handler below matches work by repository *name*. A repo name is not a
/// tenant boundary: anyone can install the Weld App on their own `acme/api`, and
/// without this scoping their PR events would move cards and post chat messages
/// in every unrelated team that happens to have typed the same repo string.
///
/// Returns every matching team, since two Weld teams may legitimately connect
/// the same installation (`github_connections` is unique per team, not per
/// installation).
async fn resolve_installation_teams(
    db: &PgPool,
    payload: &serde_json::Value,
) -> Result<Vec<uuid::Uuid>, ApiError> {
    let Some(installation_id) = payload["installation"]["id"].as_i64() else {
        return Ok(Vec::new());
    };

    let teams = sqlx::query_scalar!(
        "SELECT team_id FROM github_connections WHERE installation_id = $1",
        installation_id
    )
    .fetch_all(db)
    .await?;

    Ok(teams)
}

async fn dispatch(
    db: &PgPool,
    hub: &Arc<Hub>,
    event: &str,
    payload: &serde_json::Value,
) -> Result<(), ApiError> {
    // Installation events are how connections are created and removed, so they
    // are handled before (and without) a team lookup.
    if event == "installation" || event == "installation_repositories" {
        return webhook_installation::handle(db, event, payload).await;
    }

    let teams = resolve_installation_teams(db, payload).await?;
    if teams.is_empty() {
        // A delivery from an installation no team has connected. Acknowledge it
        // so GitHub stops retrying, but touch nothing.
        tracing::warn!(event, "Webhook from an unconnected installation — ignored");
        return Ok(());
    }

    match event {
        "issues" | "issue_comment" => webhook_issues::handle(db, hub, event, payload, &teams).await,
        "pull_request" | "pull_request_review" => {
            webhook_pull_request::handle(db, hub, event, payload, &teams).await
        }
        "push" => webhook_push::handle(db, payload, &teams).await,
        "check_suite" => webhook_check_suite::handle(db, hub, payload, &teams).await,
        _ => {
            tracing::debug!(event, "Unhandled GitHub event type");
            Ok(())
        }
    }
}
