use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

pub const GET_LIMIT: u32 = 120;
pub const WRITE_LIMIT: u32 = 60;
pub const WINDOW_SECONDS: u64 = 60;

pub fn is_read_method(method: &Method) -> bool {
    method == Method::GET || method == Method::HEAD
}

pub fn limit_for_method(method: &Method) -> u32 {
    if is_read_method(method) { GET_LIMIT } else { WRITE_LIMIT }
}

pub fn rate_limit_key(agent_id: &str, method: &Method, window: u64) -> String {
    let method_type = if is_read_method(method) { "r" } else { "w" };
    format!("rl:{}:{}:{}", agent_id, method_type, window)
}

pub fn current_window() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / WINDOW_SECONDS
}

pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset: u64,
}

#[derive(Serialize)]
struct RateLimitError {
    success: bool,
    error: String,
    hint: String,
}

pub fn rate_limit_exceeded_response(info: &RateLimitInfo) -> Response {
    let body = RateLimitError {
        success: false,
        error: "Rate limit exceeded".to_string(),
        hint: format!(
            "You have exceeded the limit of {} requests per minute. Try again after Unix timestamp {}.",
            info.limit, info.reset
        ),
    };
    let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
    let headers = response.headers_mut();
    headers.insert("x-ratelimit-limit", info.limit.into());
    headers.insert("x-ratelimit-remaining", 0u32.into());
    headers.insert(
        "x-ratelimit-reset",
        info.reset.try_into().unwrap_or(u32::MAX).into(),
    );
    response
}

pub async fn check_rate_limit(
    agent_id: &str,
    method: &Method,
    env: &worker::Env,
) -> Result<RateLimitInfo, Response> {
    let limit = limit_for_method(method);
    let window = current_window();
    let reset = (window + 1) * WINDOW_SECONDS;
    let key = rate_limit_key(agent_id, method, window);

    let kv = match env.kv("KV") {
        Ok(store) => store,
        Err(_) => {
            return Ok(RateLimitInfo { limit, remaining: limit, reset });
        }
    };

    let current_count: u32 = kv
        .get(&key)
        .text()
        .await
        .ok()
        .flatten()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    if current_count >= limit {
        let info = RateLimitInfo { limit, remaining: 0, reset };
        return Err(rate_limit_exceeded_response(&info));
    }

    let new_count = current_count + 1;
    let _ = kv
        .put(&key, new_count.to_string().as_str())
        .map(|builder| builder.expiration_ttl(WINDOW_SECONDS))
        .ok();

    Ok(RateLimitInfo {
        limit,
        remaining: limit - new_count,
        reset,
    })
}

pub fn add_rate_limit_headers(response: &mut Response, info: &RateLimitInfo) {
    let headers = response.headers_mut();
    headers.insert("x-ratelimit-limit", info.limit.into());
    headers.insert("x-ratelimit-remaining", info.remaining.into());
    headers.insert(
        "x-ratelimit-reset",
        info.reset.try_into().unwrap_or(u32::MAX).into(),
    );
}

#[cfg(test)]
mod tests {
    use axum::http::Method;
    use super::*;

    #[test]
    fn get_method_is_read() {
        assert!(is_read_method(&Method::GET));
        assert!(is_read_method(&Method::HEAD));
    }

    #[test]
    fn write_methods_are_not_read() {
        assert!(!is_read_method(&Method::POST));
        assert!(!is_read_method(&Method::PUT));
        assert!(!is_read_method(&Method::PATCH));
        assert!(!is_read_method(&Method::DELETE));
    }

    #[test]
    fn get_limit_is_120() {
        assert_eq!(limit_for_method(&Method::GET), 120);
    }

    #[test]
    fn write_limit_is_60() {
        assert_eq!(limit_for_method(&Method::POST), 60);
        assert_eq!(limit_for_method(&Method::PUT), 60);
        assert_eq!(limit_for_method(&Method::DELETE), 60);
    }

    #[test]
    fn rate_limit_key_format_for_read() {
        let key = rate_limit_key("agent-123", &Method::GET, 1000);
        assert_eq!(key, "rl:agent-123:r:1000");
    }

    #[test]
    fn rate_limit_key_format_for_write() {
        let key = rate_limit_key("agent-123", &Method::POST, 1000);
        assert_eq!(key, "rl:agent-123:w:1000");
    }

    #[test]
    fn different_agents_get_different_keys() {
        let key_a = rate_limit_key("agent-a", &Method::GET, 1000);
        let key_b = rate_limit_key("agent-b", &Method::GET, 1000);
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn different_windows_get_different_keys() {
        let key_1 = rate_limit_key("agent-a", &Method::GET, 1000);
        let key_2 = rate_limit_key("agent-a", &Method::GET, 1001);
        assert_ne!(key_1, key_2);
    }
}
