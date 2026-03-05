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

/// POST /api/v1/projects/:id/labels
pub async fn create(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
    Json(body): Json<CreateLabelRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.name.is_empty() || body.name.len() > 50 {
        return Err(AppError::bad_request("Label name must be 1-50 characters"));
    }

    let db = env.d1("DB").map_err(AppError::from)?;
    check_member(&db, &project_id, &auth.id).await?;

    let id = uuid::Uuid::new_v4().to_string();

    db.prepare("INSERT INTO labels (id, project_id, name, color) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[id.clone().into(), project_id.clone().into(), body.name.clone().into(), body.color.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({
        "success": true,
        "data": { "id": id, "project_id": project_id, "name": body.name, "color": body.color }
    }))))
}

/// GET /api/v1/projects/:id/labels
pub async fn list(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;
    check_member(&db, &project_id, &auth.id).await?;

    let rows = db
        .prepare("SELECT * FROM labels WHERE project_id = ?1 ORDER BY name ASC")
        .bind(&[project_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all().await.map_err(|e| AppError::internal(&e.to_string()))?;

    let labels: Vec<serde_json::Value> = rows.results().unwrap_or_default();

    Ok(Json(serde_json::json!({ "success": true, "data": labels })))
}

/// PUT /api/v1/labels/:id
pub async fn update(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(label_id): Path<String>,
    Json(body): Json<UpdateLabelRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let label = db
        .prepare("SELECT project_id FROM labels WHERE id = ?1")
        .bind(&[label_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await.map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Label not found"))?;

    let project_id = label["project_id"].as_str().unwrap_or_default();
    check_member(&db, project_id, &auth.id).await?;

    if let Some(ref name) = body.name {
        db.prepare("UPDATE labels SET name = ?1 WHERE id = ?2")
            .bind(&[name.clone().into(), label_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }
    if let Some(ref color) = body.color {
        db.prepare("UPDATE labels SET color = ?1 WHERE id = ?2")
            .bind(&[color.clone().into(), label_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }

    Ok(Json(serde_json::json!({ "success": true, "message": "Label updated" })))
}

/// DELETE /api/v1/labels/:id
pub async fn delete(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(label_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let label = db
        .prepare("SELECT project_id FROM labels WHERE id = ?1")
        .bind(&[label_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await.map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Label not found"))?;

    let project_id = label["project_id"].as_str().unwrap_or_default();
    check_member(&db, project_id, &auth.id).await?;

    db.prepare("DELETE FROM labels WHERE id = ?1")
        .bind(&[label_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "message": "Label deleted" })))
}

/// POST /api/v1/tasks/:id/labels — add label to task
pub async fn add_to_task(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(task_id): Path<String>,
    Json(body): Json<AddLabelToTaskRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let task = db
        .prepare("SELECT project_id FROM tasks WHERE id = ?1")
        .bind(&[task_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await.map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Task not found"))?;

    let project_id = task["project_id"].as_str().unwrap_or_default();
    check_member(&db, project_id, &auth.id).await?;

    db.prepare("INSERT OR IGNORE INTO task_labels (task_id, label_id) VALUES (?1, ?2)")
        .bind(&[task_id.into(), body.label_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "success": true, "message": "Label added to task" }))))
}

/// DELETE /api/v1/tasks/:task_id/labels/:label_id — remove label from task
pub async fn remove_from_task(
    env: Arc<Env>,
    auth: AuthAgent,
    Path((task_id, label_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let task = db
        .prepare("SELECT project_id FROM tasks WHERE id = ?1")
        .bind(&[task_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await.map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Task not found"))?;

    let project_id = task["project_id"].as_str().unwrap_or_default();
    check_member(&db, project_id, &auth.id).await?;

    db.prepare("DELETE FROM task_labels WHERE task_id = ?1 AND label_id = ?2")
        .bind(&[task_id.into(), label_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "message": "Label removed from task" })))
}
