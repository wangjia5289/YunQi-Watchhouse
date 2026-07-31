use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Error as UpdaterError, UpdaterExt};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub configured: bool,
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateCheck, String> {
    let current_version = app.package_info().version.to_string();
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(UpdaterError::EmptyEndpoints) => {
            return Ok(UpdateCheck {
                configured: false,
                available: false,
                current_version,
                version: None,
                notes: None,
                published_at: None,
            });
        }
        Err(error) => return Err(error.to_string()),
    };
    let update = updater.check().await.map_err(|error| error.to_string())?;
    Ok(match update {
        Some(update) => UpdateCheck {
            configured: true,
            available: true,
            current_version,
            version: Some(update.version),
            notes: update.body,
            published_at: update.date.map(|date| date.to_string()),
        },
        None => UpdateCheck {
            configured: true,
            available: false,
            current_version,
            version: None,
            notes: None,
            published_at: None,
        },
    })
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "no update is currently available".to_owned())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    app.restart()
}
