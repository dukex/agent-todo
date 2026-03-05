use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use worker::Env;

use crate::auth::AuthAgent;
use crate::error::AppError;
use crate::models::*;

/// POST /api/v1/projects
pub async fn create(
    env: Arc<Env>,
    auth: AuthAgent,
    Json(body): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.name.is_empty() || body.name.len() > 100 {
        return Err(AppError::bad_request("Project name must be 1-100 characters"));
    }

    let db = env.d1("DB").map_err(AppError::from)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Create project
    db.prepare("INSERT INTO projects (id, owner_id, name, description, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")
        .bind(&[
            id.clone().into(),
            auth.id.clone().into(),
            body.name.clone().into(),
            body.description.clone().into(),
            body.color.clone().into(),
            now.clone().into(),
            now.clone().into(),
        ])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    // Add owner as member
    db.prepare("INSERT INTO project_members (project_id, agent_id, role) VALUES (?1, ?2, 'owner')")
        .bind(&[id.clone().into(), auth.id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    let project = Project {
        id,
        owner_id: auth.id,
        name: body.name,
        description: body.description,
        color: body.color,
        created_at: now.clone(),
        updated_at: now,
    };

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(project))))
}

/// GET /api/v1/projects
pub async fn list(
    env: Arc<Env>,
    auth: AuthAgent,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let rows = db
        .prepare(
            "SELECT p.id, p.owner_id, p.name, p.description, p.color, p.created_at, p.updated_at
             FROM projects p
             JOIN project_members pm ON pm.project_id = p.id
             WHERE pm.agent_id = ?1
             ORDER BY p.updated_at DESC",
        )
        .bind(&[auth.id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    let projects: Vec<serde_json::Value> = rows.results().unwrap_or_default();

    Ok(Json(serde_json::json!({
        "success": true,
        "data": projects,
    })))
}

/// GET /api/v1/projects/:id
pub async fn get(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    // Check membership
    check_member(&db, &project_id, &auth.id).await?;

    let row = db
        .prepare("SELECT id, owner_id, name, description, color, created_at, updated_at FROM projects WHERE id = ?1")
        .bind(&[project_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Project not found"))?;

    Ok(Json(ApiResponse::ok(row)))
}

/// PUT /api/v1/projects/:id
pub async fn update(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
    Json(body): Json<UpdateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;
    check_member(&db, &project_id, &auth.id).await?;

    let now = chrono::Utc::now().to_rfc3339();

    if let Some(ref name) = body.name {
        db.prepare("UPDATE projects SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&[name.clone().into(), now.clone().into(), project_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }
    if let Some(ref desc) = body.description {
        db.prepare("UPDATE projects SET description = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&[desc.clone().into(), now.clone().into(), project_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }
    if let Some(ref color) = body.color {
        db.prepare("UPDATE projects SET color = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&[color.clone().into(), now.clone().into(), project_id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run().await.map_err(|e| AppError::internal(&e.to_string()))?;
    }

    Ok(Json(serde_json::json!({ "success": true, "message": "Project updated" })))
}

/// DELETE /api/v1/projects/:id
pub async fn delete(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    // Only owner can delete
    let row = db
        .prepare("SELECT owner_id FROM projects WHERE id = ?1")
        .bind(&[project_id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Project not found"))?;

    if row["owner_id"].as_str().unwrap_or_default() != auth.id {
        return Err(AppError::forbidden("Only the project owner can delete it"));
    }

    db.prepare("DELETE FROM projects WHERE id = ?1")
        .bind(&[project_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "message": "Project deleted" })))
}

/// POST /api/v1/projects/:id/members
pub async fn add_member(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    // Must be owner or editor
    let role = get_member_role(&db, &project_id, &auth.id).await?;
    if role != "owner" && role != "editor" {
        return Err(AppError::forbidden("Only owners and editors can add members"));
    }

    // Find agent by name
    let agent = db
        .prepare("SELECT id FROM agents WHERE name = ?1")
        .bind(&[body.agent_name.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Agent not found"))?;

    let agent_id = agent["id"].as_str().unwrap_or_default().to_string();

    db.prepare("INSERT OR IGNORE INTO project_members (project_id, agent_id, role) VALUES (?1, ?2, ?3)")
        .bind(&[project_id.into(), agent_id.into(), body.role.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "success": true, "message": "Member added" }))))
}

/// GET /api/v1/projects/:id/members
pub async fn list_members(
    env: Arc<Env>,
    auth: AuthAgent,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;
    check_member(&db, &project_id, &auth.id).await?;

    let rows = db
        .prepare(
            "SELECT pm.agent_id, a.name as agent_name, pm.role, pm.added_at
             FROM project_members pm
             JOIN agents a ON a.id = pm.agent_id
             WHERE pm.project_id = ?1",
        )
        .bind(&[project_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .all()
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    let members: Vec<serde_json::Value> = rows.results().unwrap_or_default();

    Ok(Json(serde_json::json!({ "success": true, "data": members })))
}

/// DELETE /api/v1/projects/:id/members/:agent_id
pub async fn remove_member(
    env: Arc<Env>,
    auth: AuthAgent,
    Path((project_id, agent_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let role = get_member_role(&db, &project_id, &auth.id).await?;
    if role != "owner" {
        return Err(AppError::forbidden("Only the owner can remove members"));
    }

    if agent_id == auth.id {
        return Err(AppError::bad_request("Cannot remove yourself (the owner)"));
    }

    db.prepare("DELETE FROM project_members WHERE project_id = ?1 AND agent_id = ?2")
        .bind(&[project_id.into(), agent_id.into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .run().await.map_err(|e| AppError::internal(&e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "message": "Member removed" })))
}

// ── Helpers ──────────────────────────────────────────────────────────

pub async fn check_member(db: &worker::D1Database, project_id: &str, agent_id: &str) -> Result<(), AppError> {
    let row = db
        .prepare("SELECT role FROM project_members WHERE project_id = ?1 AND agent_id = ?2")
        .bind(&[project_id.to_string().into(), agent_id.to_string().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    if row.is_none() {
        return Err(AppError::forbidden("You are not a member of this project"));
    }
    Ok(())
}

pub async fn get_member_role(db: &worker::D1Database, project_id: &str, agent_id: &str) -> Result<String, AppError> {
    let row = db
        .prepare("SELECT role FROM project_members WHERE project_id = ?1 AND agent_id = ?2")
        .bind(&[project_id.to_string().into(), agent_id.to_string().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::forbidden("You are not a member of this project"))?;

    Ok(row["role"].as_str().unwrap_or("viewer").to_string())
}
