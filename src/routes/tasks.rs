use axum::extract::{Path, Query, State};
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

/// POST /api/v1/projects/:id/tasks
pub fn create(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
    Json(body): Json<CreateTaskRequest>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
        if body.title.is_empty() || body.title.len() > 500 {
            return Err(AppError::bad_request("Title must be 1-500 characters"));
        }

        let db = env.d1("DB").map_err(AppError::from)?;
        check_member(&db, &project_id, &auth.id).await?;

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let assignee = body.assignee_id.clone().unwrap_or_default();
        let due = body.due_date.clone().unwrap_or_default();

        db.prepare(
            "INSERT INTO tasks (id, project_id, creator_id, assignee_id, title, description, status, priority, due_date, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULLIF(?4, ''), ?5, ?6, ?7, ?8, NULLIF(?9, ''), ?10, ?11)",
        )
        .bind(&[
            id.clone().into(),
            project_id.clone().into(),
            auth.id.clone().into(),
            assignee.into(),
            body.title.clone().into(),
            body.description.clone().into(),
            body.status.clone().into(),
            JsValue::from(body.priority),
            due.into(),
            now.clone().into(),
            now.clone().into(),
        ])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

        let task = serde_json::json!({
            "id": id,
            "project_id": project_id,
            "creator_id": auth.id,
            "title": body.title,
            "description": body.description,
            "status": body.status,
            "priority": body.priority,
            "due_date": body.due_date,
            "created_at": now,
            "updated_at": now,
        });

        Ok((StatusCode::CREATED, Json(serde_json::json!({ "success": true, "data": task }))))
    })
}

/// GET /api/v1/projects/:id/tasks
pub fn list(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
    Query(filter): Query<TaskFilter>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
        let db = env.d1("DB").map_err(AppError::from)?;
        check_member(&db, &project_id, &auth.id).await?;

        let mut sql = String::from(
            "SELECT t.id, t.project_id, t.creator_id, t.assignee_id, t.title, t.description,
                    t.status, t.priority, t.due_date, t.position, t.created_at, t.updated_at
             FROM tasks t WHERE t.project_id = ?1",
        );
        let mut params: Vec<String> = vec![project_id.clone()];
        let mut idx = 2;

        if let Some(ref status) = filter.status {
            sql.push_str(&format!(" AND t.status = ?{}", idx));
            params.push(status.clone());
            idx += 1;
        }
        if let Some(priority) = filter.priority {
            sql.push_str(&format!(" AND t.priority = ?{}", idx));
            params.push(priority.to_string());
            idx += 1;
        }
        if let Some(ref assignee) = filter.assignee_id {
            sql.push_str(&format!(" AND t.assignee_id = ?{}", idx));
            params.push(assignee.clone());
            idx += 1;
        }
        if let Some(ref label_name) = filter.label {
            sql.push_str(&format!(
                " AND t.id IN (SELECT tl.task_id FROM task_labels tl JOIN labels l ON l.id = tl.label_id WHERE l.name = ?{})",
                idx
            ));
            params.push(label_name.clone());
            idx += 1;
        }
        let _ = idx;

        sql.push_str(" ORDER BY t.position ASC, t.created_at DESC");
        sql.push_str(&format!(" LIMIT {} OFFSET {}", filter.limit.min(100), filter.offset));

        let d1_params: Vec<JsValue> = params.into_iter().map(|p| JsValue::from(p)).collect();
        let rows = db
            .prepare(&sql)
            .bind(&d1_params)
            .map_err(|e| AppError::internal(&e.to_string()))?
            .all()
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?;

        let tasks: Vec<serde_json::Value> = rows.results().unwrap_or_default();

        Ok(Json(serde_json::json!({
            "success": true,
            "data": tasks,
            "limit": filter.limit,
            "offset": filter.offset,
        })))
    })
}

/// GET /api/v1/tasks/:id
pub fn get(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(task_id): Path<String>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
        let db = env.d1("DB").map_err(AppError::from)?;

        let task = db
            .prepare("SELECT * FROM tasks WHERE id = ?1")
            .bind(&[task_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .first::<serde_json::Value>(None)
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?
            .ok_or_else(|| AppError::not_found("Task not found"))?;

        let project_id = task["project_id"].as_str().unwrap_or_default();
        check_member(&db, project_id, &auth.id).await?;

        let subtask_rows = db
            .prepare("SELECT * FROM subtasks WHERE task_id = ?1 ORDER BY position ASC")
            .bind(&[task_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .all()
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?;
        let subtasks: Vec<serde_json::Value> = subtask_rows.results().unwrap_or_default();

        let label_rows = db
            .prepare("SELECT l.* FROM labels l JOIN task_labels tl ON tl.label_id = l.id WHERE tl.task_id = ?1")
            .bind(&[task_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .all()
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?;
        let labels: Vec<serde_json::Value> = label_rows.results().unwrap_or_default();

        let count_row = db
            .prepare("SELECT COUNT(*) as count FROM comments WHERE task_id = ?1")
            .bind(&[task_id.into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .first::<serde_json::Value>(None)
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?;
        let comment_count = count_row
            .and_then(|r| r["count"].as_i64())
            .unwrap_or(0);

        Ok(Json(serde_json::json!({
            "success": true,
            "data": {
                "task": task,
                "subtasks": subtasks,
                "labels": labels,
                "comment_count": comment_count,
            }
        })))
    })
}

/// PUT /api/v1/tasks/:id
pub fn update(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(task_id): Path<String>,
    Json(body): Json<UpdateTaskRequest>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
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

        let now = chrono::Utc::now().to_rfc3339();

        if let Some(ref title) = body.title {
            db.prepare("UPDATE tasks SET title = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(&[title.clone().into(), now.clone().into(), task_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }
        if let Some(ref desc) = body.description {
            db.prepare("UPDATE tasks SET description = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(&[desc.clone().into(), now.clone().into(), task_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }
        if let Some(ref status) = body.status {
            let valid = ["pending", "in_progress", "done", "cancelled"];
            if !valid.contains(&status.as_str()) {
                return Err(AppError::bad_request("Status must be: pending, in_progress, done, or cancelled"));
            }
            db.prepare("UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(&[status.clone().into(), now.clone().into(), task_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }
        if let Some(priority) = body.priority {
            if !(1..=5).contains(&priority) {
                return Err(AppError::bad_request("Priority must be 1-5"));
            }
            db.prepare("UPDATE tasks SET priority = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(&[JsValue::from(priority), now.clone().into(), task_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }
        if let Some(ref due) = body.due_date {
            db.prepare("UPDATE tasks SET due_date = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(&[due.clone().into(), now.clone().into(), task_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }
        if let Some(ref assignee) = body.assignee_id {
            db.prepare("UPDATE tasks SET assignee_id = NULLIF(?1, ''), updated_at = ?2 WHERE id = ?3")
                .bind(&[assignee.clone().into(), now.clone().into(), task_id.clone().into()])
                .map_err(|e| AppError::internal(&e.to_string()))?
                .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
        }

        Ok(Json(serde_json::json!({ "success": true, "message": "Task updated" })))
    })
}

/// DELETE /api/v1/tasks/:id
pub fn delete(
    State(env): State<Arc<Env>>,
    auth: AuthAgent,
    Path(task_id): Path<String>,
) -> impl Future<Output = Result<impl IntoResponse, AppError>> + Send {
    WasmSend(async move {
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

        db.prepare("DELETE FROM tasks WHERE id = ?1")
            .bind(&[task_id.into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

        Ok(Json(serde_json::json!({ "success": true, "message": "Task deleted" })))
    })
}
