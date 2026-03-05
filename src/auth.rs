use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::error::AppError;

/// The authenticated agent's ID, extracted from the Bearer token.
#[derive(Debug, Clone)]
pub struct AuthAgent {
    pub id: String,
    pub name: String,
}

/// Hash an API key for storage / lookup.
pub fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a new API key.
pub fn generate_api_key() -> String {
    let id = uuid::Uuid::new_v4();
    format!("agt_{}", id.to_string().replace("-", ""))
}

impl<S> FromRequestParts<S> for AuthAgent
where
    S: Send + Sync,
    Arc<worker::Env>: FromRequestParts<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract Bearer token from Authorization header
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("Missing Authorization header"))?;

        let api_key = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::unauthorized("Invalid Authorization format. Use: Bearer YOUR_API_KEY"))?;

        let key_hash = hash_api_key(api_key);

        // Get Env from extensions (set by our main handler)
        let env = parts
            .extensions
            .get::<Arc<worker::Env>>()
            .ok_or_else(|| AppError::internal("Missing environment"))?;

        // Try KV cache first
        let kv = env.kv("KV").map_err(|e| AppError::internal(&format!("KV error: {}", e)))?;
        let cache_key = format!("apikey:{}", key_hash);

        if let Ok(Some(cached)) = kv.get(&cache_key).text().await {
            // cached value is "agent_id:agent_name"
            if let Some((id, name)) = cached.split_once(':') {
                return Ok(AuthAgent {
                    id: id.to_string(),
                    name: name.to_string(),
                });
            }
        }

        // Fallback to D1
        let db = env.d1("DB").map_err(|e| AppError::internal(&format!("DB error: {}", e)))?;
        let stmt = db.prepare("SELECT id, name FROM agents WHERE api_key_hash = ?1 AND is_active = 1");
        let result = stmt
            .bind(&[key_hash.clone().into()])
            .map_err(|e| AppError::internal(&format!("Bind error: {}", e)))?
            .first::<serde_json::Value>(None)
            .await
            .map_err(|e| AppError::internal(&format!("Query error: {}", e)))?;

        match result {
            Some(row) => {
                let id = row["id"].as_str().unwrap_or_default().to_string();
                let name = row["name"].as_str().unwrap_or_default().to_string();

                // Cache in KV for 5 minutes
                let cache_value = format!("{}:{}", id, name);
                let _ = kv
                    .put(&cache_key, &cache_value)
                    .map(|p| p.expiration_ttl(300))
                    .ok();

                Ok(AuthAgent { id, name })
            }
            None => Err(AppError::unauthorized("Invalid API key")),
        }
    }
}
