use std::fs;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::activity::Application;
use crate::database::ActivityRepository;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationIcon {
    mime_type: &'static str,
    bytes: Vec<u8>,
    revision: String,
}

#[tauri::command]
pub fn update_application_preferences(
    application_id: i64,
    category: String,
    is_ignored: bool,
    repository: State<'_, ActivityRepository>,
) -> Result<Application, String> {
    repository
        .update_application_preferences(application_id, &category, is_ignored)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_application_icon(
    application_id: i64,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<Option<ApplicationIcon>, String> {
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let application = repository
            .application(application_id)
            .map_err(|error| error.to_string())?;
        let Some(executable_path) = application.and_then(|item| item.executable_path) else {
            return Ok(None);
        };

        let icon_directory = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join("icons");
        let icon_path = icon_directory.join(format!("{application_id}.png"));
        let revision_path = icon_directory.join(format!("{application_id}.revision"));

        #[cfg(target_os = "macos")]
        let revision = crate::platform::application_icon_revision(&executable_path);

        #[cfg(not(target_os = "macos"))]
        let revision = String::new();

        let cached_revision = fs::read_to_string(&revision_path).ok();
        let bytes = match fs::read(&icon_path) {
            Ok(bytes)
                if !bytes.is_empty() && cached_revision.as_deref() == Some(revision.as_str()) =>
            {
                bytes
            }
            _ => {
                #[cfg(target_os = "macos")]
                let bytes = crate::platform::application_icon_png(&executable_path)
                    .map_err(|error| error.to_string())?;

                #[cfg(not(target_os = "macos"))]
                return Ok(None);

                fs::create_dir_all(&icon_directory).map_err(|error| error.to_string())?;
                fs::write(&icon_path, &bytes).map_err(|error| error.to_string())?;
                fs::write(&revision_path, &revision).map_err(|error| error.to_string())?;
                bytes
            }
        };

        Ok(Some(ApplicationIcon {
            mime_type: "image/png",
            bytes,
            revision,
        }))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn clear_application_icon_cache(app: AppHandle) -> Result<(), String> {
    let icon_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("icons");
    match fs::remove_dir_all(icon_directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
