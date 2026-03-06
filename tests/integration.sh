#!/usr/bin/env bash
# Integration tests for AgentTodo API
# Usage: BASE_URL=https://agenttodo.zebrasignal.com ./tests/integration.sh
# Or against local dev: BASE_URL=http://localhost:8787 ./tests/integration.sh

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8787}"
API_BASE="$BASE_URL/api/v1"

RUN_ID=$(date +%s)
PASS=0
FAIL=0
API_KEY=""
AGENT_ID=""
PROJECT_ID=""
TASK_ID=""
SUBTASK_ID=""
LABEL_ID=""
COMMENT_ID=""
WEBHOOK_ID=""
SECOND_AGENT_KEY=""
SECOND_AGENT_ID=""

green() { printf '\033[0;32m%s\033[0m\n' "$1"; }
red()   { printf '\033[0;31m%s\033[0m\n' "$1"; }
blue()  { printf '\033[0;34m%s\033[0m\n' "$1"; }

assert_success() {
  local test_name="$1"
  local response="$2"
  local success
  success=$(echo "$response" | grep -o '"success":true' || true)
  if [ -n "$success" ]; then
    green "  PASS: $test_name"
    PASS=$((PASS + 1))
  else
    red "  FAIL: $test_name"
    red "  Response: $response"
    FAIL=$((FAIL + 1))
  fi
}

assert_field() {
  local test_name="$1"
  local response="$2"
  local field="$3"
  local field_present
  field_present=$(echo "$response" | grep -o "\"$field\"" || true)
  if [ -n "$field_present" ]; then
    green "  PASS: $test_name"
    PASS=$((PASS + 1))
  else
    red "  FAIL: $test_name (missing field: $field)"
    red "  Response: $response"
    FAIL=$((FAIL + 1))
  fi
}

assert_http_status() {
  local test_name="$1"
  local expected_status="$2"
  local actual_status="$3"
  if [ "$actual_status" = "$expected_status" ]; then
    green "  PASS: $test_name (HTTP $actual_status)"
    PASS=$((PASS + 1))
  else
    red "  FAIL: $test_name (expected HTTP $expected_status, got $actual_status)"
    FAIL=$((FAIL + 1))
  fi
}

extract_json_field() {
  local json="$1"
  local field="$2"
  echo "$json" | grep -o "\"$field\":\"[^\"]*\"" | head -1 | sed 's/.*":"\([^"]*\)"/\1/'
}

# ---

blue "=== Agents ==="

blue "-- Register agent --"
REGISTER_RESPONSE=$(curl -s -X POST "$API_BASE/agents/register" \
  -H "Content-Type: application/json" \
  -d "{\"name\": \"TestAgent_${RUN_ID}\", \"description\": \"Integration test agent\"}")
assert_success "Register agent" "$REGISTER_RESPONSE"
assert_field "Register returns api_key" "$REGISTER_RESPONSE" "api_key"

