use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use chacha20poly1305::aead::rand_core::{OsRng, RngCore};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

use super::backup_crypto::{BackupCryptoError, decrypt_file};
use crate::{
    AppLocaleState, DatabaseMaintenanceState, TrackingTrayMenuItem,
    activity::{MonitorHandle, SessionManagerHandle},
    database::{ActivityRepository, Database},
};

const PREPARED_RESTORE_TTL: Duration = Duration::from_secs(10 * 60);
const PREPARED_RESTORE_TOKEN_BYTES: usize = 24;
const MAX_BACKUP_FILE_NAME_CHARS: usize = 160;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPreview {
    token: String,
    file_name: String,
    encrypted: bool,
    schema_version: u32,
    file_size_bytes: u64,
    application_count: u64,
    session_count: u64,
    earliest_session_at_ms: Option<i64>,
    latest_session_at_ms: Option<i64>,
    weekly_report_count: u64,
}

struct PreparedRestore {
    token: String,
    created_at: SystemTime,
    _directory: tempfile::TempDir,
    database_path: PathBuf,
}

#[derive(Clone, Default)]
pub struct PreparedRestoreState(Arc<Mutex<Option<PreparedRestore>>>);

impl PreparedRestoreState {
    fn replace(&self, prepared: PreparedRestore) -> Result<(), String> {
        *self
            .0
            .lock()
            .map_err(|_| "Could not store the prepared restore.".to_owned())? = Some(prepared);
        Ok(())
    }

    fn take(&self, token: &str) -> Result<PreparedRestore, String> {
        let mut slot = self
            .0
            .lock()
            .map_err(|_| "Could not read the prepared restore.".to_owned())?;
        let expired = slot.as_ref().is_some_and(|prepared| {
            prepared
                .created_at
                .elapsed()
                .unwrap_or(PREPARED_RESTORE_TTL)
                >= PREPARED_RESTORE_TTL
        });
        if expired {
            *slot = None;
            return Err("The restore preview expired. Choose the backup again.".to_owned());
        }
        if slot.as_ref().is_none_or(|prepared| prepared.token != token) {
            return Err("The restore preview is no longer available.".to_owned());
        }
        slot.take()
            .ok_or_else(|| "The restore preview is no longer available.".to_owned())
    }

    fn cancel(&self, token: &str) -> Result<(), String> {
        let mut slot = self
            .0
            .lock()
            .map_err(|_| "Could not read the prepared restore.".to_owned())?;
        if slot
            .as_ref()
            .is_some_and(|prepared| prepared.token == token)
        {
            *slot = None;
        }
        Ok(())
    }

    fn expire(&self, token: &str) {
        if let Ok(mut slot) = self.0.lock()
            && slot
                .as_ref()
                .is_some_and(|prepared| prepared.token == token)
        {
            *slot = None;
        }
    }
}

#[tauri::command]
pub async fn preview_database_restore(app: AppHandle) -> Result<Option<BackupPreview>, String> {
    let dialog_app = app.clone();
    let source = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("SQLite Database", &["sqlite3", "db"])
            .blocking_pick_file()
            .and_then(|path| path.into_path().ok())
    })
    .await
    .map_err(|_| "The database restore dialog could not be opened.".to_owned())?;
    let Some(source) = source else {
        return Ok(None);
    };
    let state = app.state::<PreparedRestoreState>();
    let prepared = tauri::async_runtime::spawn_blocking(move || prepare_plain(source, false))
        .await
        .map_err(|_| "The database restore preview could not be prepared.".to_owned())??;
    let preview = prepared.0;
    state.replace(prepared.1)?;
    schedule_expiration(state.inner().clone(), preview.token.clone());
    Ok(Some(preview))
}

