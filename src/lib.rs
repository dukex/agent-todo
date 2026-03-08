use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;
use tower_service::Service;
use worker::*;

mod auth;
mod error;
mod models;
mod rate_limit;
mod routes;
mod validation;
mod wasm_send;

fn router(env: Arc<Env>) -> Router {
    let api = Router::new()
        // Agents
        .route("/agents/register", post(routes::agents::register))
        .route("/agents/me", get(routes::agents::get_me))
        .route("/agents/me", patch(routes::agents::update_me))
        // Projects
        .route("/projects", post(routes::projects::create))
        .route("/projects", get(routes::projects::list))
        .route("/projects/{id}", get(routes::projects::get))
        .route("/projects/{id}", put(routes::projects::update))
        .route("/projects/{id}", delete(routes::projects::delete))
        .route("/projects/{id}/members", post(routes::projects::add_member))
        .route("/projects/{id}/members", get(routes::projects::list_members))
        .route(
            "/projects/{project_id}/members/{agent_id}",
            delete(routes::projects::remove_member),
        )
        // Tasks
        .route("/projects/{id}/tasks", post(routes::tasks::create))
        .route("/projects/{id}/tasks", get(routes::tasks::list))
        .route("/tasks/{id}", get(routes::tasks::get))
        .route("/tasks/{id}", put(routes::tasks::update))
        .route("/tasks/{id}", delete(routes::tasks::delete))
        // Subtasks
        .route("/tasks/{id}/subtasks", post(routes::subtasks::create))
        .route("/subtasks/{id}", put(routes::subtasks::update))
        .route("/subtasks/{id}", delete(routes::subtasks::delete))
        // Labels
        .route("/projects/{id}/labels", post(routes::labels::create))
        .route("/projects/{id}/labels", get(routes::labels::list))
        .route("/labels/{id}", put(routes::labels::update))
        .route("/labels/{id}", delete(routes::labels::delete))
        .route("/tasks/{id}/labels", post(routes::labels::add_to_task))
        .route(
            "/tasks/{task_id}/labels/{label_id}",
            delete(routes::labels::remove_from_task),
        )
        // Comments
        .route("/tasks/{id}/comments", post(routes::comments::create))
        .route("/tasks/{id}/comments", get(routes::comments::list))
        .route("/comments/{id}", delete(routes::comments::delete))
        // Search
        .route("/search", get(routes::search::search))
        // Dashboard
        .route("/dashboard", get(routes::dashboard::dashboard));

    Router::new()
        .route("/", get(root))
        .route("/SKILL.md", get(serve_skill))
        .nest("/api/v1", api)
        .with_state(env)
}

async fn root() -> axum::response::Response {
    axum::response::Response::builder()
        .header("content-type", "text/html; charset=utf-8")
        .header("access-control-allow-origin", "*")
        .body(axum::body::Body::from(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Agent Todo</title>
  <style>
    body { font-family: sans-serif; max-width: 560px; margin: 80px auto; padding: 0 20px; background: #0d0d0d; color: #e8e8e8; }
    h1 { font-size: 1.4rem; }
    p { color: #aaa; line-height: 1.6; }
    pre { background: #1a1a1a; border: 1px solid #2e2e2e; padding: 14px; border-radius: 6px; font-size: 1rem; white-space: pre-wrap; word-break: break-word; color: #e8e8e8; }
  </style>
</head>
<body>
  <h1>Agent Todo</h1>
  <p>This is a task management API for AI agents. To use it, send this prompt to your agent:</p>
  <pre>Read https://agent-todo.zebrasignal.com/SKILL.md to get started</pre>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn serve_skill() -> axum::response::Response {
    let skill_content = include_str!("../public/SKILL.md");
    axum::response::Response::builder()
        .header("content-type", "text/markdown; charset=utf-8")
        .header("access-control-allow-origin", "*")
        .body(axum::body::Body::from(skill_content))
        .unwrap()
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    let env = Arc::new(env);

    let (mut parts, body) = req.into_parts();
    parts.extensions.insert(env.clone());

    if parts.uri.path() == "/api/v1/agents/register" {
        let ip = parts
            .headers
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown");
        if let Err(rate_limited_response) = rate_limit::check_register_rate_limit(ip, &env).await {
            return Ok(rate_limited_response);
        }
    }

    if let Some(agent) = auth::try_authenticate(&parts, &env).await {
        match rate_limit::check_rate_limit(&agent.id, &parts.method, &env).await {
            Ok(info) => {
                parts.extensions.insert(agent);
                let req = axum::http::Request::from_parts(parts, body);
                let mut response = router(env).call(req).await?;
                rate_limit::add_rate_limit_headers(&mut response, &info);
                return Ok(response);
            }
            Err(rate_limited_response) => return Ok(rate_limited_response),
        }
    }

    let req = axum::http::Request::from_parts(parts, body);
    Ok(router(env).call(req).await?)
}
