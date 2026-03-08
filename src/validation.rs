use crate::error::AppError;

const VALID_TASK_STATUSES: &[&str] = &["pending", "in_progress", "done", "cancelled"];
const VALID_SUBTASK_STATUSES: &[&str] = &["pending", "done"];
const VALID_MEMBER_ROLES: &[&str] = &["owner", "editor", "viewer"];

pub fn validate_required_text(field: &str, value: &str, max_len: usize) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::bad_request(&format!(
            "Field '{}' is required and cannot be empty",
            field
        )));
    }
    if value.len() > max_len {
        return Err(AppError::bad_request(&format!(
            "Field '{}' exceeds maximum length of {} characters",
            field, max_len
        )));
    }
    Ok(())
}

pub fn validate_optional_text(
    field: &str,
    value: &Option<String>,
    max_len: usize,
) -> Result<(), AppError> {
    if let Some(text) = value {
        if text.len() > max_len {
            return Err(AppError::bad_request(&format!(
                "Field '{}' exceeds maximum length of {} characters",
                field, max_len
            )));
        }
    }
    Ok(())
}

pub fn validate_hex_color(color: &str) -> Result<(), AppError> {
    let is_valid = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !is_valid {
        return Err(AppError::bad_request(
            "Color must be a valid hex color (e.g. #3B82F6)",
        ));
    }
    Ok(())
}

pub fn validate_priority(priority: i32) -> Result<(), AppError> {
    if !(1..=5).contains(&priority) {
        return Err(AppError::bad_request(
            "Priority must be between 1 (low) and 5 (critical)",
        ));
    }
    Ok(())
}

pub fn validate_task_status(status: &str) -> Result<(), AppError> {
    if !VALID_TASK_STATUSES.contains(&status) {
        return Err(AppError::bad_request(&format!(
            "Invalid status '{}'. Valid values: {}",
            status,
            VALID_TASK_STATUSES.join(", ")
        )));
    }
    Ok(())
}

pub fn validate_subtask_status(status: &str) -> Result<(), AppError> {
    if !VALID_SUBTASK_STATUSES.contains(&status) {
        return Err(AppError::bad_request(&format!(
            "Invalid subtask status '{}'. Valid values: {}",
            status,
            VALID_SUBTASK_STATUSES.join(", ")
        )));
    }
    Ok(())
}

pub fn validate_iso_date(field: &str, value: &str) -> Result<(), AppError> {
    let parts: Vec<&str> = value.split('-').collect();
    let valid = parts.len() == 3
        && parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()));
    if !valid {
        return Err(AppError::bad_request(&format!(
            "Field '{}' must be a valid ISO date (YYYY-MM-DD)",
            field
        )));
    }
    Ok(())
}

pub fn validate_search_query(query: &str) -> Result<(), AppError> {
    if query.trim().is_empty() {
        return Err(AppError::bad_request("Search query 'q' is required"));
    }
    if query.len() > 200 {
        return Err(AppError::bad_request(
            "Search query exceeds maximum length of 200 characters",
        ));
    }
    Ok(())
}

pub fn validate_member_role(role: &str) -> Result<(), AppError> {
    if !VALID_MEMBER_ROLES.contains(&role) {
        return Err(AppError::bad_request(&format!(
            "Invalid role '{}'. Valid values: {}",
            role,
            VALID_MEMBER_ROLES.join(", ")
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_required_text ---

    #[test]
    fn empty_required_field_fails() {
        assert!(validate_required_text("name", "", 255).is_err());
    }

    #[test]
    fn whitespace_only_required_field_fails() {
        assert!(validate_required_text("name", "   ", 255).is_err());
    }

    #[test]
    fn valid_required_field_passes() {
        assert!(validate_required_text("name", "My Project", 255).is_ok());
    }

    #[test]
    fn field_exceeding_max_length_fails() {
        let long = "a".repeat(256);
        assert!(validate_required_text("name", &long, 255).is_err());
    }

    #[test]
    fn field_at_max_length_passes() {
        let exactly_max = "a".repeat(255);
        assert!(validate_required_text("name", &exactly_max, 255).is_ok());
    }

    // --- validate_optional_text ---

    #[test]
    fn none_optional_field_passes() {
        assert!(validate_optional_text("description", &None, 10000).is_ok());
    }

    #[test]
    fn some_optional_field_within_limit_passes() {
        assert!(validate_optional_text("description", &Some("hello".to_string()), 10000).is_ok());
    }

    #[test]
    fn some_optional_field_exceeding_limit_fails() {
        let long = "a".repeat(10001);
        assert!(validate_optional_text("description", &Some(long), 10000).is_err());
    }

    // --- validate_hex_color ---

    #[test]
    fn valid_hex_color_passes() {
        assert!(validate_hex_color("#3B82F6").is_ok());
        assert!(validate_hex_color("#ffffff").is_ok());
        assert!(validate_hex_color("#000000").is_ok());
    }

    #[test]
    fn hex_color_without_hash_fails() {
        assert!(validate_hex_color("3B82F6").is_err());
    }

    #[test]
    fn hex_color_wrong_length_fails() {
        assert!(validate_hex_color("#FFF").is_err());
        assert!(validate_hex_color("#FFFFFFF").is_err());
    }

    #[test]
    fn hex_color_with_invalid_chars_fails() {
        assert!(validate_hex_color("#GGGGGG").is_err());
        assert!(validate_hex_color("#12345Z").is_err());
    }

    // --- validate_priority ---

    #[test]
    fn valid_priorities_pass() {
        for priority in 1..=5 {
            assert!(validate_priority(priority).is_ok());
        }
    }

    #[test]
    fn priority_zero_fails() {
        assert!(validate_priority(0).is_err());
    }

    #[test]
    fn priority_six_fails() {
        assert!(validate_priority(6).is_err());
    }

    // --- validate_task_status ---

    #[test]
    fn valid_task_statuses_pass() {
        for status in &["pending", "in_progress", "done", "cancelled"] {
            assert!(validate_task_status(status).is_ok());
        }
    }

    #[test]
    fn invalid_task_status_fails() {
        assert!(validate_task_status("active").is_err());
        assert!(validate_task_status("").is_err());
    }

    // --- validate_subtask_status ---

    #[test]
    fn valid_subtask_statuses_pass() {
        assert!(validate_subtask_status("pending").is_ok());
        assert!(validate_subtask_status("done").is_ok());
    }

    #[test]
    fn invalid_subtask_status_fails() {
        assert!(validate_subtask_status("in_progress").is_err());
        assert!(validate_subtask_status("cancelled").is_err());
    }

    // --- validate_member_role ---

    #[test]
    fn valid_member_roles_pass() {
        assert!(validate_member_role("owner").is_ok());
        assert!(validate_member_role("editor").is_ok());
        assert!(validate_member_role("viewer").is_ok());
    }

    #[test]
    fn invalid_member_role_fails() {
        assert!(validate_member_role("admin").is_err());
        assert!(validate_member_role("").is_err());
    }
}
