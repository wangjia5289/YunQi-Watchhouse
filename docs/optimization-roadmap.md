# YunQi-Watchhouse Optimization Roadmap

This document is the durable implementation record for the ongoing optimization work.
Update it whenever a feature, design decision, validation step, or blocker changes.

## Quality Gate

Every phase must pass:

- `npm run build`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `npm run tauri build -- --bundles app`

## Completed Foundation

- Activity collection, idle detection, crash recovery, and SQLite persistence.
- Dashboard, timeline, application, history, settings, backup, restore, and export views.
- Application categories and category usage filtering.
- Application ignore rules that prevent future sessions from being recorded.
- Timeline filtering and closed-session time correction.
- Global tracking status and pause/resume control.
- Data retention, automatic cleanup, scheduled backups, and backup rotation.

## Phase 1: Runtime Efficiency And Scale

Status: Complete

- [x] Emit backend activity-data events after durable checkpoints and transitions.
- [x] Replace repetitive frontend polling with event subscriptions and focus recovery.
- [x] Expose automatic maintenance status and recent failures.
- [x] Add incremental timeline rendering in stable chronological order.
- [x] Prevent stale asynchronous responses from overwriting newer page state.

Decisions:

- Backend events are notifications, not large data payloads. Views reload their narrow models.
- Keep a low-frequency focus recovery refresh in case a window was suspended.
- Timeline details render in stable chronological batches while full-day aggregates remain exact.

## Phase 2: Focus Experience

Status: Complete

- [x] Calculate focus blocks and application-switch counts.
- [x] Add daily focus targets and progress.
- [x] Add configurable break reminders and quiet hours.
- [x] Add tray actions for status, pause/resume, and focus mode.
- [x] Keep all notifications opt-in.

Validation:

- Frontend production build passed.
- 56 Rust tests passed after Focus Mode integration.
- Focus Mode compilation issue fixed by importing Tauri's `Manager` trait.

## Phase 3: Window Titles And Privacy

Status: Complete

- [x] Report macOS Accessibility permission status.
- [x] Record window titles only when explicitly enabled.
- [x] Add per-application title recording controls.
- [x] Add local redaction rules before persistence.
- [x] Never include captured titles in diagnostic logs.

Decisions:

- Window title collection requires both the global setting and an application-specific opt-in.
- The macOS Accessibility API is never called unless both settings allow collection.
- Accessibility permission is reported without triggering a system prompt.
- Email-like values, URL query strings, secret assignments, and long numeric values are
  redacted in memory before a title reaches session persistence.
- Captured title text is never passed to diagnostic logging.

Validation:

- Frontend production build passed.
- 58 Rust tests passed.
- Clippy passed for all targets with warnings denied.

## Phase 4: Editing And Import

Status: Complete

- [x] Add multi-select timeline operations.
- [x] Merge compatible adjacent sessions.
- [x] Add session notes and batch category changes.
- [x] Add undo for destructive timeline operations.
- [x] Import Watchhouse JSON and CSV exports.
- [x] Preview import counts, conflicts, and date ranges before writing.
- [x] Support transactional skip/merge conflict policies.

Validation:

- Frontend production build passed.
- 58 Rust tests passed.
- Clippy passed for all targets with warnings denied.
- The macOS application bundle was generated successfully.

## Phase 5: Focus Reliability And Editing UX

Status: Complete

- [x] Persist active Focus Mode state and restore it after application restart.
- [x] Deliver native macOS break reminders while the application is hidden.
- [x] Deduplicate reminders by elapsed focus interval and respect quiet hours.
- [x] Replace timeline prompts and confirmations with accessible action dialogs.
- [x] Move closed-session time editing into a dedicated validation dialog.

Decisions:

- Native reminders are driven by the backend so they do not depend on the dashboard being open.
- Focus Mode is persisted in the local settings database and changed atomically before UI events.
- Native notifications use the official Tauri notification plugin and the current macOS
  notification framework.

Validation:

- Frontend production build passed.
- 63 Rust tests passed.
- Clippy passed for all targets with warnings denied.
- The macOS application bundle was generated successfully.
- Timeline layouts were visually checked at desktop and 375 px widths with no horizontal overflow.

## Phase 6: Plans, Reports And Reversible Editing

Status: Complete

- [x] Add timed Focus Plans with pause, resume, early end, and completion notifications.
- [x] Persist active plan state and completed or cancelled plan history.
- [x] Add weekly and monthly reports with previous-period comparisons.
- [x] Add daily trends, hourly activity heatmaps, and category allocation.
- [x] Split closed timeline sessions at a validated timestamp.
- [x] Preserve multiple undo steps and support reversible session splits.

Decisions:

- Reports are calculated from current session data so edits, imports, and category changes are
  reflected immediately without maintaining duplicate aggregates.
- Focus Plan pauses extend the planned end time by the exact paused duration.
- Undo snapshots list both the sessions to restore and current session IDs to remove; legacy
  snapshot arrays remain readable.

Validation:

- Frontend production build passed.
- 66 Rust tests passed.
- Clippy passed for all targets with warnings denied.
- Reports layouts were visually checked at desktop and 375 px widths with no horizontal overflow.
- The macOS application bundle was generated successfully.

## Phase 7: History And Notification Controls

Status: Complete

- [x] Add persistent undo history with timestamps, affected session counts, and expiration hints.
- [x] Add weekly and monthly Focus Plan completion statistics and recent plan history.
- [x] Allow a completed or cancelled timed Focus Plan to be repeated from Reports.
- [x] Report native notification permission state in Settings.
- [x] Allow notification permission requests and test notifications.
- [x] Add repository-level coverage for Focus Plan history range filtering and ordering.

Decisions:

- Undo metadata is derived from existing snapshots, so no database migration is required.
- Undo snapshots remain available for 24 hours and can be selected individually.
- Focus Plan statistics use the plan end time to assign plans to the selected report period.
- Notification state and test delivery use the official Tauri notification plugin.

Validation:

- Frontend production build passed.
- 67 Rust tests passed.
- Clippy passed for all targets with warnings denied.
- Reports and Timeline were visually checked in the browser at desktop width.

## Phase 8: History Context And Focus Consistency

Status: Complete

- [x] Label undo entries by operation type for delete, merge, and split edits.
- [x] Keep legacy unlabeled undo snapshots readable.
- [x] Show the longest completed Focus Plan streak in weekly and monthly reports.
- [x] Deduplicate multiple completed plans on the same day when calculating streaks.

Decisions:

- Operation labels live inside the existing undo snapshot JSON, avoiding a database migration.
- Legacy snapshots fall back to a generic Timeline edit label.
- A streak is the longest run of consecutive local calendar days with at least one completed plan
  inside the selected report period.
