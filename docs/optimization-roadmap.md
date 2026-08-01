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

## Phase 9: Flexible Reports And Data Care

Status: Complete

- [x] Add custom report date ranges.
- [x] Export report summaries, daily trends, hourly usage, and categories as CSV.
- [x] Add persisted Focus Plan templates with create, start, and delete actions.
- [x] Add data health checks for overlapping and zero-duration closed sessions.
- [x] Add confirmed repair for safely trimming overlaps and removing zero-duration records.

Decisions:

- Custom date ranges use local calendar-day boundaries and an exclusive end timestamp.
- Report CSV files are generated by the backend and saved through the native file dialog.
- Focus templates are local database records with validated names and 5-240 minute durations.
- Health repair never changes an open session and requires explicit confirmation in Settings.

## Phase 10: Safety, Scale And Delivery

Status: Complete

- [x] Create an automatic SQLite backup before every data health repair.
- [x] Persist a repair snapshot and allow the most recent repair to be undone.
- [x] Load Timeline sessions from a backend pagination API with exact day totals.
- [x] Automatically complete paginated data when overview or filters require the full day.
- [x] Add Focus Plan template editing, ordering, start counts, and completion rates.
- [x] Persist template identity across application restarts and plan completion.
- [x] Add macOS GitHub Actions checks for frontend build, Rust tests, formatting, and Clippy.

Decisions:

- Health repair undo restores only the sessions affected by the repair; the full safety backup is
  retained separately in the local backups directory.
- Timeline pages contain at most 1,000 sessions and expose exact total count and durations.
- Template statistics use a persisted template ID rather than matching plans by name or duration.

## Phase 11: Fast Control, Search And Performance

Status: Complete

- [x] Add global shortcuts to toggle, pause, and start Focus Plans.
- [x] Show Focus Plan countdown and template launch actions in the tray menu.
- [x] Search Timeline notes and redacted window titles.
- [x] Filter Timeline sessions by duration and local time of day.
- [x] Add a deterministic one-year SQLite performance baseline.

Decisions:

- Global shortcuts use the official Tauri plugin and default to `Cmd+Shift+F`,
  `Cmd+Shift+P`, and `Cmd+Shift+1`.
- Advanced Timeline filters complete the selected day's pagination before filtering so results
  are exact.
- The performance baseline creates 35,040 deterministic sessions and measures day pagination and
  full-year totals independently.

## Phase 12: Chinese And English Interface

Status: Complete

- [x] Detect Chinese and English from the operating system on first launch.
- [x] Add an always-available language switch in the application sidebar.
- [x] Persist the selected interface language locally.
- [x] Translate primary pages, dialogs, empty states, controls, and accessibility labels.
- [x] Localize tray actions, Focus countdowns, and native notification copy.
- [x] Localize native confirmations and dynamic maintenance or editing results.
- [x] Keep English source text as the fallback for newly introduced copy.

Decisions:

- The locale preference is device-specific UI state and is stored in local browser storage rather
  than the activity database or exported settings.
- Existing screens use a compatibility translator while new screens can migrate incrementally to
  explicit translation keys without changing the persisted locale contract.
- Locale changes are synchronized to the Rust runtime so native tray state and notifications stay
  consistent with the visible interface.

## Phase 13: Configurable Shortcuts And Explicit Localization

Status: Complete

- [x] Persist global shortcut preferences independently from activity settings.
- [x] Allow each Focus shortcut to be changed or disabled.
- [x] Detect duplicate or unavailable shortcuts and restore the previous registration on failure.
- [x] Move the application shell to explicit locale lookups.
- [x] Add localization coverage tests for primary surfaces and dynamic messages.

Decisions:

- Shortcut choices use validated presets to avoid storing platform-specific strings that cannot be
  registered on the current operating system.
- Shortcut persistence is committed only after every enabled shortcut registers successfully.
- The compatibility translator remains for complex legacy screens while components migrate to the
  explicit `t()` API.

## Phase 14: Complete Explicit Localization

Status: Complete

- [x] Move every application screen and dialog to explicit `t()` lookups.
- [x] Localize dates, clocks, and durations using the selected interface language.
- [x] Translate dynamic Timeline editing, import, filtering, and accessibility copy.
- [x] Add an AST-based source check for raw English JSX text and attributes.
- [x] Verify literal `t()` calls have a Chinese translation or an intentional fallback.
- [x] Remove the DOM traversal and MutationObserver compatibility translator.

Decisions:

- User-owned content such as application names, categories, window titles, notes, paths, and
  diagnostic errors remains unchanged.
- English source copy remains the runtime fallback, while source tests prevent untranslated
  literals from entering interface components unnoticed.
- Locale-sensitive formatting uses `zh-CN` or `en` explicitly instead of depending on the host
  process locale.

Validation:

- 9 frontend localization and formatting tests passed.
- Frontend production build passed.
- 74 Rust tests passed; the deterministic performance baseline remained ignored by default.
- Rust formatting and Clippy passed with warnings denied.
- Chinese and English switching was visually checked at desktop and 375 px widths.
- The macOS application bundle was generated successfully.

## Phase 15: Backend Timeline Search

Status: Complete

- [x] Move Timeline text, state, duration, and local-time filtering into SQLite.
- [x] Return exact filtered session counts and active or idle duration totals.
- [x] Preserve paginated loading in the detail view while completing overview data on demand.
- [x] Debounce text search and enforce the shared 200-character query limit.
- [x] Treat SQLite `LIKE` wildcard characters as literal search input.
- [x] Add repository and statistics tests for filtering, totals, and pagination.

Decisions:

- Timeline search matches application names, bundle identifiers, categories, redacted window
  titles, and user notes without loading the full selected day into the frontend.
- Duration totals and filters use session bounds clipped to the selected local calendar day.
- The hourly overview completes filtered pagination because it needs every matching session to
  construct accurate time blocks; the detail view remains incrementally paginated.

Validation:

- 9 frontend tests passed.
- Frontend production build passed.
- 76 Rust tests passed; the deterministic performance baseline remained ignored by default.
- Rust formatting and Clippy passed with warnings denied.

## Phase 16: Cross-Date Activity Search

Status: Complete

- [x] Add a dedicated cross-date Search view for the last 7 days, last 30 days, or a custom range.
- [x] Search and filter up to 366 inclusive local calendar days in SQLite.
- [x] Return newest results first with exact active, idle, and matching-session totals.
- [x] Group loaded results by local date and paginate without loading the full range.
- [x] Open a result in its single-day Timeline, load its page, scroll to it, and highlight it.
- [x] Save, apply, replace, and remove named searches in device-local interface storage.
- [x] Validate date, duration, and clock bounds in both the interface and backend.
- [x] Preserve hourly Timeline progress through repeated daylight-saving hours.

Decisions:

- Cross-date searches are limited to 366 days so an accidental broad query cannot scan an
  unbounded activity history.
- Global results use descending `(started_at_ms, id)` ordering, while the single-day Timeline
  retains chronological ordering.
- Global search does not refresh on every activity checkpoint because that would discard loaded
  pages and repeatedly scan long ranges.
- Saved searches are device-specific interface state; preset ranges remain relative to today,
  while custom ranges preserve their explicit dates.
- User-owned application names, categories, window titles, notes, and saved-search names remain
  untranslated.

Validation:

- 13 frontend localization, date-model, grouping, and daylight-saving tests passed.
- Frontend production build passed.
- 83 Rust tests passed; the deterministic performance baseline remained ignored by default.
- Rust formatting and Clippy passed with warnings denied.
- Chinese and English Search views were visually checked at desktop, 375 px, and 320 px widths.
- Mobile layouts had no horizontal overflow.

## Phase 17: Application And Category Usage Limits

Status: Complete

- [x] Add application and category usage-limit rules with separate weekday and weekend budgets.
- [x] Allow rules and their native notifications to be enabled independently.
- [x] Show today's used time, limit, remaining time, and progress on the Dashboard.
- [x] Send native notifications at 80% and 100% of a rule's daily limit.
- [x] Persist notification delivery so each rule, date, and threshold is announced only once.
- [x] Add complete Chinese and English copy for rule management, progress, and notifications.

Decisions:

- Usage is calculated from local-day session intersections so open sessions and sessions crossing
  midnight contribute only to the selected day.
- Application rules follow stable application IDs; category rules follow the configured category
  name and immediately reflect later activity categorization.
- Dashboard progress prioritizes reached and approaching limits, then the highest percentage.
- Threshold delivery is backend-driven and persisted because the interface may be hidden or closed.
- Settings reads a compact application and category target list instead of aggregating the full
  activity history just to populate a picker.

Validation:

- 18 frontend tests passed, including usage-limit input validation and Chinese duration formatting.
- Frontend production build passed.
- 89 Rust tests passed; the deterministic performance baseline remained ignored by default.
- Rust formatting and Clippy passed with warnings denied.
- The macOS application bundle was generated and passed ad-hoc strict code-signature verification.
- Interactive visual checks were blocked because the current execution environment denied local
  port binding after the initial preview server stopped.

## Phase 18: Usage Limit Reminder Center

Status: Complete

- [x] Add a reminder center with delivered 80% and 100% notification history.
- [x] Allow a rule's notifications to be delayed for 30 minutes or muted for the local day.
- [x] Add temporary daily allowance increases without changing the regular weekday or weekend rule.
- [x] Show effective daily limits and temporary adjustments immediately across Settings and Today.
- [x] Apply the existing quiet-hours schedule to both focus-break and usage-limit notifications.
- [x] Keep reminder history readable after a rule changes or is deleted.
- [x] Clear reminder history, deduplication records, and daily exceptions with Delete All Activity.