API_KEY=$(extract_json_field "$REGISTER_RESPONSE" "api_key")
AGENT_ID=$(echo "$REGISTER_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

if [ -z "$API_KEY" ]; then
  red "FATAL: Could not extract API key. Aborting."
  exit 1
fi

blue "-- Get agent profile --"
PROFILE_RESPONSE=$(curl -s "$API_BASE/agents/me" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Get agent profile" "$PROFILE_RESPONSE"
assert_field "Profile has name" "$PROFILE_RESPONSE" "name"

blue "-- Update agent profile --"
UPDATE_AGENT_RESPONSE=$(curl -s -X PATCH "$API_BASE/agents/me" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"description": "Updated integration test agent"}')
assert_success "Update agent profile" "$UPDATE_AGENT_RESPONSE"

blue "-- Register second agent (for collaboration tests) --"
SECOND_REGISTER=$(curl -s -X POST "$API_BASE/agents/register" \
  -H "Content-Type: application/json" \
  -d "{\"name\": \"TestAgent_Collab_${RUN_ID}\", \"description\": \"Second agent for collaboration tests\"}")
assert_success "Register second agent" "$SECOND_REGISTER"
SECOND_AGENT_KEY=$(extract_json_field "$SECOND_REGISTER" "api_key")
SECOND_AGENT_ID=$(echo "$SECOND_REGISTER" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

# ---

blue "=== Projects ==="

blue "-- Create project --"
CREATE_PROJECT_RESPONSE=$(curl -s -X POST "$API_BASE/projects" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "Integration Test Project", "description": "Created by integration tests", "color": "#3B82F6"}')
assert_success "Create project" "$CREATE_PROJECT_RESPONSE"
assert_field "Project has id" "$CREATE_PROJECT_RESPONSE" "id"

PROJECT_ID=$(echo "$CREATE_PROJECT_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

blue "-- List projects --"
LIST_PROJECTS_RESPONSE=$(curl -s "$API_BASE/projects" \
  -H "Authorization: Bearer $API_KEY")
assert_success "List projects" "$LIST_PROJECTS_RESPONSE"

blue "-- Get project by ID --"
GET_PROJECT_RESPONSE=$(curl -s "$API_BASE/projects/$PROJECT_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Get project by ID" "$GET_PROJECT_RESPONSE"
assert_field "Project has name" "$GET_PROJECT_RESPONSE" "name"

blue "-- Update project --"
UPDATE_PROJECT_RESPONSE=$(curl -s -X PUT "$API_BASE/projects/$PROJECT_ID" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "Integration Test Project (Updated)", "description": "Updated by integration tests"}')
assert_success "Update project" "$UPDATE_PROJECT_RESPONSE"

# ---

blue "=== Project Members ==="

blue "-- Add member to project --"
ADD_MEMBER_RESPONSE=$(curl -s -X POST "$API_BASE/projects/$PROJECT_ID/members" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"agent_name\": \"TestAgent_Collab_${RUN_ID}\", \"role\": \"editor\"}")
assert_success "Add member to project" "$ADD_MEMBER_RESPONSE"

blue "-- List project members --"
LIST_MEMBERS_RESPONSE=$(curl -s "$API_BASE/projects/$PROJECT_ID/members" \
  -H "Authorization: Bearer $API_KEY")
assert_success "List project members" "$LIST_MEMBERS_RESPONSE"

blue "-- Remove member from project --"
REMOVE_MEMBER_RESPONSE=$(curl -s -X DELETE "$API_BASE/projects/$PROJECT_ID/members/$SECOND_AGENT_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Remove member from project" "$REMOVE_MEMBER_RESPONSE"

# ---

blue "=== Tasks ==="

blue "-- Create task --"
CREATE_TASK_RESPONSE=$(curl -s -X POST "$API_BASE/projects/$PROJECT_ID/tasks" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"title": "Integration Test Task", "description": "Task created by integration tests", "priority": 4, "due_date": "2026-12-31", "status": "pending"}')
assert_success "Create task" "$CREATE_TASK_RESPONSE"
assert_field "Task has id" "$CREATE_TASK_RESPONSE" "id"

TASK_ID=$(echo "$CREATE_TASK_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

blue "-- List tasks in project --"
LIST_TASKS_RESPONSE=$(curl -s "$API_BASE/projects/$PROJECT_ID/tasks" \
  -H "Authorization: Bearer $API_KEY")
assert_success "List tasks" "$LIST_TASKS_RESPONSE"

blue "-- List tasks with status filter --"
FILTERED_TASKS_RESPONSE=$(curl -s "$API_BASE/projects/$PROJECT_ID/tasks?status=pending" \
  -H "Authorization: Bearer $API_KEY")
assert_success "List tasks with status filter" "$FILTERED_TASKS_RESPONSE"

blue "-- Get task by ID --"
GET_TASK_RESPONSE=$(curl -s "$API_BASE/tasks/$TASK_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Get task by ID" "$GET_TASK_RESPONSE"
assert_field "Task detail has subtasks" "$GET_TASK_RESPONSE" "subtasks"
assert_field "Task detail has labels" "$GET_TASK_RESPONSE" "labels"
assert_field "Task detail has comment_count" "$GET_TASK_RESPONSE" "comment_count"

blue "-- Update task status to in_progress --"
UPDATE_TASK_RESPONSE=$(curl -s -X PUT "$API_BASE/tasks/$TASK_ID" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"status": "in_progress", "priority": 5}')
assert_success "Update task status" "$UPDATE_TASK_RESPONSE"

# ---

blue "=== Subtasks ==="

blue "-- Create subtask --"
CREATE_SUBTASK_RESPONSE=$(curl -s -X POST "$API_BASE/tasks/$TASK_ID/subtasks" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"title": "Write unit tests"}')
assert_success "Create subtask" "$CREATE_SUBTASK_RESPONSE"
assert_field "Subtask has id" "$CREATE_SUBTASK_RESPONSE" "id"

SUBTASK_ID=$(echo "$CREATE_SUBTASK_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

blue "-- Update subtask to done --"
UPDATE_SUBTASK_RESPONSE=$(curl -s -X PUT "$API_BASE/subtasks/$SUBTASK_ID" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"status": "done"}')
assert_success "Update subtask to done" "$UPDATE_SUBTASK_RESPONSE"

blue "-- Delete subtask --"
DELETE_SUBTASK_RESPONSE=$(curl -s -X DELETE "$API_BASE/subtasks/$SUBTASK_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Delete subtask" "$DELETE_SUBTASK_RESPONSE"

# ---

blue "=== Labels ==="

blue "-- Create label --"
CREATE_LABEL_RESPONSE=$(curl -s -X POST "$API_BASE/projects/$PROJECT_ID/labels" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "integration-test", "color": "#EF4444"}')
assert_success "Create label" "$CREATE_LABEL_RESPONSE"
assert_field "Label has id" "$CREATE_LABEL_RESPONSE" "id"

LABEL_ID=$(echo "$CREATE_LABEL_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

blue "-- List labels in project --"
LIST_LABELS_RESPONSE=$(curl -s "$API_BASE/projects/$PROJECT_ID/labels" \
  -H "Authorization: Bearer $API_KEY")
assert_success "List project labels" "$LIST_LABELS_RESPONSE"

blue "-- Add label to task --"
ADD_LABEL_RESPONSE=$(curl -s -X POST "$API_BASE/tasks/$TASK_ID/labels" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"label_id\": \"$LABEL_ID\"}")
assert_success "Add label to task" "$ADD_LABEL_RESPONSE"

blue "-- Remove label from task --"
REMOVE_LABEL_RESPONSE=$(curl -s -X DELETE "$API_BASE/tasks/$TASK_ID/labels/$LABEL_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Remove label from task" "$REMOVE_LABEL_RESPONSE"

# ---

blue "=== Comments ==="

blue "-- Add comment to task --"
ADD_COMMENT_RESPONSE=$(curl -s -X POST "$API_BASE/tasks/$TASK_ID/comments" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"content": "Integration test comment — looking good!"}')
assert_success "Add comment to task" "$ADD_COMMENT_RESPONSE"
assert_field "Comment has id" "$ADD_COMMENT_RESPONSE" "id"

COMMENT_ID=$(echo "$ADD_COMMENT_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

blue "-- List comments on task --"
LIST_COMMENTS_RESPONSE=$(curl -s "$API_BASE/tasks/$TASK_ID/comments" \
  -H "Authorization: Bearer $API_KEY")
assert_success "List task comments" "$LIST_COMMENTS_RESPONSE"

blue "-- Delete comment --"
DELETE_COMMENT_RESPONSE=$(curl -s -X DELETE "$API_BASE/comments/$COMMENT_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Delete comment" "$DELETE_COMMENT_RESPONSE"

# ---

blue "=== Webhooks ==="

blue "-- Create webhook --"
CREATE_WEBHOOK_RESPONSE=$(curl -s -X POST "$API_BASE/webhooks" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"url\": \"https://example.com/webhook\", \"events\": [\"task.created\", \"task.completed\"], \"project_id\": \"$PROJECT_ID\"}")
assert_success "Create webhook" "$CREATE_WEBHOOK_RESPONSE"
assert_field "Webhook has id" "$CREATE_WEBHOOK_RESPONSE" "id"

WEBHOOK_ID=$(echo "$CREATE_WEBHOOK_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | sed 's/"id":"\([^"]*\)"/\1/')

blue "-- List webhooks --"
LIST_WEBHOOKS_RESPONSE=$(curl -s "$API_BASE/webhooks" \
  -H "Authorization: Bearer $API_KEY")
assert_success "List webhooks" "$LIST_WEBHOOKS_RESPONSE"

blue "-- Update webhook (deactivate) --"
UPDATE_WEBHOOK_RESPONSE=$(curl -s -X PUT "$API_BASE/webhooks/$WEBHOOK_ID" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"is_active": false}')
assert_success "Update webhook" "$UPDATE_WEBHOOK_RESPONSE"

blue "-- Delete webhook --"
DELETE_WEBHOOK_RESPONSE=$(curl -s -X DELETE "$API_BASE/webhooks/$WEBHOOK_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Delete webhook" "$DELETE_WEBHOOK_RESPONSE"

# ---

blue "=== Search ==="

blue "-- Search tasks --"
SEARCH_RESPONSE=$(curl -s "$API_BASE/search?q=Integration+Test" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Search tasks" "$SEARCH_RESPONSE"

blue "-- Search with status filter --"
SEARCH_FILTERED_RESPONSE=$(curl -s "$API_BASE/search?q=Integration&status=in_progress" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Search with status filter" "$SEARCH_FILTERED_RESPONSE"

# ---

blue "=== Dashboard ==="

blue "-- Get dashboard --"
DASHBOARD_RESPONSE=$(curl -s "$API_BASE/dashboard" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Get dashboard" "$DASHBOARD_RESPONSE"
assert_field "Dashboard has projects" "$DASHBOARD_RESPONSE" "projects"
assert_field "Dashboard has overdue_tasks" "$DASHBOARD_RESPONSE" "overdue_tasks"
assert_field "Dashboard has recent_activity" "$DASHBOARD_RESPONSE" "recent_activity"

# ---

blue "=== Authentication Errors ==="

blue "-- Request without API key returns error --"
NO_AUTH_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$API_BASE/projects")
assert_http_status "Unauthenticated request rejected" "401" "$NO_AUTH_STATUS"

blue "-- Request with invalid API key returns error --"
INVALID_KEY_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$API_BASE/projects" \
  -H "Authorization: Bearer invalid_key_xyz")
assert_http_status "Invalid API key rejected" "401" "$INVALID_KEY_STATUS"

# ---

blue "=== Cleanup ==="

blue "-- Mark task as done --"
DONE_TASK_RESPONSE=$(curl -s -X PUT "$API_BASE/tasks/$TASK_ID" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"status": "done"}')
assert_success "Mark task as done" "$DONE_TASK_RESPONSE"

blue "-- Delete task --"
DELETE_TASK_RESPONSE=$(curl -s -X DELETE "$API_BASE/tasks/$TASK_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Delete task" "$DELETE_TASK_RESPONSE"

blue "-- Delete project --"
DELETE_PROJECT_RESPONSE=$(curl -s -X DELETE "$API_BASE/projects/$PROJECT_ID" \
  -H "Authorization: Bearer $API_KEY")
assert_success "Delete project" "$DELETE_PROJECT_RESPONSE"

# ---

blue "==============================="
TOTAL=$((PASS + FAIL))
if [ "$FAIL" -eq 0 ]; then
  green "All $TOTAL tests passed"
else
  red "$FAIL/$TOTAL tests failed"
  exit 1
fi
