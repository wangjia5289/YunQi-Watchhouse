use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::{AppError, AppResult};

use super::{ActivityRepository, now_millis};

const ORGANIZATION_NAME_MAX_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub archived: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub archived: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTagInput {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOrganization {
    pub project: Option<Project>,
    pub tags: Vec<ActivityTag>,
}

impl ActivityRepository {
    pub fn list_projects(&self, include_archived: bool) -> AppResult<Vec<Project>> {
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, name, color, archived, created_at_ms, updated_at_ms
             FROM projects
             WHERE ?1 OR archived = 0
             ORDER BY archived, name COLLATE NOCASE, id",
        )?;
        Ok(statement
            .query_map([include_archived], map_project)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_project(&self, input: &ProjectInput) -> AppResult<Project> {
        let (name, color) = normalize_organization_input(&input.name, &input.color, "project")?;
        let connection = self.database.lock()?;
        ensure_project_name_available(&connection, &name, None)?;
        let now_ms = now_millis();
        connection.execute(
            "INSERT INTO projects (name, color, archived, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, 0, ?3, ?3)",
            params![name, color, now_ms],
        )?;
        find_project(&connection, connection.last_insert_rowid())?
            .ok_or_else(|| AppError::InvalidSession("created project was not found".to_owned()))
    }

    pub fn update_project(&self, project_id: i64, input: &ProjectInput) -> AppResult<Project> {
        let (name, color) = normalize_organization_input(&input.name, &input.color, "project")?;
        let connection = self.database.lock()?;
        if find_project(&connection, project_id)?.is_none() {
            return Err(AppError::InvalidSession("project was not found".to_owned()));
        }
        ensure_project_name_available(&connection, &name, Some(project_id))?;
        connection.execute(
            "UPDATE projects
             SET name = ?2, color = ?3, updated_at_ms = MAX(updated_at_ms, ?4)
             WHERE id = ?1",
            params![project_id, name, color, now_millis()],
        )?;
        find_project(&connection, project_id)?
            .ok_or_else(|| AppError::InvalidSession("updated project was not found".to_owned()))
    }

    pub fn set_project_archived(&self, project_id: i64, archived: bool) -> AppResult<Project> {
        let connection = self.database.lock()?;
        if find_project(&connection, project_id)?.is_none() {
            return Err(AppError::InvalidSession("project was not found".to_owned()));
        }
        connection.execute(
            "UPDATE projects
             SET archived = ?2, updated_at_ms = MAX(updated_at_ms, ?3)
             WHERE id = ?1",
            params![project_id, archived, now_millis()],
        )?;
        find_project(&connection, project_id)?
            .ok_or_else(|| AppError::InvalidSession("updated project was not found".to_owned()))
    }

    pub fn list_activity_tags(&self, include_archived: bool) -> AppResult<Vec<ActivityTag>> {
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, name, color, archived, created_at_ms, updated_at_ms
             FROM activity_tags
             WHERE ?1 OR archived = 0
             ORDER BY archived, name COLLATE NOCASE, id",
        )?;
        Ok(statement
            .query_map([include_archived], map_activity_tag)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_activity_tag(&self, input: &ActivityTagInput) -> AppResult<ActivityTag> {
        let (name, color) = normalize_organization_input(&input.name, &input.color, "tag")?;
        let connection = self.database.lock()?;
        ensure_activity_tag_name_available(&connection, &name, None)?;
        let now_ms = now_millis();
        connection.execute(
            "INSERT INTO activity_tags (name, color, archived, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, 0, ?3, ?3)",
            params![name, color, now_ms],
        )?;
        find_activity_tag(&connection, connection.last_insert_rowid())?.ok_or_else(|| {
            AppError::InvalidSession("created activity tag was not found".to_owned())
        })
    }

    pub fn update_activity_tag(
        &self,
        tag_id: i64,
        input: &ActivityTagInput,
    ) -> AppResult<ActivityTag> {
        let (name, color) = normalize_organization_input(&input.name, &input.color, "tag")?;
        let connection = self.database.lock()?;
        if find_activity_tag(&connection, tag_id)?.is_none() {
            return Err(AppError::InvalidSession(
                "activity tag was not found".to_owned(),
            ));
        }
        ensure_activity_tag_name_available(&connection, &name, Some(tag_id))?;
        connection.execute(
            "UPDATE activity_tags
             SET name = ?2, color = ?3, updated_at_ms = MAX(updated_at_ms, ?4)
             WHERE id = ?1",
            params![tag_id, name, color, now_millis()],
        )?;
        find_activity_tag(&connection, tag_id)?.ok_or_else(|| {
            AppError::InvalidSession("updated activity tag was not found".to_owned())
        })
    }

    pub fn set_activity_tag_archived(&self, tag_id: i64, archived: bool) -> AppResult<ActivityTag> {
        let connection = self.database.lock()?;
        if find_activity_tag(&connection, tag_id)?.is_none() {
            return Err(AppError::InvalidSession(
                "activity tag was not found".to_owned(),
            ));
        }
        connection.execute(
            "UPDATE activity_tags
             SET archived = ?2, updated_at_ms = MAX(updated_at_ms, ?3)
             WHERE id = ?1",
            params![tag_id, archived, now_millis()],
        )?;
        find_activity_tag(&connection, tag_id)?.ok_or_else(|| {
            AppError::InvalidSession("updated activity tag was not found".to_owned())
        })
    }

    pub fn get_session_organization(&self, session_id: i64) -> AppResult<SessionOrganization> {
        let connection = self.database.lock()?;
        ensure_session_exists(&connection, session_id)?;
        session_organization_from_connection(&connection, session_id)
    }

    pub fn set_session_organization(
        &self,
        session_id: i64,
        project_id: Option<i64>,
        tag_ids: &[i64],
    ) -> AppResult<SessionOrganization> {
        let unique_tag_ids = tag_ids.iter().copied().collect::<BTreeSet<_>>();
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        ensure_closed_session(&transaction, session_id)?;
        replace_session_organization(&transaction, session_id, project_id, &unique_tag_ids)?;

        let organization = session_organization_from_connection(&transaction, session_id)?;
        transaction.commit()?;
        Ok(organization)
    }

    pub fn update_closed_session_with_organization(
        &self,
        session_id: i64,
        started_at_ms: i64,
        ended_at_ms: i64,
        project_id: Option<i64>,
        tag_ids: &[i64],
    ) -> AppResult<()> {
        if ended_at_ms <= started_at_ms {
            return Err(AppError::InvalidTimeRange(
                "session end must be after its start".to_owned(),
            ));
        }
        let unique_tag_ids = tag_ids.iter().copied().collect::<BTreeSet<_>>();
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        ensure_closed_session(&transaction, session_id)?;
        let overlaps_another: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM activity_sessions
                WHERE id != ?1 AND started_at_ms < ?3 AND ended_at_ms > ?2
            )",
            params![session_id, started_at_ms, ended_at_ms],
            |row| row.get(0),
        )?;
        if overlaps_another {
            return Err(AppError::InvalidTimeRange(
                "edited session cannot overlap another session".to_owned(),
            ));
        }
        replace_session_organization(&transaction, session_id, project_id, &unique_tag_ids)?;
        transaction.execute(
            "UPDATE activity_sessions
             SET started_at_ms = ?2, ended_at_ms = ?3,
                 duration_ms = ?3 - ?2, updated_at_ms = ?3
             WHERE id = ?1",
            params![session_id, started_at_ms, ended_at_ms],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn replace_session_organization(
    connection: &Connection,
    session_id: i64,
    project_id: Option<i64>,
    tag_ids: &BTreeSet<i64>,
) -> AppResult<()> {
    if let Some(project_id) = project_id {
        ensure_project_assignable(connection, project_id)?;
    }
    for tag_id in tag_ids {
        ensure_activity_tag_assignable(connection, *tag_id)?;
    }

    connection.execute(
        "DELETE FROM session_projects WHERE session_id = ?1",
        [session_id],
    )?;
    if let Some(project_id) = project_id {
        connection.execute(
            "INSERT INTO session_projects (session_id, project_id) VALUES (?1, ?2)",
            params![session_id, project_id],
        )?;
    }
    connection.execute(
        "DELETE FROM session_tags WHERE session_id = ?1",
        [session_id],
    )?;
    for tag_id in tag_ids {
        connection.execute(
            "INSERT INTO session_tags (session_id, tag_id) VALUES (?1, ?2)",
            params![session_id, tag_id],
        )?;
    }
    Ok(())
}

fn normalize_organization_input(
    name: &str,
    color: &str,
    entity: &str,
) -> AppResult<(String, String)> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > ORGANIZATION_NAME_MAX_CHARS {
        return Err(AppError::InvalidSession(format!(
            "{entity} name must contain between 1 and {ORGANIZATION_NAME_MAX_CHARS} characters"
        )));
    }
    let color = color.trim();
    if color.len() != 7
        || !color.starts_with('#')
        || !color.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AppError::InvalidSession(format!(
            "{entity} color must use #RRGGBB"
        )));
    }
    Ok((name.to_owned(), color.to_ascii_uppercase()))
}

fn ensure_project_name_available(
    connection: &Connection,
    name: &str,
    except_id: Option<i64>,
) -> AppResult<()> {
    let duplicate = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM projects
           WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id != ?2)
         )",
        params![name, except_id],
        |row| row.get::<_, bool>(0),
    )?;
    if duplicate {
        return Err(AppError::InvalidSession(
            "a project with this name already exists".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_activity_tag_name_available(
    connection: &Connection,
    name: &str,
    except_id: Option<i64>,
) -> AppResult<()> {
    let duplicate = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM activity_tags
           WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id != ?2)
         )",
        params![name, except_id],
        |row| row.get::<_, bool>(0),
    )?;
    if duplicate {
        return Err(AppError::InvalidSession(
            "an activity tag with this name already exists".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_session_exists(connection: &Connection, session_id: i64) -> AppResult<()> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM activity_sessions WHERE id = ?1)",
        [session_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Err(AppError::SessionNotFound(session_id));
    }
    Ok(())
}

fn ensure_closed_session(connection: &Connection, session_id: i64) -> AppResult<()> {
    let is_open = connection
        .query_row(
            "SELECT is_open FROM activity_sessions WHERE id = ?1",
            [session_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .ok_or(AppError::SessionNotFound(session_id))?;
    if is_open {
        return Err(AppError::InvalidSession(
            "projects and tags can only be assigned to closed sessions".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_project_assignable(connection: &Connection, project_id: i64) -> AppResult<()> {
    let archived = connection
        .query_row(
            "SELECT archived FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::InvalidSession("project was not found".to_owned()))?;
    if archived {
        return Err(AppError::InvalidSession(
            "archived projects cannot be assigned".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_activity_tag_assignable(connection: &Connection, tag_id: i64) -> AppResult<()> {
    let archived = connection
        .query_row(
            "SELECT archived FROM activity_tags WHERE id = ?1",
            [tag_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::InvalidSession("activity tag was not found".to_owned()))?;
    if archived {
        return Err(AppError::InvalidSession(
            "archived activity tags cannot be assigned".to_owned(),
        ));
    }
    Ok(())
}

fn session_organization_from_connection(
    connection: &Connection,
    session_id: i64,
) -> AppResult<SessionOrganization> {
    let project = connection
        .query_row(
            "SELECT p.id, p.name, p.color, p.archived, p.created_at_ms, p.updated_at_ms
             FROM session_projects AS sp
             JOIN projects AS p ON p.id = sp.project_id
             WHERE sp.session_id = ?1",
            [session_id],
            map_project,
        )
        .optional()?;
    let mut statement = connection.prepare(
        "SELECT t.id, t.name, t.color, t.archived, t.created_at_ms, t.updated_at_ms
         FROM session_tags AS st
         JOIN activity_tags AS t ON t.id = st.tag_id
         WHERE st.session_id = ?1
         ORDER BY t.name COLLATE NOCASE, t.id",
    )?;
    let tags = statement
        .query_map([session_id], map_activity_tag)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SessionOrganization { project, tags })
}

fn find_project(connection: &Connection, project_id: i64) -> AppResult<Option<Project>> {
    connection
        .query_row(
            "SELECT id, name, color, archived, created_at_ms, updated_at_ms
             FROM projects WHERE id = ?1",
            [project_id],
            map_project,
        )
        .optional()
        .map_err(Into::into)
}

fn find_activity_tag(connection: &Connection, tag_id: i64) -> AppResult<Option<ActivityTag>> {
    connection
        .query_row(
            "SELECT id, name, color, archived, created_at_ms, updated_at_ms
             FROM activity_tags WHERE id = ?1",
            [tag_id],
            map_activity_tag,
        )
        .optional()
        .map_err(Into::into)
}

fn map_project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        archived: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
    })
}

fn map_activity_tag(row: &Row<'_>) -> rusqlite::Result<ActivityTag> {
    Ok(ActivityTag {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        archived: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        activity::{ActivityState, ClosedReason, NewSession},
        database::Database,
    };

    use super::*;

    fn repository() -> ActivityRepository {
        ActivityRepository::new(Database::in_memory().expect("database should open"))
    }

    fn closed_session(repository: &ActivityRepository, started_at_ms: i64) -> i64 {
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
                started_at_ms,
            })
            .expect("session should be created");
        repository
            .close_session(session.id, started_at_ms + 100, ClosedReason::BecameActive)
            .expect("session should close")
            .id
    }

    fn project_input(name: &str, color: &str) -> ProjectInput {
        ProjectInput {
            name: name.to_owned(),
            color: color.to_owned(),
        }
    }

    fn tag_input(name: &str, color: &str) -> ActivityTagInput {
        ActivityTagInput {
            name: name.to_owned(),
            color: color.to_owned(),
        }
    }

    #[test]
    fn project_crud_normalizes_values_and_filters_archived_rows() {
        let repository = repository();
        let project = repository
            .create_project(&project_input("  Client Work  ", "#1a2b3c"))
            .expect("project should be created");
        assert_eq!(project.name, "Client Work");
        assert_eq!(project.color, "#1A2B3C");
        assert!(!project.archived);
        assert!(
            repository
                .create_project(&project_input("client work", "#FFFFFF"))
                .is_err(),
            "project names must be unique without regard to case"
        );

        let updated = repository
            .update_project(project.id, &project_input("Primary Client", "#ABCDEF"))
            .expect("project should update");
        assert_eq!(updated.name, "Primary Client");
        let archived = repository
            .set_project_archived(project.id, true)
            .expect("project should archive");
        assert!(archived.archived);
        assert!(repository.list_projects(false).unwrap().is_empty());
        assert_eq!(repository.list_projects(true).unwrap(), vec![archived]);
    }

    #[test]
    fn activity_tag_crud_enforces_case_insensitive_names() {
        let repository = repository();
        let tag = repository
            .create_activity_tag(&tag_input("Deep Work", "#445566"))
            .expect("tag should be created");
        assert!(
            repository
                .create_activity_tag(&tag_input("DEEP WORK", "#778899"))
                .is_err()
        );
        let updated = repository
            .update_activity_tag(tag.id, &tag_input("Focus", "#AABBCC"))
            .expect("tag should update");
        assert_eq!(updated.name, "Focus");
        let archived = repository
            .set_activity_tag_archived(tag.id, true)
            .expect("tag should archive");
        assert!(archived.archived);
        assert!(repository.list_activity_tags(false).unwrap().is_empty());
        assert_eq!(repository.list_activity_tags(true).unwrap(), vec![archived]);
    }

    #[test]
    fn organization_inputs_reject_invalid_names_and_colors() {
        let repository = repository();
        for input in [
            project_input("   ", "#112233"),
            project_input(&"x".repeat(81), "#112233"),
            project_input("Valid", "112233"),
            project_input("Valid", "#12345G"),
            project_input("Valid", "#1234567"),
        ] {
            assert!(repository.create_project(&input).is_err());
        }
        assert!(
            repository
                .create_activity_tag(&tag_input("Tag", "#12 456"))
                .is_err()
        );
    }

    #[test]
    fn session_organization_is_atomic_deduplicated_and_closed_only() {
        let repository = repository();
        let first_project = repository
            .create_project(&project_input("First", "#111111"))
            .unwrap();
        let second_project = repository
            .create_project(&project_input("Second", "#222222"))
            .unwrap();
        let first_tag = repository
            .create_activity_tag(&tag_input("Alpha", "#AAAAAA"))
            .unwrap();
        let second_tag = repository
            .create_activity_tag(&tag_input("Beta", "#BBBBBB"))
            .unwrap();
        let session_id = closed_session(&repository, 100);

        let organization = repository
            .set_session_organization(
                session_id,
                Some(first_project.id),
                &[second_tag.id, first_tag.id, second_tag.id],
            )
            .expect("organization should be assigned");
        assert_eq!(organization.project, Some(first_project.clone()));
        assert_eq!(
            organization.tags,
            vec![first_tag.clone(), second_tag.clone()]
        );

        repository
            .set_activity_tag_archived(second_tag.id, true)
            .unwrap();
        assert!(
            repository
                .set_session_organization(
                    session_id,
                    Some(second_project.id),
                    &[first_tag.id, second_tag.id],
                )
                .is_err()
        );
        let preserved = repository.get_session_organization(session_id).unwrap();
        assert_eq!(
            preserved.project.map(|project| project.id),
            Some(first_project.id),
            "a rejected assignment must not replace the existing project"
        );
        assert_eq!(
            preserved.tags.iter().map(|tag| tag.id).collect::<Vec<_>>(),
            vec![first_tag.id, second_tag.id],
            "a rejected assignment must not partially replace existing tags"
        );
        assert!(preserved.tags[1].archived);

        repository
            .set_project_archived(second_project.id, true)
            .unwrap();
        assert!(
            repository
                .set_session_organization(session_id, Some(second_project.id), &[first_tag.id])
                .is_err()
        );
        assert!(
            repository
                .set_session_organization(session_id, Some(99_999), &[first_tag.id])
                .is_err()
        );
        assert!(
            repository
                .set_session_organization(session_id, None, &[99_999])
                .is_err()
        );
        assert_eq!(
            repository
                .get_session_organization(session_id)
                .unwrap()
                .project
                .map(|project| project.id),
            Some(first_project.id)
        );

        let open = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
                started_at_ms: 1_000,
            })
            .unwrap();
        assert!(
            repository
                .set_session_organization(open.id, Some(second_project.id), &[])
                .is_err()
        );
    }

    #[test]
    fn time_and_organization_update_is_atomic() {
        let repository = repository();
        let original_project = repository
            .create_project(&project_input("Original", "#111111"))
            .unwrap();
        let replacement_project = repository
            .create_project(&project_input("Replacement", "#222222"))
            .unwrap();
        let active_tag = repository
            .create_activity_tag(&tag_input("Active", "#333333"))
            .unwrap();
        let archived_tag = repository
            .create_activity_tag(&tag_input("Archived", "#444444"))
            .unwrap();
        let session_id = closed_session(&repository, 100);
        repository
            .set_session_organization(session_id, Some(original_project.id), &[active_tag.id])
            .unwrap();
        repository
            .set_activity_tag_archived(archived_tag.id, true)
            .unwrap();

        assert!(
            repository
                .update_closed_session_with_organization(
                    session_id,
                    120,
                    220,
                    Some(replacement_project.id),
                    &[archived_tag.id],
                )
                .is_err()
        );

        let session = repository
            .records_overlapping(0, 300)
            .unwrap()
            .into_iter()
            .find(|record| record.session.id == session_id)
            .unwrap()
            .session;
        assert_eq!((session.started_at_ms, session.ended_at_ms), (100, 200));
        let organization = repository.get_session_organization(session_id).unwrap();
        assert_eq!(organization.project.unwrap().id, original_project.id);
        assert_eq!(organization.tags, vec![active_tag]);
    }

    #[test]
    fn archiving_preserves_history_and_session_deletion_cascades_assignments() {
        let repository = repository();
        let project = repository
            .create_project(&project_input("Historic", "#123456"))
            .unwrap();
        let tag = repository
            .create_activity_tag(&tag_input("Reviewed", "#654321"))
            .unwrap();
        let session_id = closed_session(&repository, 100);
        repository
            .set_session_organization(session_id, Some(project.id), &[tag.id])
            .unwrap();
        repository.set_project_archived(project.id, true).unwrap();
        repository.set_activity_tag_archived(tag.id, true).unwrap();

        let organization = repository.get_session_organization(session_id).unwrap();
        assert!(organization.project.unwrap().archived);
        assert!(organization.tags[0].archived);
        {
            let connection = repository.database.lock().unwrap();
            assert!(
                connection
                    .execute("DELETE FROM projects WHERE id = ?1", [project.id])
                    .is_err(),
                "assigned projects must not be physically deleted"
            );
            assert!(
                connection
                    .execute("DELETE FROM activity_tags WHERE id = ?1", [tag.id])
                    .is_err(),
                "assigned tags must not be physically deleted"
            );
            connection
                .execute("DELETE FROM activity_sessions WHERE id = ?1", [session_id])
                .expect("session should be deleted");
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM session_projects", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM session_tags", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        assert_eq!(repository.list_projects(true).unwrap().len(), 1);
        assert_eq!(repository.list_activity_tags(true).unwrap().len(), 1);
    }
}
