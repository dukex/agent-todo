use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use worker::Env;

use crate::auth::AuthAgent;
use crate::error::AppError;

/// GET /api/v1/dashboard
pub async fn dashboard(
    env: Arc<Env>,
    auth: AuthAgent,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    // Get agent profile
    let agent = db
        .prepare("SELECT id, name, description, created_at FROM agents WHERE id = ?1")
        .bind(&[auth.id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    // Get projects with task counts
    let projects = db
        .prepare(
            "SELECT p.id, p.name, p.color,
                    COUNT(t.id) as total_tasks,
                    SUM(CASE WHEN t.status = 'pending' THEN 1 ELSE 0 END) as pending,
                    SUM(CASE WHEN t.status = 'in_progress' THEN 1 ELSE 0 END) as in_progress,
                    SUM(CASE WHEN t.status = 'done' THEN 1 ELSE 0 END) as done
             FROM projects p
             JOIN project_members pm ON pm.project_id = p.id
             LEFT JOIN tasks t ON t.project_id = p.id
             WHERE pm.agent_id = ?1
             GROUP BY p.id
             ORDER BY p.updated_at DESC",
        )
        .bind(&[auth.id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    let project_list: Vec<serde_json::Value> = projects.results().unwrap_or_default();

    // Get overdue tasks
    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let overdue = db
        .prepare(
            "SELECT t.id, t.title, t.due_date, t.status, t.priority, p.name as project_name
             FROM tasks t
             JOIN projects p ON p.id = t.project_id
             JOIN project_members pm ON pm.project_id = t.project_id AND pm.agent_id = ?1
             WHERE t.due_date < ?2 AND t.status NOT IN ('done', 'cancelled')
             ORDER BY t.due_date ASC
             LIMIT 20",
        )
        .bind(&[auth.id.clone().into(), now.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    let overdue_list: Vec<serde_json::Value> = overdue.results().unwrap_or_default();

    // Get recently updated tasks
    let recent = db
        .prepare(
            "SELECT t.id, t.title, t.status, t.updated_at, p.name as project_name
             FROM tasks t
             JOIN projects p ON p.id = t.project_id
             JOIN project_members pm ON pm.project_id = t.project_id AND pm.agent_id = ?1
             ORDER BY t.updated_at DESC
             LIMIT 10",
        )
        .bind(&[auth.id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    let recent_list: Vec<serde_json::Value> = recent.results().unwrap_or_default();

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "agent": agent,
            "projects": project_list,
            "overdue_tasks": overdue_list,
            "recent_activity": recent_list,
        }
    })))
}
