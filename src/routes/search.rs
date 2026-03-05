use axum::extract::Query;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use worker::Env;

use crate::auth::AuthAgent;
use crate::error::AppError;
use crate::models::*;

/// GET /api/v1/search?q=...&project_id=...&status=...&label=...
pub async fn search(
    env: Arc<Env>,
    auth: AuthAgent,
    Query(query): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    if query.q.is_empty() {
        return Err(AppError::bad_request("Search query 'q' is required"));
    }

    let db = env.d1("DB").map_err(AppError::from)?;

    // Search tasks the agent has access to
    let mut sql = String::from(
        "SELECT t.id, t.project_id, t.title, t.description, t.status, t.priority,
                t.due_date, t.assignee_id, t.created_at, p.name as project_name
         FROM tasks t
         JOIN projects p ON p.id = t.project_id
         JOIN project_members pm ON pm.project_id = t.project_id AND pm.agent_id = ?1
         WHERE (t.title LIKE ?2 OR t.description LIKE ?2)",
    );

    let search_pattern = format!("%{}%", query.q);
    let mut params: Vec<String> = vec![auth.id, search_pattern];
    let mut idx = 3;

    if let Some(ref project_id) = query.project_id {
        sql.push_str(&format!(" AND t.project_id = ?{}", idx));
        params.push(project_id.clone());
        idx += 1;
    }
    if let Some(ref status) = query.status {
        sql.push_str(&format!(" AND t.status = ?{}", idx));
        params.push(status.clone());
        idx += 1;
    }
    if let Some(ref label_name) = query.label {
        sql.push_str(&format!(
            " AND t.id IN (SELECT tl.task_id FROM task_labels tl JOIN labels l ON l.id = tl.label_id WHERE l.name = ?{})",
            idx
        ));
        params.push(label_name.clone());
        idx += 1;
    }
    let _ = idx;

    sql.push_str(" ORDER BY t.updated_at DESC");
    sql.push_str(&format!(" LIMIT {} OFFSET {}", query.limit.min(100), query.offset));

    let d1_params: Vec<worker::d1::D1Type> = params.into_iter().map(|p| p.into()).collect();

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
        "query": query.q,
        "data": tasks,
        "limit": query.limit,
        "offset": query.offset,
    })))
}
