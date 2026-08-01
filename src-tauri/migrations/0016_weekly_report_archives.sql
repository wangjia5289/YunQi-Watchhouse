CREATE TABLE weekly_report_archives (
    week_start_date TEXT PRIMARY KEY,
    week_end_date TEXT NOT NULL,
    generated_at_ms INTEGER NOT NULL,
    active_duration_ms INTEGER NOT NULL CHECK(active_duration_ms >= 0),
    idle_duration_ms INTEGER NOT NULL CHECK(idle_duration_ms >= 0),
    previous_week_active_duration_ms INTEGER NOT NULL CHECK(previous_week_active_duration_ms >= 0),
    strongest_day_date TEXT,
    peak_hour INTEGER CHECK(peak_hour IS NULL OR (peak_hour >= 0 AND peak_hour <= 23)),
    leading_category TEXT,
    focus_completion_rate INTEGER CHECK(
        focus_completion_rate IS NULL OR
        (focus_completion_rate >= 0 AND focus_completion_rate <= 100)
    ),
    payload_json TEXT NOT NULL,
    notified_at_ms INTEGER
);

CREATE INDEX idx_weekly_report_archives_generated
ON weekly_report_archives(generated_at_ms DESC);
