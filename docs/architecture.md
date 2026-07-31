# YunQi-Watchhouse Architecture

YunQi-Watchhouse is local-first, privacy-first, and designed for continuous
low-overhead operation. Rust owns collection, session rules, persistence, and
statistics. React renders typed view models exposed by narrow Tauri commands.

## Planned directory layout

```text
src/                         React UI
├── app/                     routing, shell, providers
├── components/              reusable presentation components
├── features/
│   ├── dashboard/
│   ├── timeline/
│   ├── applications/
│   ├── history/
│   └── settings/
├── lib/                     typed IPC client and display helpers
└── styles/                  theme tokens and global styles

src-tauri/
├── migrations/              ordered, immutable SQLite migrations
└── src/
    ├── activity/            monitor, state machine, session manager
    ├── commands/            narrow Tauri IPC boundary
    ├── database/            connection, migration, repositories
    ├── permissions/         centralized platform permission status
    ├── platform/            ActivityProvider trait and OS adapters
    ├── settings/            validated settings service
    ├── statistics/          range clipping and aggregate queries
    ├── tray/                tray lifecycle and pause/resume actions
    ├── error.rs             domain and infrastructure errors
    ├── state.rs             application-owned shared services
    ├── lib.rs               composition root
    └── main.rs              desktop entry point
```

Platform adapters will be isolated behind an `ActivityProvider` trait. The
macOS adapter is implemented first; Windows and Linux modules remain
compile-time-selected extension points rather than empty production stubs.

## Core data flow

```text
macOS public APIs
  -> ActivityProvider (idle duration + frontmost application)
  -> Activity monitor (low-cost periodic sample)
  -> Session manager (ACTIVE / IDLE / PAUSED transitions)
  -> Repository (transactional session checkpoints)
  -> SQLite in the Tauri application-data directory

SQLite
  -> Statistics service (clip sessions to requested local-day/range boundaries)
  -> Typed Tauri commands and lightweight status events
  -> React Dashboard / Timeline / Applications / History / Settings
```

The monitor uses wall-clock UTC for persisted timestamps and a monotonic clock
to detect sampling gaps. When idle is first detected, the session manager moves
the boundary back to `now - idle_duration`. A large wall/monotonic sampling gap
closes the previous session at its last trustworthy checkpoint; unknown sleep
time is never attributed to an application.

## SQLite schema plan

All instants are signed Unix epoch milliseconds in UTC. This avoids ambiguous
timezone-free strings while allowing overlap queries and local-time rendering.
`ended_at_ms` is checkpointed while a session is open; `closed_reason` and
`is_open` make recovery explicit.

```sql
CREATE TABLE applications (
  id                INTEGER PRIMARY KEY,
  identity_key      TEXT NOT NULL UNIQUE,
  name              TEXT NOT NULL,
  bundle_id         TEXT,
  executable_path   TEXT,
  first_seen_at_ms  INTEGER NOT NULL,
  last_seen_at_ms   INTEGER NOT NULL,
  CHECK (last_seen_at_ms >= first_seen_at_ms)
);

CREATE TABLE activity_sessions (
  id                INTEGER PRIMARY KEY,
  state             TEXT NOT NULL CHECK (state IN ('ACTIVE', 'IDLE')),
  application_id    INTEGER REFERENCES applications(id),
  window_title      TEXT,
  started_at_ms     INTEGER NOT NULL,
  ended_at_ms       INTEGER NOT NULL,
  duration_ms       INTEGER NOT NULL CHECK (duration_ms >= 0),
  is_open           INTEGER NOT NULL DEFAULT 1 CHECK (is_open IN (0, 1)),
  closed_reason     TEXT,
  created_at_ms     INTEGER NOT NULL,
  updated_at_ms     INTEGER NOT NULL,
  CHECK (
    (state = 'ACTIVE' AND application_id IS NOT NULL) OR
    (state = 'IDLE' AND application_id IS NULL)
  ),
  CHECK (ended_at_ms >= started_at_ms)
);

CREATE INDEX idx_sessions_overlap
  ON activity_sessions(started_at_ms, ended_at_ms);
CREATE INDEX idx_sessions_application_range
  ON activity_sessions(application_id, started_at_ms, ended_at_ms);
CREATE UNIQUE INDEX idx_sessions_one_open
  ON activity_sessions(is_open) WHERE is_open = 1;
```

`identity_key` is derived from the best stable identifier available, in order:
bundle identifier, executable path, then application name. This avoids SQLite's
`NULL` uniqueness behavior creating duplicate application rows.

Settings use a singleton row with database constraints for idle threshold,
automatic tracking, window-title recording, close behavior, and appearance.
The authoritative schema is the numbered migration in
`src-tauri/migrations/0001_initial.sql`; future schema changes are added as new
immutable migrations.

`PAUSED` is intentionally not stored as an activity session: it means collection
was disabled, not that computer activity was observed. Pause/resume timestamps
may later be kept in a separate lifecycle-event table for diagnostics without
polluting activity totals.

## Phase boundaries

Phase 1 establishes the buildable Tauri/React shell and this design contract.
Database dependencies, migrations, and domain models begin in Phase 2. Platform
APIs, background tasks, plugins, and runtime permissions are added only in their
own phases so each layer can be tested before the next is introduced.
