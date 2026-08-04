use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

use super::backup_credentials;
use super::backup_crypto::{BackupCryptoError, decrypt_file, encrypt_file};
use crate::{
    AppLocaleState, DatabaseMaintenanceState, TrackingTrayMenuItem,
    activity::{MonitorHandle, SessionManagerHandle},
    database::ActivityRepository,
};

pub(crate) fn create_automatic_encrypted_backup(
    repository: &ActivityRepository,
    destination: &std::path::Path,
) -> Result<(), String> {
    let password = backup_credentials::load()?
        .ok_or_else(|| "automatic encrypted backup password is not configured".to_owned())?;
    let plaintext_directory = tempfile::Builder::new()
        .prefix("watchhouse-auto-encrypted-")
        .tempdir()
        .map_err(|_| "Could not prepare the automatic encrypted backup.".to_owned())?;
    let plaintext = plaintext_directory.path().join("backup.sqlite3");
    repository
        .backup_database(&plaintext)
        .map_err(|_| "Could not create the database backup.".to_owned())?;
    encrypt_file(&plaintext, destination, password.as_bytes()).map_err(encryption_error_message)
}

#[tauri::command]
pub async fn set_automatic_encrypted_backup_password(password: String) -> Result<(), String> {
    let password = Zeroizing::new(password);
    validate_password(&password)?;
    tauri::async_runtime::spawn_blocking(move || backup_credentials::save(&password))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn clear_automatic_encrypted_backup_password(
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    if repository
        .settings()
        .map_err(|error| error.to_string())?
        .automatic_encrypted_backup_enabled
    {
        return Err("Disable automatic encrypted backups before removing its password.".to_owned());
    }
    tauri::async_runtime::spawn_blocking(backup_credentials::delete)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn has_automatic_encrypted_backup_password() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| backup_credentials::load().map(|value| value.is_some()))
        .await
        .map_err(|error| error.to_string())?
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("The backup password cannot be empty.".to_owned());
    }
    if password.chars().count() < 10 {
        return Err("Use at least 10 characters.".to_owned());
    }
    Ok(())
}

#[tauri::command]
pub async fn create_encrypted_database_backup(
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
    password: String,
) -> Result<Option<String>, String> {
    let protected_password = Zeroizing::new(password);
    if protected_password.is_empty() {
        return Err("The backup password cannot be empty.".to_owned());
    }
    if protected_password.chars().count() < 10 {
        return Err("Use at least 10 characters.".to_owned());
    }
    let repository = repository.inner().clone();
    let destination = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_file_name("watchhouse-backup.yqbackup")
            .add_filter("YunQi Encrypted Backup", &["yqbackup"])
            .blocking_save_file()
            .and_then(|path| path.into_path().ok())
    })
    .await
    .map_err(|_| "The encrypted backup dialog could not be opened.".to_owned())?;
    let Some(destination) = destination else {
        return Ok(None);
    };

    let result = tauri::async_runtime::spawn_blocking(move || {
        let plaintext_directory = tempfile::Builder::new()
            .prefix("watchhouse-backup-")
            .tempdir()
            .map_err(|_| "Could not prepare the encrypted database backup.".to_owned())?;
        let plaintext = plaintext_directory.path().join("backup.sqlite3");
        repository
            .backup_database(&plaintext)
            .map_err(|_| "Could not create the database backup.".to_owned())?;
        encrypt_file(&plaintext, &destination, protected_password.as_bytes())
            .map_err(encryption_error_message)?;
        Ok::<_, String>(destination.to_string_lossy().into_owned())
    })
    .await
    .map_err(|_| "The encrypted backup task could not be completed.".to_owned())?;

    result.map(Some)
}

