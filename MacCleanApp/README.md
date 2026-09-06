# MacCleanApp

Native SwiftUI shell for the Rust `macclean` backend.

See [the living roadmap](../docs/SWIFTUI_ROADMAP.md) for current priorities and the fluid pull queue.
See [the storage intelligence architecture](../docs/STORAGE_INTELLIGENCE_ARCHITECTURE.md) for the target system, current limitations, and migration design.

## Run

From the repository root:

```bash
cargo build --release
cd MacCleanApp
swift run MacCleanApp
```

## Build the App Bundle

From the repository root:

```bash
./scripts/build-swiftui-app.sh
open dist/MacClean.app
```

The bundle embeds the release Rust backend in `Contents/Resources/macclean` and is ad-hoc signed for local use.

The app resolves the backend in this order:

1. `MACCLEAN_BIN`
2. The backend embedded in the `.app` bundle
3. `../target/release/macclean`
4. `../target/debug/macclean`
5. `macclean` on `PATH`

## Current Scope

- Dashboard cards from `app-scan`
- Storage map and largest-items list
- Safety tier summary
- Cleanup review candidates
- Developer-focused cleanup list
- Basic monitor cards from the Rust health snapshot

- Cleanup confirmation and Trash-backed execution
- Cleanup history and per-session restore
- Full Disk Access detection, onboarding, status diagnostics, and System Settings shortcut

## Full Disk Access

MacClean probes protected Mail, Messages, and Safari locations to determine whether scans can read privacy-protected storage. If access is unavailable, the app explains that totals may be incomplete and links directly to the Full Disk Access pane in System Settings. Permission is checked again whenever the app becomes active.
