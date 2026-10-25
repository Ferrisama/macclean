# Testing the native Mac app

MacClean is a SwiftUI macOS app backed by a Rust executable. Playwright controls web pages, so it cannot exercise the app's native windows. Use the automated checks below, then perform a window pass on a disposable fixture.

## Automated checks

On macOS with Xcode, Rust, Swift, and `jq` installed, run from any directory:

```bash
/path/to/macclean/scripts/test-app.sh
```

This runs Rust formatting and tests, Swift tests, builds the signed app bundle, and exercises the packaged backend with temporary scan, duplicate, cleanup, History, and restore fixtures. New target fixtures also check editor caches, Cargo project output, temporary targets, active-file rejection after review, and preservation of source/settings files. Version fixtures additionally test Rust, Node, Python, and obsolete extensions, with pins added after review and exact-path restore. A passing run checks the backend and app logic; it does not prove that the visible controls work.

In a logged-in macOS desktop session, also run:

```bash
./scripts/test-ui.sh
```

This hosts the actual SwiftUI views with disposable in-memory data. It checks
Explore and History at 860×600 and 1440×900, circuit branch selection without a scan, and
scrolling a long review sheet. Screenshots are saved to `dist/ui-snapshots/`.
These checks do not delete files or start real scans. The separate packaged
backend smoke test retains legacy duplicate/version fixtures for compatibility.

## Human window pass

Create disposable data and launch the built app:

```bash
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/macclean-ui.XXXXXX")"
mkdir -p "$TEST_ROOT/Library/Caches/example" "$TEST_ROOT/duplicates" "$TEST_ROOT/folder"
printf 'cache fixture\n' > "$TEST_ROOT/Library/Caches/example/payload.txt"
printf 'duplicate fixture\n' > "$TEST_ROOT/duplicates/a.txt"
cp "$TEST_ROOT/duplicates/a.txt" "$TEST_ROOT/duplicates/b.txt"
printf 'scan fixture\n' > "$TEST_ROOT/folder/file.txt"
printf 'Fixture folder: %s\n' "$TEST_ROOT"
open /path/to/macclean/dist/MacClean.app
```

Enter `$TEST_ROOT` in the app's Path field. Replace it with the actual path printed by `mktemp` if needed. Check these flows in both a wide window and the smallest allowed window:

| Area | Action | Expected result |
|---|---|---|
| Startup | Open the app | Explore and History only; no automatic home scan or cache traversal |
| Scan | Scan folder on the disposable fixture | Full tree retained beyond display depth/limits; correct scope and freshness, incomplete coverage visible |
| Explorer | Select a circuit branch, then open its folder; navigate deep and back/forward | Selection opens compact details or the wide inspector; navigation uses the loaded tree without rescanning |
| Caches | Choose Find caches; inspect Stremio, Cargo, npm or pnpm if installed | Only known locations; settings/sign-in data excluded; no default selection |
| Collection | Add individual paths or Add safe caches | One shared tray; overlapping paths deduplicated; review-only sources are not bulk-added |
| Review | Check selected paths, then confirm only disposable fixture paths | Replaced/active targets fail; exact identity-bound paths move to Trash |
| Results and History | Inspect outcomes and restore the fixture session | Per-path failures visible; exact recorded destinations restored without overwrites |
| Recent | Complete a scan, reopen it from Recent, restart the app | Recent scopes persist; clearing recents does not erase receipts |
| Activity | Run App activity or Folder activity; change age and Unknown filters | Read-only Spotlight metadata; unknown/stale dates do not authorize cleanup |
| Settings | Check access and open System Settings | Permission limitations explained; no maintenance controls |
| Cancellation | Stop a scan or cache lookup | Workers exit before another mutation can begin; cancelled results do not replace current state |
| Layout | Scroll and select at compact/wide sizes | Controls fit; map, inspector, and review remain usable |

After confirming the restored fixture, remove the disposable `TEST_ROOT` folder yourself. Do not use personal folders for destructive UI checks.

## Native UI automation

The Swift package includes opt-in AppKit-hosted rendered tests through `scripts/test-ui.sh`. It has no XCUITest runner against the packaged app (`dev.macclean.app`). OS dialogs, trackpad momentum, real scan performance, and full cleanup/restore flows still need the human window pass above. A successful automated check alone is not a claim of full UI coverage.

## Retained compatibility checks

The broad legacy CLI remains available. Its duplicate/version safety and restore
fixtures remain in the packaged smoke script, but those workflows are no longer
native app destinations. Bulk scanning and persistent backend queries remain
separate engine work in the lean storage plan.

For capacity/search changes, verify capacity appears before any scan and remains
separate from measured scope bytes. Check its unavailable/retry state. After a
full scan, find a deeply nested filename from the root, search a path, change
kind/size filters, expand results, clear the query, and show a result in its
containing folder. Navigation must use the loaded inventory. Rapidly replace a
query and start a new scan to confirm old results do not reappear. The 100K-file
unit fixture checks full counts and bounded results; it is not a release benchmark.

Check branch bars with an 80/15/5 split, a single child, zero allocated bytes,
partial coverage and unknown totals. Every sibling must use the same measured
folder denominator, and row height must remain readable. On a disposable scope
with more than 512 classified cleanup roots, verify full candidate counts, filter
matches and Show next expansion. Existing review/identity/restore protections must
still apply to candidates beyond the original first batch.
