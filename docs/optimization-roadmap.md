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

## Known Environment Blocker

Codex can modify workspace files but the current sandbox may reject writes to `.git`.
If commit or push approval fails, preserve the verified working tree and report the exact
commit message and commands needed. Never bypass the sandbox or use force push.