#[tauri::command]
pub async fn restore_encrypted_database_backup(
    app: AppHandle,
    password: String,
) -> Result<bool, String> {
    let protected_password = Zeroizing::new(password);
    if protected_password.is_empty() {
        return Err("The backup password cannot be empty.".to_owned());
    }
    if protected_password.chars().count() < 10 {
        return Err("Use at least 10 characters.".to_owned());
    }
    let dialog_app = app.clone();
    let encrypted_path = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("YunQi Encrypted Backup", &["yqbackup"])
            .blocking_pick_file()
            .and_then(|path| path.into_path().ok())
    })
    .await
    .map_err(|_| "The encrypted restore dialog could not be opened.".to_owned())?;
    let Some(encrypted_path) = encrypted_path else {
        return Ok(false);
    };
    let _maintenance_guard = app.state::<DatabaseMaintenanceState>().try_begin()?;

    let plaintext = tauri::async_runtime::spawn_blocking(move || {
        let plaintext_directory = tempfile::Builder::new()
            .prefix("watchhouse-restore-")
            .tempdir()
            .map_err(|_| "Could not prepare the encrypted database restore.".to_owned())?;
        let plaintext = plaintext_directory.path().join("restore.sqlite3");
        decrypt_file(&encrypted_path, &plaintext, protected_password.as_bytes())
            .map_err(decryption_error_message)?;
        Ok::<_, String>((plaintext_directory, plaintext))
    })
    .await
    .map_err(|_| "The encrypted restore task could not be completed.".to_owned())??;

    let monitor = app.state::<MonitorHandle>();
    let was_paused = monitor.is_paused();
    monitor.set_paused(true);
    if let Some(receiver) = app.state::<SessionManagerHandle>().request_pause() {
        let _ = receiver.await;
    }

    let repository = app.state::<ActivityRepository>().inner().clone();
    let restore_repository = repository.clone();
    let restore_result = tauri::async_runtime::spawn_blocking(move || {
        let (_plaintext_directory, plaintext_path) = plaintext;
        restore_repository.restore_database(&plaintext_path)
    })
    .await
    .map_err(|_| "The database restore task could not be completed.".to_owned())?;
    if restore_result.is_err() {
        if !was_paused {
            monitor.set_paused(false);
            let _ = app.state::<TrackingTrayMenuItem>().0.set_text(
                if app.state::<AppLocaleState>().is_chinese() {
                    "暂停追踪"
                } else {
                    "Pause Tracking"
                },
            );
        }
        return Err("The decrypted backup is not a valid Watchhouse database.".to_owned());
    }

    super::settings::reload_runtime_after_restore(&app, &repository)
        .map_err(|_| "The restored application state could not be loaded.".to_owned())?;
    let _ = app.state::<TrackingTrayMenuItem>().0.set_text(
        if app.state::<AppLocaleState>().is_chinese() {
            "继续追踪"
        } else {
            "Resume Tracking"
        },
    );
    log::info!("encrypted database restore completed; tracking remains paused");
    Ok(true)
}

fn encryption_error_message(error: BackupCryptoError) -> String {
    match error {
        BackupCryptoError::EmptyPassword => "The backup password cannot be empty.",
        BackupCryptoError::Io(_) => "The encrypted backup file could not be written.",
        BackupCryptoError::InvalidFormat
        | BackupCryptoError::AuthenticationFailed
        | BackupCryptoError::KeyDerivationFailed => "The database backup could not be encrypted.",
    }
    .to_owned()
}

fn decryption_error_message(error: BackupCryptoError) -> String {
    match error {
        BackupCryptoError::EmptyPassword => "The backup password cannot be empty.",
        BackupCryptoError::InvalidFormat => "The selected file is not a valid encrypted backup.",
        BackupCryptoError::AuthenticationFailed => {
            "The backup could not be decrypted. Check the password or backup integrity."
        }
        BackupCryptoError::Io(_) => "The encrypted backup file could not be read.",
        BackupCryptoError::KeyDerivationFailed => "The backup decryption key could not be derived.",
    }
    .to_owned()
}
