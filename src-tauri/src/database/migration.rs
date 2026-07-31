use rusqlite_migration::{M, Migrations};

pub(super) fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/0001_initial.sql")),
        M::up(include_str!("../../migrations/0002_onboarding.sql")),
        M::up(include_str!(
            "../../migrations/0003_application_preferences.sql"
        )),
        M::up(include_str!("../../migrations/0004_data_maintenance.sql")),
        M::up(include_str!("../../migrations/0005_focus_settings.sql")),
        M::up(include_str!("../../migrations/0006_timeline_editing.sql")),
        M::up(include_str!(
            "../../migrations/0007_window_title_privacy.sql"
        )),
        M::up(include_str!("../../migrations/0008_focus_mode_state.sql")),
        M::up(include_str!("../../migrations/0009_focus_plans.sql")),
        M::up(include_str!("../../migrations/0010_focus_templates.sql")),
    ])
}
