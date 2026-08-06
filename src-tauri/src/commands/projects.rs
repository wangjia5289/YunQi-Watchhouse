use tauri::{AppHandle, Emitter, State};

use crate::database::{
    ActivityRepository, ActivityTag, ActivityTagInput, Project, ProjectInput, SessionOrganization,
    SessionTagUpdateMode, TimelineMutationResult,
};

use super::{IpcResult, run_blocking};

#[tauri::command]
pub async fn list_projects(
    include_archived: bool,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Vec<Project>> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.list_projects(include_archived)).await
}

#[tauri::command]
pub async fn create_project(
    input: ProjectInput,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Project> {
    let repository = repository.inner().clone();
    let project = run_blocking(move || repository.create_project(&input)).await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(project)
}

#[tauri::command]
pub async fn update_project(
    project_id: i64,
    input: ProjectInput,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Project> {
    let repository = repository.inner().clone();
    let project = run_blocking(move || repository.update_project(project_id, &input)).await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(project)
}

#[tauri::command]
pub async fn set_project_archived(
    project_id: i64,
    archived: bool,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Project> {
    let repository = repository.inner().clone();
    let project =
        run_blocking(move || repository.set_project_archived(project_id, archived)).await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(project)
}

#[tauri::command]
pub async fn list_activity_tags(
    include_archived: bool,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Vec<ActivityTag>> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.list_activity_tags(include_archived)).await
}

#[tauri::command]
pub async fn create_activity_tag(
    input: ActivityTagInput,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<ActivityTag> {
    let repository = repository.inner().clone();
    let tag = run_blocking(move || repository.create_activity_tag(&input)).await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(tag)
}

#[tauri::command]
pub async fn update_activity_tag(
    tag_id: i64,
    input: ActivityTagInput,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<ActivityTag> {
    let repository = repository.inner().clone();
    let tag = run_blocking(move || repository.update_activity_tag(tag_id, &input)).await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(tag)
}

#[tauri::command]
pub async fn set_activity_tag_archived(
    tag_id: i64,
    archived: bool,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<ActivityTag> {
    let repository = repository.inner().clone();
    let tag = run_blocking(move || repository.set_activity_tag_archived(tag_id, archived)).await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(tag)
}

#[tauri::command]
pub async fn get_session_organization(
    session_id: i64,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<SessionOrganization> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.get_session_organization(session_id)).await
}

#[tauri::command]
pub async fn set_session_organization(
    session_id: i64,
    project_id: Option<i64>,
    tag_ids: Vec<i64>,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<SessionOrganization> {
    let repository = repository.inner().clone();
    let organization =
        run_blocking(move || repository.set_session_organization(session_id, project_id, &tag_ids))
            .await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(organization)
}

#[tauri::command]
pub async fn set_sessions_organization(
    session_ids: Vec<i64>,
    project_id: Option<i64>,
    tag_ids: Vec<i64>,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<TimelineMutationResult> {
    let repository = repository.inner().clone();
    let result = run_blocking(move || {
        repository.set_sessions_organization(&session_ids, project_id, &tag_ids)
    })
    .await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(result)
}

#[tauri::command]
pub async fn update_sessions_tags(
    session_ids: Vec<i64>,
    tag_ids: Vec<i64>,
    mode: SessionTagUpdateMode,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<TimelineMutationResult> {
    let repository = repository.inner().clone();
    let result =
        run_blocking(move || repository.update_sessions_tags(&session_ids, &tag_ids, mode)).await?;
    let _ = app.emit("activity-data-changed", ());
    Ok(result)
}
