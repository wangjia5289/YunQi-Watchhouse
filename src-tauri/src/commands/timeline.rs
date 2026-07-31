use tauri::State;

use crate::{
    activity::ActivityState,
    database::{ActivityRepository, ImportRecord, TimelineMutationResult},
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    record_count: usize,
    conflict_count: usize,
    invalid_count: usize,
    started_at_ms: Option<i64>,
    ended_at_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    imported_count: usize,
    merged_count: usize,
    skipped_count: usize,
}

#[tauri::command]
pub fn delete_timeline_session(
    session_id: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<TimelineMutationResult, String> {
    repository
        .delete_closed_sessions(&[session_id])
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_timeline_sessions(
    session_ids: Vec<i64>,
    repository: State<'_, ActivityRepository>,
) -> Result<TimelineMutationResult, String> {
    repository
        .delete_closed_sessions(&session_ids)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn merge_timeline_sessions(
    session_ids: Vec<i64>,
    repository: State<'_, ActivityRepository>,
) -> Result<TimelineMutationResult, String> {
    repository
        .merge_closed_sessions(&session_ids)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn split_timeline_session(
    session_id: i64,
    split_at_ms: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<TimelineMutationResult, String> {
    repository
        .split_closed_session(session_id, split_at_ms)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_timeline_session_notes(
    session_ids: Vec<i64>,
    note: Option<String>,
    repository: State<'_, ActivityRepository>,
) -> Result<usize, String> {
    repository
        .update_session_notes(&session_ids, note.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_timeline_session_categories(
    session_ids: Vec<i64>,
    category: String,
    repository: State<'_, ActivityRepository>,
) -> Result<usize, String> {
    repository
        .update_session_application_categories(&session_ids, &category)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn undo_timeline_edit(
    undo_token: String,
    repository: State<'_, ActivityRepository>,
) -> Result<usize, String> {
    repository
        .undo_timeline_edit(&undo_token)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_timeline_undo_tokens(
    repository: State<'_, ActivityRepository>,
) -> Result<Vec<String>, String> {
    repository
        .timeline_undo_tokens()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_timeline_session(
    session_id: i64,
    started_at_ms: i64,
    ended_at_ms: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    repository
        .update_closed_session_bounds(session_id, started_at_ms, ended_at_ms)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_activity_import(
    contents: String,
    format: String,
    repository: State<'_, ActivityRepository>,
) -> Result<ImportPreview, String> {
    let (records, invalid_count) = parse_import(&contents, &format)?;
    let conflict_count = repository
        .import_conflict_count(&records)
        .map_err(|error| error.to_string())?;
    Ok(ImportPreview {
        record_count: records.len(),
        conflict_count,
        invalid_count,
        started_at_ms: records.iter().map(|record| record.started_at_ms).min(),
        ended_at_ms: records.iter().map(|record| record.ended_at_ms).max(),
    })
}

#[tauri::command]
pub fn import_activity(
    contents: String,
    format: String,
    conflict_policy: String,
    repository: State<'_, ActivityRepository>,
) -> Result<ImportResult, String> {
    if !matches!(conflict_policy.as_str(), "skip" | "merge") {
        return Err("conflict policy must be skip or merge".to_owned());
    }
    let (records, invalid_count) = parse_import(&contents, &format)?;
    if invalid_count > 0 {
        return Err(format!(
            "import contains {invalid_count} invalid records; fix them before importing"
        ));
    }
    let (imported_count, merged_count, skipped_count) = repository
        .import_records(&records, conflict_policy == "merge")
        .map_err(|error| error.to_string())?;
    Ok(ImportResult {
        imported_count,
        merged_count,
        skipped_count,
    })
}

fn parse_import(contents: &str, format: &str) -> Result<(Vec<ImportRecord>, usize), String> {
    match format.to_ascii_lowercase().as_str() {
        "json" => parse_json_import(contents),
        "csv" => parse_csv_import(contents),
        _ => Err("import format must be json or csv".to_owned()),
    }
}

fn parse_json_import(contents: &str) -> Result<(Vec<ImportRecord>, usize), String> {
    let value: serde_json::Value =
        serde_json::from_str(contents).map_err(|error| format!("invalid JSON: {error}"))?;
    let values = value
        .get("records")
        .and_then(serde_json::Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| "JSON must contain a records array".to_owned())?;
    let mut records = Vec::new();
    let mut invalid = 0;
    for value in values {
        let session = value.get("session").unwrap_or(value);
        let application = value.get("application");
        let state = match session.get("state").and_then(serde_json::Value::as_str) {
            Some("ACTIVE") | Some("Active") => ActivityState::Active,
            Some("IDLE") | Some("Idle") => ActivityState::Idle,
            _ => {
                invalid += 1;
                continue;
            }
        };
        let Some(started_at_ms) = session
            .get("startedAtMs")
            .or_else(|| session.get("started_at_ms"))
            .and_then(serde_json::Value::as_i64)
        else {
            invalid += 1;
            continue;
        };
        let Some(ended_at_ms) = session
            .get("endedAtMs")
            .or_else(|| session.get("ended_at_ms"))
            .and_then(serde_json::Value::as_i64)
        else {
            invalid += 1;
            continue;
        };
        let record = ImportRecord {
            state,
            application_name: application
                .and_then(|value| value.get("name"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            bundle_identifier: application
                .and_then(|value| value.get("bundleId").or_else(|| value.get("bundle_id")))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            started_at_ms,
            ended_at_ms,
            window_title: session
                .get("windowTitle")
                .or_else(|| session.get("window_title"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            note: session
                .get("note")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        };
        if !valid_import_record(&record) {
            invalid += 1;
        } else {
            records.push(record);
        }
    }
    Ok((records, invalid))
}

fn parse_csv_import(contents: &str) -> Result<(Vec<ImportRecord>, usize), String> {
    let mut lines = contents.lines().filter(|line| !line.starts_with('#'));
    let header = lines.next().ok_or_else(|| "CSV is empty".to_owned())?;
    let headers = csv_fields(header);
    let column = |name: &str| headers.iter().position(|value| value == name);
    let state_column = column("state").ok_or_else(|| "CSV is missing state".to_owned())?;
    let start_column =
        column("started_at_ms").ok_or_else(|| "CSV is missing started_at_ms".to_owned())?;
    let end_column =
        column("ended_at_ms").ok_or_else(|| "CSV is missing ended_at_ms".to_owned())?;
    let mut records = Vec::new();
    let mut invalid = 0;
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let fields = csv_fields(line);
        let parsed = (|| {
            let state = match fields.get(state_column)?.as_str() {
                "Active" | "ACTIVE" => ActivityState::Active,
                "Idle" | "IDLE" => ActivityState::Idle,
                _ => return None,
            };
            Some(ImportRecord {
                state,
                application_name: column("application_name")
                    .and_then(|index| fields.get(index))
                    .filter(|value| !value.is_empty())
                    .cloned(),
                bundle_identifier: column("bundle_identifier")
                    .and_then(|index| fields.get(index))
                    .filter(|value| !value.is_empty())
                    .cloned(),
                started_at_ms: fields.get(start_column)?.parse().ok()?,
                ended_at_ms: fields.get(end_column)?.parse().ok()?,
                window_title: column("window_title")
                    .and_then(|index| fields.get(index))
                    .filter(|value| !value.is_empty())
                    .cloned(),
                note: column("note")
                    .and_then(|index| fields.get(index))
                    .filter(|value| !value.is_empty())
                    .cloned(),
            })
        })();
        match parsed {
            Some(record) if valid_import_record(&record) => records.push(record),
            None => invalid += 1,
            Some(_) => invalid += 1,
        }
    }
    Ok((records, invalid))
}

fn valid_import_record(record: &ImportRecord) -> bool {
    record.ended_at_ms > record.started_at_ms
        && (record.state == ActivityState::Idle
            || record
                .application_name
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty()))
        && record
            .note
            .as_ref()
            .is_none_or(|note| note.chars().count() <= 500)
}

fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(field);
                field = String::new();
            }
            _ => field.push(character),
        }
    }
    fields.push(field);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_watchhouse_csv_with_quoted_names() {
        let csv = "# watchhouse_export_schema=1\nsession_id,state,application_name,bundle_identifier,started_at_ms,ended_at_ms,duration_ms,closed_reason\n1,Active,\"A, B\",\"com.test\",10,20,10,Shutdown\n";
        let (records, invalid) = parse_csv_import(csv).unwrap();
        assert_eq!(invalid, 0);
        assert_eq!(records[0].application_name.as_deref(), Some("A, B"));
    }
}
