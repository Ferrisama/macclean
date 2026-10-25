# MacCleanApp

Native SwiftUI shell for the Rust `macclean` backend.

See [the single product plan](../ROADMAP.md) for the current
model, UI direction, competitor audit, and ordered roadmap.

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

The bundle embeds the release Rust backend in `Contents/Resources/macclean` and uses the stable local development signing identity when available.

The app resolves the backend in this order:

1. `MACCLEAN_BIN`
2. The backend embedded in the `.app` bundle
3. `../target/release/macclean`
4. `../target/debug/macclean`
5. `macclean` on `PATH`

## Current Scope

Explore and History, with a black/red circuit explorer, contextual inspection, one
review tray, on-demand known-cache discovery, recent scan folders, and read-only
app/folder last-opened metadata. Add safe caches is explicit and never preselects
review-only Cargo sources/checkouts. Cleanup uses reviewed identities, activity
checks, Trash receipts, and exact-destination restore.

App activity checks standard application folders. Folder activity checks the
current visible scan folders. Unknown dates remain Unknown and never establish
that data is unused. No uninstall or automatic age-based removal is exposed.

Startup loads cached scan and access status without eager recipes, a home scan,
or maintenance probes. The broader legacy CLI is retained while engine pruning
and bulk scanning remain separate migration slices. Complete scans retain a flat
file/folder inventory, and descendant navigation uses it without rescanning.

## Full Disk Access

MacClean probes protected Mail, Messages, and Safari locations to determine whether scans can read privacy-protected storage. If access is unavailable, the app explains that totals may be incomplete and links directly to the Full Disk Access pane in System Settings. Access can be checked again from Settings.

The explorer shows OS volume capacity separately from the scanned scope. After a
full scan, search names/paths across the loaded tree; Filters narrows by file/folder
and allocated size. Results show their full locations and offer Show in containing
folder without a new scan. Caches and Opportunities use local list filters.

Circuit branch bars show each child’s share of its current folder’s measured
allocation. Cleanup discovery has no 512-item cap; Caches/Opportunities show full
counts with expandable 100-row batches. Add safe caches includes matching rows
not currently displayed, followed by the usual review and preflight.
