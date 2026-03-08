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
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthAgent>()
            .cloned()
            .ok_or_else(|| AppError::unauthorized("Missing or invalid API key"))
    }
}

/// Attempts to authenticate a request using Bearer token, checking KV cache then D1.
/// Called in the fetch handler before routing so async WASM futures stay outside axum extractors.
pub async fn try_authenticate(parts: &Parts, env: &Arc<worker::Env>) -> Option<AuthAgent> {
    let api_key = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())?
        .strip_prefix("Bearer ")?;

    let key_hash = hash_api_key(api_key);

    let kv = env.kv("KV").ok()?;
    let cache_key = format!("apikey:{}", key_hash);

    if let Ok(Some(cached)) = kv.get(&cache_key).text().await {
        if let Some((id, name)) = cached.split_once(':') {
            return Some(AuthAgent {
                id: id.to_string(),
                name: name.to_string(),
            });
        }
    }

    let db = env.d1("DB").ok()?;
    let result = db
        .prepare("SELECT id, name FROM agents WHERE api_key_hash = ?1 AND is_active = 1")
        .bind(&[key_hash.clone().into()])
        .ok()?
        .first::<serde_json::Value>(None)
        .await
        .ok()??;

    let id = result["id"].as_str()?.to_string();
    let name = result["name"].as_str()?.to_string();

    let cache_value = format!("{}:{}", id, name);
    let _ = kv
        .put(&cache_key, &cache_value)
        .map(|p| p.expiration_ttl(60))
        .ok();

    Some(AuthAgent { id, name })
}
