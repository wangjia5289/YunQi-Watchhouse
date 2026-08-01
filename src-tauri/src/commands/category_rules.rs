use tauri::State;

use crate::database::{ActivityRepository, CategoryRule, CategoryRuleInput, CategoryRulePreview};

use super::{IpcResult, run_blocking};

#[tauri::command]
pub fn get_category_rules(
    repository: State<'_, ActivityRepository>,
) -> Result<Vec<CategoryRule>, String> {
    repository
        .category_rules()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn preview_category_rule(
    input: CategoryRuleInput,
    rule_id: Option<i64>,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<CategoryRulePreview> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.preview_category_rule(&input, rule_id)).await
}

#[tauri::command]
pub fn create_category_rule(
    input: CategoryRuleInput,
    repository: State<'_, ActivityRepository>,
) -> Result<CategoryRule, String> {
    repository
        .create_category_rule(&input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_category_rule(
    rule_id: i64,
    input: CategoryRuleInput,
    repository: State<'_, ActivityRepository>,
) -> Result<CategoryRule, String> {
    repository
        .update_category_rule(rule_id, &input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_category_rule(
    rule_id: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    repository
        .delete_category_rule(rule_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reapply_category_rules(repository: State<'_, ActivityRepository>) -> IpcResult<usize> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.reapply_category_rules()).await
}
