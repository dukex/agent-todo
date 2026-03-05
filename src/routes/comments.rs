use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use worker::Env;

use crate::auth::AuthAgent;
use crate::error::AppError;
use crate::models::*;
use crate::routes::projects::check_member;

/// POST /api/v1/tasks/:task_id/comments
pub async fn create(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(task_id): Path<String>,
    Json(body): Json<CreateCommentRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.content.is_empty() || body.content.len() > 5000 {
        return Err(AppError::bad_request("Comment must be 1-5000 characters"));
    }

    let db = env.d1("DB").map_err(AppError::from)?;

    let task = db
        .prepare("SELECT project_id FROM tasks WHERE id = ?1")
        .bind(&[task_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Task not found"))?;

    let project_id = task["project_id"].as_str().unwrap_or_default();
    check_member(&db, project_id, &auth.id).await?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare("INSERT INTO comments (id, task_id, agent_id, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(&[
            id.clone().into(),
            task_id.clone().into(),
            auth.id.clone().into(),
            body.content.clone().into(),
            now.clone().into(),
        ])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({
        "success": true,
        "data": {
            "id": id,
            "task_id": task_id,
            "agent_id": auth.id,
            "agent_name": auth.name,
            "content": body.content,
            "created_at": now,
        }
    }))))
}

/// GET /api/v1/tasks/:task_id/comments
pub async fn list(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let task = db
        .prepare("SELECT project_id FROM tasks WHERE id = ?1")
        .bind(&[task_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Task not found"))?;

    let project_id = task["project_id"].as_str().unwrap_or_default();
    check_member(&db, project_id, &auth.id).await?;

    let rows = db
        .prepare(
            "SELECT c.id, c.task_id, c.agent_id, a.name as agent_name, c.content, c.created_at
             FROM comments c
             JOIN agents a ON a.id = c.agent_id
             WHERE c.task_id = ?1
             ORDER BY c.created_at ASC",
        )
        .bind(&[task_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all().await.map_err(|e| AppError::internal(&e.to_string()))?;

    let comments: Vec<serde_json::Value> = rows.results().unwrap_or_default();

    Ok(Json(serde_json::json!({ "success": true, "data": comments })))
}

/// DELETE /api/v1/comments/:id
pub async fn delete(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(comment_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let comment = db
        .prepare("SELECT c.agent_id, t.project_id FROM comments c JOIN tasks t ON t.id = c.task_id WHERE c.id = ?1")
        .bind(&[comment_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Comment not found"))?;

    // Only comment author can delete
    if comment["agent_id"].as_str().unwrap_or_default() != auth.id {
        return Err(AppError::forbidden("You can only delete your own comments"));
    }

    db.prepare("DELETE FROM comments WHERE id = ?1")
        .bind(&[comment_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "message": "Comment deleted" })))
}
