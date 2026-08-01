ALTER TABLE settings ADD COLUMN automatic_encrypted_backup_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN last_encrypted_backup_at_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN weekly_report_auto_archive_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN weekly_report_notification_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN weekly_report_notification_weekday INTEGER NOT NULL DEFAULT 1
    CHECK(weekly_report_notification_weekday BETWEEN 1 AND 7);
ALTER TABLE settings ADD COLUMN weekly_report_notification_time TEXT NOT NULL DEFAULT '09:00';
