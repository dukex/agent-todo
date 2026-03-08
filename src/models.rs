use serde::{Deserialize, Serialize};

// ── Agents ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub agent: Agent,
    pub api_key: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentRequest {
    pub description: Option<String>,
}

// ── Projects ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub description: String,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_project_color")]
    pub color: String,
}

fn default_project_color() -> String {
    "#3B82F6".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub agent_name: String,
    #[serde(default = "default_member_role")]
    pub role: String,
}

fn default_member_role() -> String {
    "editor".to_string()
}

// ── Tasks ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: i32,
    pub due_date: Option<String>,
    pub assignee_id: Option<String>,
}

fn default_status() -> String {
    "pending".to_string()
}

fn default_priority() -> i32 {
    3
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i32>,
    pub due_date: Option<String>,
    pub assignee_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskFilter {
    pub status: Option<String>,
    pub priority: Option<i32>,
    pub assignee_id: Option<String>,
    pub label: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default)]
    pub offset: i32,
}

fn default_limit() -> i32 {
    50
}

// ── Subtasks ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSubtaskRequest {
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubtaskRequest {
    pub title: Option<String>,
    pub status: Option<String>,
    pub position: Option<i32>,
}

// ── Labels ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
    #[serde(default = "default_label_color")]
    pub color: String,
}

fn default_label_color() -> String {
    "#6B7280".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateLabelRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddLabelToTaskRequest {
    pub label_id: String,
}

// ── Comments ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
}

// ── Search ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub label: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default)]
    pub offset: i32,
}

// ── Generic API response ────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }

}
