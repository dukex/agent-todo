use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use worker::Env;

use crate::auth::AuthAgent;
use crate::error::AppError;
use crate::models::*;

/// POST /api/v1/webhooks
pub async fn create(
    env: Arc<Env>,
    auth: AuthAgent,
    Json(body): Json<CreateWebhookRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.url.is_empty() {
        return Err(AppError::bad_request("URL is required"));
    }
    if body.events.is_empty() {
        return Err(AppError::bad_request("At least one event is required"));
    }

    let valid_events = [
        "task.created", "task.updated", "task.completed", "task.deleted",
        "comment.added", "member.added", "member.removed",
    ];
    for event in &body.events {
        if !valid_events.contains(&event.as_str()) {
            return Err(AppError::bad_request(&format!(
                "Invalid event '{}'. Valid: {}", event, valid_events.join(", ")
            )));
        }
    }

    let db = env.d1("DB").map_err(AppError::from)?;
    let id = uuid::Uuid::new_v4().to_string();
    let events_json = serde_json::to_string(&body.events).unwrap_or_default();
    let project_id = body.project_id.clone().unwrap_or_default();
    let secret = body.secret.clone().unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare(
        "INSERT INTO webhooks (id, agent_id, project_id, url, events, secret, created_at)
         VALUES (?1, ?2, NULLIF(?3, ''), ?4, ?5, NULLIF(?6, ''), ?7)",
    )
    .bind(&[
        id.clone().into(),
        auth.id.clone().into(),
        project_id.into(),
        body.url.clone().into(),
        events_json.into(),
        secret.into(),
        now.clone().into(),
    ])
    .map_err(|e| AppError::internal(&e.to_string()))?
    .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({
        "success": true,
        "data": {
            "id": id,
            "url": body.url,
            "events": body.events,
            "is_active": true,
            "created_at": now,
        }
    }))))
}

/// GET /api/v1/webhooks
pub async fn list(
    env: Arc<Env>,
    auth: AuthAgent,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let rows = db
        .prepare("SELECT id, agent_id, project_id, url, events, is_active, created_at FROM webhooks WHERE agent_id = ?1")
        .bind(&[auth.id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all().await.map_err(|e| AppError::internal(&e.to_string()))?;

    let webhooks: Vec<serde_json::Value> = rows.results().unwrap_or_default();

    Ok(Json(serde_json::json!({ "success": true, "data": webhooks })))
}

/// PUT /api/v1/webhooks/:id
pub async fn update(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(webhook_id): Path<String>,
    Json(body): Json<UpdateWebhookRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    // Verify ownership
    let wh = db
        .prepare("SELECT agent_id FROM webhooks WHERE id = ?1")
        .bind(&[webhook_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await.map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Webhook not found"))?;

    if wh["agent_id"].as_str().unwrap_or_default() != auth.id {
        return Err(AppError::forbidden("You can only update your own webhooks"));
    }

    if let Some(ref url) = body.url {
        db.prepare("UPDATE webhooks SET url = ?1 WHERE id = ?2")
            .bind(&[url.clone().into(), webhook_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }
    if let Some(ref events) = body.events {
        let events_json = serde_json::to_string(events).unwrap_or_default();
        db.prepare("UPDATE webhooks SET events = ?1 WHERE id = ?2")
            .bind(&[events_json.into(), webhook_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }
    if let Some(is_active) = body.is_active {
        let val = if is_active { "1" } else { "0" };
        db.prepare("UPDATE webhooks SET is_active = ?1 WHERE id = ?2")
            .bind(&[val.to_string().into(), webhook_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }

    Ok(Json(serde_json::json!({ "success": true, "message": "Webhook updated" })))
}

/// DELETE /api/v1/webhooks/:id
pub async fn delete(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(webhook_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let wh = db
        .prepare("SELECT agent_id FROM webhooks WHERE id = ?1")
        .bind(&[webhook_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await.map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Webhook not found"))?;

    if wh["agent_id"].as_str().unwrap_or_default() != auth.id {
        return Err(AppError::forbidden("You can only delete your own webhooks"));
    }

    db.prepare("DELETE FROM webhooks WHERE id = ?1")
        .bind(&[webhook_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "message": "Webhook deleted" })))
}
