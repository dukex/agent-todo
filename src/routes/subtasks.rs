use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::future::Future;
use std::sync::Arc;
use wasm_bindgen::JsValue;
use worker::Env;

use crate::auth::AuthAgent;
use crate::error::AppError;
use crate::models::*;
use crate::routes::projects::check_member;
use crate::wasm_send::WasmSend;

/// POST /api/v1/tasks/:task_id/subtasks
pub fn create(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(task_id): Path<String>,
    Json(body): Json<CreateSubtaskRequest>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
        if body.title.is_empty() || body.title.len() > 300 {
            return Err(AppError::bad_request("Title must be 1-300 characters"));
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

        db.prepare("INSERT INTO subtasks (id, task_id, title, created_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(&[id.clone().into(), task_id.clone().into(), body.title.clone().into(), now.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

        Ok((StatusCode::CREATED, Json(serde_json::json!({
            "success": true,
            "data": { "id": id, "task_id": task_id, "title": body.title, "status": "pending", "created_at": now }
        }))))
    })
}

/// PUT /api/v1/subtasks/:id
pub fn update(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(subtask_id): Path<String>,
    Json(body): Json<UpdateSubtaskRequest>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
        let db = env.d1("DB").map_err(AppError::from)?;

        let subtask = db
            .prepare("SELECT s.task_id, t.project_id FROM subtasks s JOIN tasks t ON t.id = s.task_id WHERE s.id = ?1")
            .bind(&[subtask_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .first::<serde_json::Value>(None)
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?
            .ok_or_else(|| AppError::not_found("Subtask not found"))?;

        let project_id = subtask["project_id"].as_str().unwrap_or_default();
        check_member(&db, project_id, &auth.id).await?;

        if let Some(ref title) = body.title {
            db.prepare("UPDATE subtasks SET title = ?1 WHERE id = ?2")
                .bind(&[title.clone().into(), subtask_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }
        if let Some(ref status) = body.status {
            if status != "pending" && status != "done" {
                return Err(AppError::bad_request("Subtask status must be: pending or done"));
            }
            db.prepare("UPDATE subtasks SET status = ?1 WHERE id = ?2")
                .bind(&[status.clone().into(), subtask_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }
        if let Some(pos) = body.position {
            db.prepare("UPDATE subtasks SET position = ?1 WHERE id = ?2")
                .bind(&[JsValue::from(pos), subtask_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }

        Ok(Json(serde_json::json!({ "success": true, "message": "Subtask updated" })))
    })
}

/// DELETE /api/v1/subtasks/:id
pub fn delete(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(subtask_id): Path<String>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
        let db = env.d1("DB").map_err(AppError::from)?;

        let subtask = db
            .prepare("SELECT s.task_id, t.project_id FROM subtasks s JOIN tasks t ON t.id = s.task_id WHERE s.id = ?1")
            .bind(&[subtask_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .first::<serde_json::Value>(None)
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?
            .ok_or_else(|| AppError::not_found("Subtask not found"))?;

        let project_id = subtask["project_id"].as_str().unwrap_or_default();
        check_member(&db, project_id, &auth.id).await?;

        db.prepare("DELETE FROM subtasks WHERE id = ?1")
            .bind(&[subtask_id.into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

        Ok(Json(serde_json::json!({ "success": true, "message": "Subtask deleted" })))
    })
}