#[tauri::command]
pub async fn preview_encrypted_database_restore(
    app: AppHandle,
    password: String,
) -> Result<Option<BackupPreview>, String> {
    let password = Zeroizing::new(password);
    if password.chars().count() < 10 {
        return Err("Use at least 10 characters.".to_owned());
    }
    let dialog_app = app.clone();
    let source = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("YunQi Encrypted Backup", &["yqbackup"])
            .blocking_pick_file()
            .and_then(|path| path.into_path().ok())
    })
    .await
    .map_err(|_| "The encrypted restore dialog could not be opened.".to_owned())?;
    let Some(source) = source else {
        return Ok(None);
    };
    let state = app.state::<PreparedRestoreState>();
    let prepared =
        tauri::async_runtime::spawn_blocking(move || prepare_encrypted(source, password))
            .await
            .map_err(|_| "The encrypted restore task could not be completed.".to_owned())??;
    let preview = prepared.0;
    state.replace(prepared.1)?;
    schedule_expiration(state.inner().clone(), preview.token.clone());
    Ok(Some(preview))
}

#[tauri::command]
pub fn cancel_prepared_database_restore(
    state: State<'_, PreparedRestoreState>,
    token: String,
) -> Result<(), String> {
    state.cancel(&token)
}

#[tauri::command]
pub async fn restore_prepared_database(app: AppHandle, token: String) -> Result<(), String> {
    let _maintenance_guard = app.state::<DatabaseMaintenanceState>().try_begin()?;
    let prepared = app.state::<PreparedRestoreState>().take(&token)?;
    let monitor = app.state::<MonitorHandle>();
    let was_paused = monitor.is_paused();
    monitor.set_paused(true);
    if let Some(receiver) = app.state::<SessionManagerHandle>().request_pause() {
        let _ = receiver.await;
    }

    let repository = app.state::<ActivityRepository>().inner().clone();
    let restore_repository = repository.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        restore_repository.restore_database(&prepared.database_path)
    })
    .await
    .map_err(|_| "The database restore task could not be completed.".to_owned())?;
    if let Err(error) = result {
        if !was_paused {
            monitor.set_paused(false);
            set_tracking_menu_text(&app, false);
        }
        return Err(error.to_string());
    }
    super::settings::reload_runtime_after_restore(&app, &repository)?;
    set_tracking_menu_text(&app, true);
    log::info!("prepared database restore completed; tracking remains paused");
    Ok(())
}

fn prepare_plain(
    source: PathBuf,
    encrypted: bool,
) -> Result<(BackupPreview, PreparedRestore), String> {
    let source_size = fs::metadata(&source)
        .map_err(|_| "The selected backup could not be read.".to_owned())?
        .len();
    let file_name = backup_file_name(&source, "backup.sqlite3");
    let directory = tempfile::Builder::new()
        .prefix("watchhouse-restore-preview-")
        .tempdir()
        .map_err(|_| "Could not prepare the database restore.".to_owned())?;
    let database_path = directory.path().join("restore.sqlite3");
    create_validated_restore_copy(&source, &database_path)?;
    build_prepared(directory, database_path, file_name, encrypted, source_size)
}

fn prepare_encrypted(
    source: PathBuf,
    password: Zeroizing<String>,
) -> Result<(BackupPreview, PreparedRestore), String> {
    let source_size = fs::metadata(&source)
        .map_err(|_| "The encrypted backup file could not be read.".to_owned())?
        .len();
    let file_name = backup_file_name(&source, "backup.yqbackup");
    let directory = tempfile::Builder::new()
        .prefix("watchhouse-restore-preview-")
        .tempdir()
        .map_err(|_| "Could not prepare the encrypted database restore.".to_owned())?;
    let decrypted_path = directory.path().join("decrypted.sqlite3");
    let database_path = directory.path().join("restore.sqlite3");
    decrypt_file(&source, &decrypted_path, password.as_bytes())
        .map_err(decryption_error_message)?;
    create_validated_restore_copy(&decrypted_path, &database_path)?;
    fs::remove_file(&decrypted_path)
        .map_err(|_| "Could not secure the prepared database restore.".to_owned())?;
    build_prepared(directory, database_path, file_name, true, source_size)
}

fn create_validated_restore_copy(source: &Path, destination: &Path) -> Result<(), String> {
    let database = Database::open(destination)
        .map_err(|_| "Could not prepare the database restore.".to_owned())?;
    let repository = ActivityRepository::new(database.clone());
    repository
        .restore_database(source)
        .map_err(|_| "The selected backup is not a valid Watchhouse database.".to_owned())?;
    database
        .lock()
        .and_then(|connection| {
            connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                .map_err(Into::into)
        })
        .map_err(|_| "Could not finalize the database restore preview.".to_owned())?;
    Ok(())
}

