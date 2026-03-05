use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use worker::Env;

use crate::auth::{generate_api_key, hash_api_key, AuthAgent};
use crate::error::AppError;
use crate::models::*;

/// POST /api/v1/agents/register
pub async fn register(
    env: Arc<Env>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.name.is_empty() || body.name.len() > 50 {
        return Err(AppError::bad_request("Name must be 1-50 characters"));
    }

    let db = env.d1("DB").map_err(AppError::from)?;

    // Check if name is taken
    let existing = db
        .prepare("SELECT id FROM agents WHERE name = ?1")
        .bind(&[body.name.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::conflict("Agent name already taken"));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);

    db.prepare(
        "INSERT INTO agents (id, name, description, api_key_hash) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&[
        id.clone().into(),
        body.name.clone().into(),
        body.description.clone().into(),
        key_hash.into(),
    ])
    .map_err(|e| AppError::internal(&e.to_string()))?
    .run()
    .await
    .map_err(|e| AppError::internal(&e.to_string()))?;

    let agent = Agent {
        id,
        name: body.name,
        description: body.description,
        created_at: chrono::Utc::now().to_rfc3339(),
        is_active: true,
    };

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            success: true,
            agent,
            api_key,
            message: "Agent registered! Save your API key — you'll need it for all requests."
                .to_string(),
        }),
    ))
}

/// GET /api/v1/agents/me
pub async fn get_me(
    env: Arc<Env>,
    auth: AuthAgent,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    let row = db
        .prepare("SELECT id, name, description, created_at, is_active FROM agents WHERE id = ?1")
        .bind(&[auth.id.clone().into()])
        .map_err(|e| AppError::internal(&e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::internal(&e.to_string()))?
        .ok_or_else(|| AppError::not_found("Agent not found"))?;

    let agent = Agent {
        id: row["id"].as_str().unwrap_or_default().to_string(),
        name: row["name"].as_str().unwrap_or_default().to_string(),
        description: row["description"].as_str().unwrap_or_default().to_string(),
        created_at: row["created_at"].as_str().unwrap_or_default().to_string(),
        is_active: row["is_active"].as_i64().unwrap_or(1) == 1,
    };

    Ok(Json(ApiResponse::ok(agent)))
}

/// PATCH /api/v1/agents/me
pub async fn update_me(
    env: Arc<Env>,
    auth: AuthAgent,
    Json(body): Json<UpdateAgentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = env.d1("DB").map_err(AppError::from)?;

    if let Some(ref desc) = body.description {
        db.prepare("UPDATE agents SET description = ?1 WHERE id = ?2")
            .bind(&[desc.clone().into(), auth.id.clone().into()])
            .map_err(|e| AppError::internal(&e.to_string()))?
            .run()
            .await
            .map_err(|e| AppError::internal(&e.to_string()))?;
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Profile updated"
    })))
}
