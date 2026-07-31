use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};

use crate::database::{ActivityRepository, MaintenanceResult};

const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

pub fn run_due(
    repository: &ActivityRepository,
    app_data: &Path,
    now_ms: i64,
) -> Result<(), String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    if now_ms.saturating_sub(settings.last_maintenance_at_ms) >= DAY_MS {
        run_cleanup(repository, app_data, now_ms, settings.retention_days)?;
    }
    if settings.automatic_backup_enabled {
        let interval_ms = if settings.backup_interval == "DAILY" {
            DAY_MS
        } else {
            7 * DAY_MS
        };
        if now_ms.saturating_sub(settings.last_backup_at_ms) >= interval_ms {
            create_automatic_backup(repository, app_data, now_ms)?;
        }
    }
    Ok(())
}

pub fn run_cleanup(
    repository: &ActivityRepository,
    app_data: &Path,
    now_ms: i64,
    retention_days: i64,
) -> Result<MaintenanceResult, String> {
    let result = repository
        .run_maintenance(now_ms, retention_days)
        .map_err(|error| error.to_string())?;
    remove_application_icons(app_data, &result.deleted_application_ids);
    Ok(result)
}

pub fn create_automatic_backup(
    repository: &ActivityRepository,
    app_data: &Path,
    now_ms: i64,
) -> Result<PathBuf, String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    let directory = settings
        .backup_directory
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| app_data.join("backups"));
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = DateTime::<Utc>::from_timestamp_millis(now_ms)
        .ok_or_else(|| "backup timestamp is outside the supported range".to_owned())?
        .format("%Y%m%d-%H%M%S");
    let destination = directory.join(format!("watchhouse-auto-{timestamp}.sqlite3"));
    repository
        .backup_database(&destination)
        .map_err(|error| error.to_string())?;
    repository
        .mark_backup_completed(now_ms)
        .map_err(|error| error.to_string())?;
    prune_backups(&directory, settings.backup_keep_count as usize)?;
    Ok(destination)
}

fn remove_application_icons(app_data: &Path, application_ids: &[i64]) {
    let directory = app_data.join("icons");
    for application_id in application_ids {
        for extension in ["png", "revision"] {
            let _ = fs::remove_file(directory.join(format!("{application_id}.{extension}")));
        }
    }
}

fn prune_backups(directory: &Path, keep_count: usize) -> Result<(), String> {
    let mut backups = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name.starts_with("watchhouse-auto-") && name.ends_with(".sqlite3")
            })
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in backups.into_iter().skip(keep_count) {
        fs::remove_file(entry.path()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::database::Database;

    use super::*;

    #[test]
    fn automatic_backups_are_rotated_and_completion_is_recorded() {
        let repository =
            ActivityRepository::new(Database::in_memory().expect("database should initialize"));
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let mut settings = repository.settings().expect("settings should load");
        settings.backup_directory = Some(directory.path().to_string_lossy().into_owned());
        settings.backup_keep_count = 2;
        repository
            .update_settings(&settings, 1)
            .expect("settings should update");

        for timestamp in [1_000, 2_000, 3_000] {
            create_automatic_backup(&repository, directory.path(), timestamp)
                .expect("backup should succeed");
        }

        let backups = fs::read_dir(directory.path())
            .expect("backup directory should be readable")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("watchhouse-auto-"))
            })
            .count();
        assert_eq!(backups, 2);
        assert_eq!(
            repository
                .settings()
                .expect("settings should reload")
                .last_backup_at_ms,
            3_000
        );
    }
}