fn build_prepared(
    directory: tempfile::TempDir,
    database_path: PathBuf,
    file_name: String,
    encrypted: bool,
    file_size_bytes: u64,
) -> Result<(BackupPreview, PreparedRestore), String> {
    let token = new_token();
    let mut preview = inspect_database(&database_path)?;
    preview.token = token.clone();
    preview.file_name = file_name;
    preview.encrypted = encrypted;
    preview.file_size_bytes = file_size_bytes;
    Ok((
        preview,
        PreparedRestore {
            token,
            created_at: SystemTime::now(),
            _directory: directory,
            database_path,
        },
    ))
}

fn inspect_database(path: &Path) -> Result<BackupPreview, String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "The selected backup is not a valid Watchhouse database.".to_owned())?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| "The selected backup is not a valid Watchhouse database.".to_owned())?;
    if integrity != "ok"
        || !["applications", "activity_sessions", "settings"]
            .into_iter()
            .all(|table| table_exists(&connection, table))
    {
        return Err("The selected backup is not a valid Watchhouse database.".to_owned());
    }
    let schema_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
        .unwrap_or(0);
    let application_count = count(&connection, "applications")?;
    let session_count = count(&connection, "activity_sessions")?;
    let (earliest_session_at_ms, latest_session_at_ms) = connection
        .query_row(
            "SELECT MIN(started_at_ms), MAX(ended_at_ms) FROM activity_sessions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "The selected backup is not a valid Watchhouse database.".to_owned())?;
    let weekly_report_count = if table_exists(&connection, "weekly_report_archives") {
        count(&connection, "weekly_report_archives")?
    } else {
        0
    };
    Ok(BackupPreview {
        token: String::new(),
        file_name: String::new(),
        encrypted: false,
        schema_version,
        file_size_bytes: 0,
        application_count,
        session_count,
        earliest_session_at_ms,
        latest_session_at_ms,
        weekly_report_count,
    })
}

fn table_exists(connection: &Connection, table: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .unwrap_or(false)
}

fn count(connection: &Connection, table: &str) -> Result<u64, String> {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|count| count.max(0) as u64)
        .map_err(|_| "The selected backup is not a valid Watchhouse database.".to_owned())
}

fn new_token() -> String {
    let mut bytes = [0_u8; PREPARED_RESTORE_TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);
    let mut token = String::with_capacity(PREPARED_RESTORE_TOKEN_BYTES * 2);
    for byte in bytes {
        write!(&mut token, "{byte:02x}").expect("writing to a String cannot fail");
    }
    token
}

fn backup_file_name(path: &Path, fallback: &str) -> String {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback);
    if name.chars().count() <= MAX_BACKUP_FILE_NAME_CHARS {
        return name.to_owned();
    }
    let mut truncated = name
        .chars()
        .take(MAX_BACKUP_FILE_NAME_CHARS.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn schedule_expiration(state: PreparedRestoreState, token: String) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PREPARED_RESTORE_TTL).await;
        state.expire(&token);
    });
}

