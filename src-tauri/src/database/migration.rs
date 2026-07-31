use rusqlite_migration::{M, Migrations};

pub(super) fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/0001_initial.sql")),
        M::up(include_str!("../../migrations/0002_onboarding.sql")),
        M::up(include_str!(
            "../../migrations/0003_application_preferences.sql"
        )),
    ])
}
