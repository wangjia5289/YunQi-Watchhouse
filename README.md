# YunQi-Watchhouse

A private, local-first computer activity timeline built with Rust, Tauri 2,
SQLite, React, TypeScript, and Vite.

The product is designed around four principles:

- Accurate
- Local
- Lightweight
- Private

The current implementation includes the verified desktop foundation, SQLite
persistence, macOS activity providers, the background monitor, and the Activity
Session state machine. See
[`docs/architecture.md`](docs/architecture.md) for the planned module layout,
core data flow, SQLite schema, and phase boundaries.

## Development

Prerequisites: current Node.js, npm, Rust, and the macOS Tauri prerequisites.

```sh
npm install
npm run tauri dev
```

Verification:

```sh
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --bundles app
```
