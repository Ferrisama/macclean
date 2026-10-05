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

This hosts the actual SwiftUI views in native windows with disposable in-memory data. It checks page width and scrolling at 860×600 and 1440×900, clicks cleanup rows and checkboxes, selects a map tile before scrolling, and verifies that temporary build output requires manual selection. Screenshots are saved to `dist/ui-snapshots/`. These checks do not delete files or start real scans.

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
| Dashboard and toolbar | Click Scan, then choose Deep Scan from the ellipsis menu. Turn System Data and Health on and off in that menu. | Scan finishes, sizes and status appear, and the app stays responsive. |
| Safe Cleanup | Open Safe Cleanup, sort through the size-ranked list, search for a cache, and use Find Hidden Caches. | Known caches appear without map navigation; a deep home scan adds eligible hidden folders and the list stays largest first. |
| Developer | Click Find Home Build Output, inspect a verified Cargo target, and select it manually. | Target is marked Review; project files outside the target stay excluded. Running Rust builds block cleanup. |
| Temporary Builds | Click Find Temporary Builds and review the largest verified targets. | Only owned Cargo output is offered; arbitrary temporary folders are excluded. |
| Versions Review | Click Check Versions, add an external project folder if needed, inspect protection reasons, and review an eligible old version. Add a project pin before confirming removal. | Defaults, pinned versions, Python virtual environments, and registered extensions stay protected. The new pin blocks execution despite the earlier review token. Successful removals restore from History. |
| Editor caches | Refresh recipes and review Editor and Codex Caches. | Known cache paths appear; VS Code User data and Codex sessions remain excluded. Close relevant apps before cleanup. |
| Map | Single-click a tile to inspect; double-click `folder` or use Open Folder. Use Back, Forward, Parent, root, and breadcrumb controls; change sort and filters. | The path and visible items follow each control. Revisiting a scanned folder is immediate and does not start another scan. |
| Duplicates | Scan `$TEST_ROOT/duplicates` with the minimum size set low enough for the fixture; choose a keeper, review the other copy, move it to Trash, then restore it from History. | Two identical files form one group; the keeper stays in place and the other copy returns after restore. |
| Clean | Review a selectable cache item, run the dry-run/review step, then confirm moving only the fixture item to Trash. | The item disappears from its original path, a result appears, and unrelated files remain. |
| History | Open the new session and restore it. | The fixture returns to its original path and the session reflects the restore. |
| Uninstall | Search installed apps and open standard/deep plan previews. | The plan is read-only; no app is removed. |
| Access, Developer, Monitor | Open each screen and inspect status, recipes, and health cards. | Each screen loads and errors or partial access are explained clearly. |
| Scrolling | Scroll every long page, expand a cleanup row, select checkboxes, and continue scrolling. Repeat with a mouse wheel and trackpad. | The main page continues to scroll, with no trapped gestures or clipped controls. |
| Scan interruption | Start a Deep Scan on a larger disposable folder and click Stop. | Progress stops and another scan can be started. |

After confirming the restored fixture, remove the disposable `TEST_ROOT` folder yourself. Do not use personal folders for destructive UI checks.

## Native UI automation

The Swift package includes opt-in AppKit-hosted rendered tests through `scripts/test-ui.sh`. It has no XCUITest runner against the packaged app (`dev.macclean.app`). OS dialogs, trackpad momentum, real scan performance, and full cleanup/restore flows still need the human window pass above. A successful automated check alone is not a claim of full UI coverage.

## Version review coverage

Versions Review inventories standard `~/.rustup/toolchains`, `~/.nvm/versions/node`,
`~/.pyenv/versions`, `~/.vscode/extensions`, and `~/.cursor/extensions` locations.
It reads defaults, Rust directory overrides, the current process environment,
`rust-toolchain`/`rust-toolchain.toml`, `.nvmrc`, `.node-version`, `.python-version`,
`.pyenv-version`, `.tool-versions`, and Node `package.json` engine requirements.
Node ranges and unresolved aliases are treated conservatively and may protect
all Node versions. Custom and linked toolchains remain protected.

Home projects and explicitly supplied additional roots are checked. Generated
folders, manager data, Library, and Git data are skipped. Depth is capped at 16
and entries at 100,000 per root. Incomplete coverage blocks removal; usage in
unscanned locations is unknown. The app accepts one additional project root;
the CLI accepts repeated `--projects` arguments. Manager installations in
custom locations are outside this first version of the inventory.

Extension cleanup requires a valid package identity, an obsolete marker, no
registration in the editor or its profiles, and a newer registered copy that
will remain. Project recommendations with an exact `id@version` are protected.
Missing or malformed registration metadata blocks cleanup. Check open files
and pin requirements again during review and immediately before each move.
Removal uses Trash and History, preserving manager defaults and editor state.

Reference formats: [Rust overrides](https://rust-lang.github.io/rustup/overrides.html),
[nvm project versions](https://github.com/nvm-sh/nvm#nvmrc),
[pyenv version selection](https://github.com/pyenv/pyenv#understanding-python-version-selection).