fn set_tracking_menu_text(app: &AppHandle, paused: bool) {
    let chinese = app.state::<AppLocaleState>().is_chinese();
    let text = match (chinese, paused) {
        (true, true) => "继续追踪",
        (true, false) => "暂停追踪",
        (false, true) => "Resume Tracking",
        (false, false) => "Pause Tracking",
    };
    let _ = app.state::<TrackingTrayMenuItem>().0.set_text(text);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_preview_reports_backup_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("backup.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "PRAGMA user_version = 16;
                 CREATE TABLE applications (id INTEGER PRIMARY KEY);
                 CREATE TABLE settings (singleton_id INTEGER PRIMARY KEY);
                 CREATE TABLE activity_sessions (
                    id INTEGER PRIMARY KEY,
                    started_at_ms INTEGER NOT NULL,
                    ended_at_ms INTEGER NOT NULL
                 );
                 CREATE TABLE weekly_report_archives (week_start_date TEXT PRIMARY KEY);
                 INSERT INTO applications DEFAULT VALUES;
                 INSERT INTO applications DEFAULT VALUES;
                 INSERT INTO activity_sessions (started_at_ms, ended_at_ms) VALUES (100, 200);
                 INSERT INTO activity_sessions (started_at_ms, ended_at_ms) VALUES (300, 450);
                 INSERT INTO weekly_report_archives VALUES ('2026-07-20');",
            )
            .unwrap();
        drop(connection);

        let preview = inspect_database(&path).unwrap();
        assert_eq!(preview.schema_version, 16);
        assert_eq!(preview.application_count, 2);
        assert_eq!(preview.session_count, 2);
        assert_eq!(preview.earliest_session_at_ms, Some(100));
        assert_eq!(preview.latest_session_at_ms, Some(450));
        assert_eq!(preview.weekly_report_count, 1);
    }

    #[test]
    fn prepared_restore_requires_matching_token_and_is_single_use() {
        let state = PreparedRestoreState::default();
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("restore.sqlite3");
        fs::write(&database_path, b"fixture").unwrap();
        state
            .replace(PreparedRestore {
                token: "expected".to_owned(),
                created_at: SystemTime::now(),
                _directory: directory,
                database_path,
            })
            .unwrap();
        assert!(state.take("wrong").is_err());
        assert!(state.take("expected").is_ok());
        assert!(state.take("expected").is_err());
    }

    #[test]
    fn prepared_restore_token_is_random_hex() {
        let first = new_token();
        let second = new_token();

        assert_eq!(first.len(), PREPARED_RESTORE_TOKEN_BYTES * 2);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn cancelling_prepared_restore_removes_its_temporary_directory() {
        let state = PreparedRestoreState::default();
        let directory = tempfile::tempdir().unwrap();
        let directory_path = directory.path().to_path_buf();
        let database_path = directory.path().join("restore.sqlite3");
        fs::write(&database_path, b"fixture").unwrap();
        state
            .replace(PreparedRestore {
                token: "cancel-me".to_owned(),
                created_at: SystemTime::now(),
                _directory: directory,
                database_path,
            })
            .unwrap();

        state.cancel("cancel-me").unwrap();

        assert!(!directory_path.exists());
    }

    #[test]
    fn plain_preview_uses_sqlite_backup_to_include_wal_data() {
        let source_directory = tempfile::tempdir().unwrap();
        let source_path = source_directory.path().join("backup.sqlite3");
        let source_database = Database::open(&source_path).unwrap();
        {
            let connection = source_database.lock().unwrap();
            connection
                .execute_batch(
                    "PRAGMA wal_checkpoint(TRUNCATE);
                     INSERT INTO applications (
                        identity_key, name, first_seen_at_ms, last_seen_at_ms
                     ) VALUES ('test.editor', 'Editor', 100, 100);
                     INSERT INTO activity_sessions (
                        state, application_id, window_title, started_at_ms, ended_at_ms,
                        duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms
                     ) VALUES (
                        'ACTIVE', 1, 'notes.txt', 100, 250,
                        150, 0, 'APP_CHANGED', 100, 250
                     );",
                )
                .unwrap();
        }
        assert!(source_path.with_extension("sqlite3-wal").exists());

        let (preview, prepared) = prepare_plain(source_path, false).unwrap();

        assert_eq!(preview.application_count, 1);
        assert_eq!(preview.session_count, 1);
        assert_eq!(preview.earliest_session_at_ms, Some(100));
        assert_eq!(preview.latest_session_at_ms, Some(250));
        assert!(prepared.database_path.exists());
    }

    #[test]
    fn plain_preview_rejects_non_watchhouse_sqlite() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("not-watchhouse.sqlite3");
        Connection::open(&source_path).unwrap();

        assert!(prepare_plain(source_path, false).is_err());
    }

    #[test]
    fn preview_file_name_never_exposes_parent_path_and_is_bounded() {
        let long_name = format!("{}.sqlite3", "a".repeat(MAX_BACKUP_FILE_NAME_CHARS + 20));
        let path = Path::new("private").join("backups").join(&long_name);
        let displayed = backup_file_name(&path, "backup.sqlite3");

        assert_eq!(displayed.chars().count(), MAX_BACKUP_FILE_NAME_CHARS);
        assert!(!displayed.contains("private"));
        assert!(displayed.ends_with("..."));
    }
}