Decisions:

- Daily exceptions are keyed by rule and local calendar date, so they expire naturally at the next
  local day without a cleanup job.
- Reminder history stores an immutable target snapshot so later application, category, or rule
  changes do not rewrite what was actually delivered.
- Existing installations backfill prior delivered reminders while filtering the legacy synthetic
  80% marker created when a rule first crossed directly to 100%.
- Temporary allowance increases are cumulative but capped at 1,440 minutes for a single day.

## Phase 19: Anime-Inspired Visual System

Status: Complete

- [x] Introduce an original Japanese animation-inspired palette with sky blue, sakura pink, and lavender accents.
- [x] Restyle the application shell, navigation, controls, cards, forms, filters, and progress indicators.
- [x] Add restrained sky, cloud, sun, and starlight decoration to the Dashboard without changing its data flow.
- [x] Redesign Settings with distinct glass-card sections for preferences, usage limits, and reminders.
- [x] Preserve semantic warning, limit, and destructive-action colors throughout the new theme.
- [x] Support dark mode, narrow mobile layouts, and reduced-motion preferences.

Decisions:

- All decorative artwork is CSS-generated and original; no copyrighted characters or franchise assets are used.
- Visual changes stay separate from activity tracking, statistics, limits, reminders, and localization logic.
- Motion is intentionally subtle and disabled when the operating system requests reduced motion.

## Phase 20: Automatic Classification And Local Insights

Status: Complete

- [x] Add prioritized classification rules for application names, bundle identifiers, and window titles.
- [x] Apply automatic categories at the session level so browsers can be classified by context.
- [x] Reapply current rules to historical active sessions and clear stale automatic overrides.
- [x] Use effective session categories consistently in reports, Timeline search, and category limits.
- [x] Add private weekly insights for activity change, strongest day, peak hour, leading category,
  and Focus Plan completion.
- [x] Add complete Chinese and English copy for rules, results, errors, and insights.

Decisions:

- A rule never overwrites the application's manually selected fallback category.
- Lower priority numbers run first; ties retain stable creation order.
- Matching is a case-insensitive substring operation and all analysis remains local.

## Phase 21: Tray Overview And Modular Styles

Status: Complete

- [x] Show today's active time, current application, today's focus time, and closest usage limit in
  the macOS tray menu.
- [x] Refresh tray summaries every 30 seconds and immediately after a locale change.
- [x] Keep existing tracking, Focus Mode, countdown, template, window, and quit actions.
- [x] Move authoritative light and dark theme tokens into `src/styles/tokens.css`.
- [x] Move Reports styles and responsive rules into the Reports feature directory.
- [x] Keep new classification, insights, and updater styles isolated by feature.

## Phase 22: Signed Releases And Updates

Status: Complete

- [x] Build `.app` and `.dmg` bundles by default on macOS.
- [x] Add release-version consistency checks across npm, Cargo, and Tauri configuration.
- [x] Add a tag-triggered universal macOS GitHub Release workflow.
- [x] Configure Developer ID signing, Apple notarization, updater artifact signing, and
  `latest.json` generation through repository secrets.
- [x] Import and verify the Developer ID certificate, verify the updater key pair, and pin release
  actions to reviewed commit hashes before exposing signing credentials.
- [x] Add automatic update checks plus manual check, install, signature verification, and restart.
- [x] Document certificate, notarization, updater-key, secret, tag, and local-build workflows.

Decisions:

- The production updater config is generated only in release CI, so local builds never contain a
  fake public key or unusable endpoint.
- Developer ID signing and Apple notarization cannot be verified locally without the owner's
  certificate and Apple credentials; CI rejects missing secrets instead of publishing an unsigned
  release.

## Phase 23: Daily Control, Data Safety And Quality

Status: Complete

- [x] Add a real menu-bar panel with current activity, daily totals, Focus controls, and limits.
- [x] Preview automatic-classification matches, conflicts, precedence, and recent samples.
- [x] Archive weekly report snapshots locally and deliver optional native notifications.
- [x] Create password-protected backups with authenticated encryption and tamper detection.
- [x] Add an in-app diagnostics center with service health and safety-backup-first repair.
- [x] Add browser UI automation for Chinese and English, primary navigation, rule previews,
  weekly archives, accessibility checks, and 390 px overflow checks.

Decisions:

- The menu-bar panel is a separate fixed-size Tauri window that stops polling while hidden.
- Weekly archives preserve the source report payload plus compact searchable summary fields.
- Encrypted backups derive keys with Argon2id and use chunked XChaCha20-Poly1305; passwords are
  never stored, and isolated temporary plaintext copies are removed automatically.
- Diagnostics repair creates a full SQLite backup before session repair, database optimization,
  and icon-cache refresh.
- Browser fixtures use Tauri's official IPC mocks only in Vite development mode and are excluded
  from production builds.
