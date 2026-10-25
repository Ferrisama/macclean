# MacClean: product model, critical audit, and roadmap

Updated 2026-10-25. **The only active product, architecture, UI, and delivery plan.**
Superseded planning documents have been removed. README files describe usage,
and docs/APP_TESTING.md describes validation; neither establishes a separate roadmap.

## Verdict and evidence

MacClean is a promising local prototype, not yet a compelling replacement for
DaisyDisk, Mole, or Reclaim. Its strongest implemented investment is reviewed
Trash/recovery. Its weakest part is completing the user's primary task: find
what occupies space, decide what can go, remove it, and see the actual result.
A distinctive appearance cannot compensate for missing actions or unreliable
size semantics. We prioritized UI iteration before completing that loop.

This audit inspected the current working tree and the sibling ../reclaim source.
Competitor product claims below come from official documentation checked on this
date. No matched performance benchmark or hands-on competitor usability study
was performed. Earlier test passes are not new audit validation, and test counts
are not a measure of readiness. Reclaim means this local snapshot, not an assumed
latest public release. Mole CLI and the separate native app are distinguished.

## One product model

**A native Mac storage explorer with guided developer cleanup and accountable results.**
Keep SwiftUI + Rust and the subprocess boundary. Target developers who accumulate
project outputs and package caches, while still letting them inspect and Trash
ordinary personal files. Do not try to become a system optimizer or app manager.

Three tasks share one inventory and one review mechanism:

1. **Explore:** understand disk capacity, inspect a selected scope, search its
   entire loaded tree, navigate and select personal files/folders.
2. **Free up space:** discover supported app caches, package stores, and project
   outputs; explain consequences; collect a reviewed batch across categories.
3. **History:** inspect actual outcomes; restore recoverable Trash operations;
   show permanent command operations honestly as non-restorable.

Explore and Free up space must be visibly discoverable entry points. History is
secondary. Settings and contextual details remain utilities. No new dashboard
suite. No promise to clean “all caches”: offer all eligible discovered items in
supported scopes, with unsupported, busy, protected, and unreadable items visible.

## Comparison across the complete journey

| Task | MacClean today | DaisyDisk | Mole | Local Reclaim | Our decision |
|---|---|---|---|---|---|
| Start and choose scope | Explicit folder scan, recents, cached view; OS volume capacity and separate scope total now delivered locally | Volume overview includes capacity/free space before scanning | CLI cleanup/purge/analyze entry points; native Clean and Analyze | Disk/home/folder entry and folder drop | Show capacity immediately; make exploration and cleanup equally obvious |
| Scan and coverage | Serial per-entry metadata traversal; full flat inventory, partial flags; scale unproven | Established disk/folder exploration, hidden-space explanations; published speed is not our benchmark | CLI analyzer and native disk drill-down; no matched speed result | Bulk metadata, descriptor-relative traversal, Rayon, compact tree | Correct accounting first, then benchmark bounded bulk traversal |
| Search | Whole-inventory name/path search, kind/allocated-size filters, bounded results, cancellation, containing-folder navigation now delivered locally | Spatial drill-down and previews documented; full-text/global filename search not established by this audit | CLI filtering documented; do not assume global indexed search | Category/size/age filters; no text-search implementation found in app.js | Whole scanned-scope filename/path search, explicit scope, results with breadcrumbs |
| Explain space | Circuit hierarchy, proportional branch bars/percentages and OS capacity strip delivered locally; disk reconciliation unfinished | Quantitative sunburst, used/free and hidden/purgeable space explanations | Native quantitative treemap; CLI ranked explorer | Quantitative sunburst, artifact highlighting | Capacity strip plus proportional branch bars; no sunburst or giant tiles |
| Find large personal files | Loaded tree available, no finished dedicated large-file workflow | Visual identification and collector | Analyzer navigation/selection | Dedicated size/category large-files view with protected internals excluded | Global size/type filters and manual Trash policy |
| Find developer output | Standard Cargo targets supported; candidate count cap removed; custom outputs incomplete | Primarily user-driven storage analysis, not a documented developer rule catalog | CLI purge groups rebuildable artifacts by project | Broad marker-based developer rules, custom tagged folders | Selected project roots, verified manifests/configuration, transparent supported providers |
| Find caches | Exact known roots plus user Library/Caches; separate lookup; Caches excludes dev artifacts | User explores and decides | Broad guided cleaning; native reviewed groups | Global roots and rule controls, app caches optional | One Caches & build output surface, grouped by owner/project |
| Clean a batch | Add safe caches + tray exists; discovery fragmented; no owner-tool execution | Collector supports files/folders | CLI clean/purge and native review workflows | Collect all + tray + Trash/permanent choices | One reviewed batch with mixed action semantics clearly separated |
| Delete ordinary folders | No finished general manual Trash workflow | Collector accepts allowed files/folders | CLI analyzer and native analyzer support Trash | Personal-file selection supported within safety boundaries | Independent manual-selection policy; no fake cache classification |
| Exclusions | Backend-owned persistent exclusions not completed | Protected system locations documented; custom exclusion parity not established | CLI whitelist and project scan paths | Never touch list, including ancestor/descendant protection | Persistent backend enforcement across discovery/review/execution |
| Activity and age | Read-only Spotlight dates, Unknown retained; not proof of disuse | No equivalent established here | CLI purge treats recent/unknown activity conservatively | Configurable recent-project suppression | Optional prioritization, never deletion authorization |
| Recovery | Reviewed device/inode/type, actual Trash destinations, receipts, collision-safe restore | Collector deletion is permanent | Analyzer Trash; CLI clean/purge permanent; history logs | Trash/permanent, deletion log; no comparable app restore flow found | Retain recovery advantage without implying permanent actions can be undone |
| Verify outcome | Invalidation and per-item outcomes; volume-change UX missing | Documents why snapshots can delay reclaimed space | CLI logs and native outcome workflows | Before/after volume query; some success values use scanned bytes | Separate selected, moved, removed, and observed volume change |
| Maturity | Local signed build/tests; external device/performance/release verification pending | Established focused commercial product | Broad CLI and separate commercial native product | Smaller focused implementation, not audited as production-safe | Finish a small end-to-end product before expanding |

Official evidence: [DaisyDisk overview](https://daisydiskapp.com/),
[collector/deletion](https://daisydiskapp.com/guide/4/en/DeletingFiles/),
[hidden space](https://web.daisydiskapp.com/guide/hidden-space),
[Mole CLI](https://github.com/tw93/Mole), [Mole native](https://mole.fit/).
These document capabilities, not independent performance or safety guarantees.

## Where we fail, and why

### 1. We do not finish the job

A user can inspect an ordinary folder but cannot simply collect it for Trash.
They can find global caches but miss project-local output in the Caches view.
There is no supported cargo/npm/pnpm command runner. This makes the app feel like
a collection of backend demonstrations instead of a cleaner. Reclaim's collector
and Mole's task-oriented cleanup currently deliver a more coherent journey.

### 2. The visual identity outran the information design

The circuit explorer now adds thin proportional bars and percentages to the
hierarchical rows, addressing the earlier need to compare numbers mentally.
Each bar uses the same current-folder measured allocation denominator; row height
stays constant, including single-child folders. Zero allocation has zero fill;
unknown, zero or inconsistent denominators produce no invented percentage.
Selection stays distinct from cleanup eligibility. Compact/wide layouts have
been checked locally, but keyboard/VoiceOver and real-user task validation still
need broader coverage. No freeform circuit canvas or alternate visualization is
planned. The OS capacity strip provides context separately from the measured tree.

### 3. Whole-inventory search delivered locally; scale validation continues

The former current-folder-only filter has been replaced in All storage by
whole-inventory name/path search. Queries run off the main actor, debounce typing,
check cancellation, and reject results from superseded requests or inventory
instances. Results include kind and allocated-size filters, full paths, the full
match count and a bounded first 100 rows with explicit expansion. Search never
walks the filesystem or grants cleanup eligibility. Caches/Opportunities retain
explicitly labeled list filters.

A result can open its directory or show its containing folder using the loaded
inventory, preserving the selected item. The current implementation uses a
size-sorted node-ID index and linear matching over immutable observations; it is
not a persistent search database. A 100K-entry synthetic test verifies complete
counts and bounded results. Million-entry query latency and UI memory remain
unmeasured. Age/activity search and content indexing are not implemented.

### 4. Candidate truncation fixed; rule coverage remains incomplete

The scanner and app formerly retained only 512 cleanup candidates. Both limits
are removed. Every nonzero discovered cleanup root survives the full-scan result,
serialization/cache and native inventory lookup. Classified parent roots suppress
redundant descendant cleanup roots; this is intentional grouping, not a count
limit. When the chosen scan scope is itself classified, its independently
classified children are now discovered without offering the scope itself.

The UI exposes the full count and renders candidates in expandable batches of
100. Bulk safe-cache selection applies to all matching rows, including undisplayed
rows, and says so explicitly. Old deep-scan caches lacking the updated discovery
version are ignored. The scan catalog is indexed/sorted off the main actor;
merging extra cache/legacy recipe sources and keeping all records in memory still
need large-catalog profiling. UI batching is not bounded backend storage.

This fixes count-based omissions. It does not fix every heuristic, discover
custom configured package stores automatically, infer missing ownership evidence,
or eliminate additional scans of explicitly requested global-cache locations.

### 5. Accounting defects found; first correction delivered locally

The audit found storage.rs substituting logical length when allocated blocks
were zero while the inspector said “Allocated on disk.” This inflated sparse-file
usage. The first implementation slice now preserves allocated blocks, includes
directory allocation, tracks distinct logical totals, and deduplicates hard links
by device plus inode. Full-tree observations and the inspector carry the distinction.
Old deep-scan caches with previous accounting semantics are ignored.

Hard-link bytes are attributed to the first encountered link; per-folder shares
can change with traversal order. Every name remains navigable. These totals are
not predicted savings: links outside the selected scope, shared APFS extents and
snapshots still matter. Volume reconciliation, explicit mount boundaries and
large-scale validation remain unfinished. Fast/legacy measurements are labeled
as estimates rather than being promoted to corrected full-scan measurements.

### 6. Safety is uneven, despite strong recovery work

Exact cache rules coexist with older broad component classifiers, including
node_modules/logs. Labels, discovery, preflight and execution are not yet one
policy authority. Strong identity tokens do not make a bad classification safe.
Persistent exclusions are unfinished. Manual deletion needs its own deliberate
policy, not broader automatic cache eligibility. Activity checks can fail or race;
never describe them as a universal guarantee.

Reclaim has clearer marker/rule organization and exclusion behavior, but its
selection check compares paths rather than our reviewed filesystem identity;
its seen-inode set is inode-only, and exclusions arrive from the UI. Do not copy
its implementation wholesale or repeat its performance/readme claims as proof.

### 7. We have reduced screens, not demonstrated a faster engine

The scan still uses serial read_dir/metadata traversal. Full inventory JSON,
Swift indexing, directory path dictionaries, and per-folder item construction
can consume substantial memory. Rendering only 100 rows does not bound the
underlying model work. Reclaim has more deliberate scanner architecture today;
we cannot honestly claim faster scanning or lower memory than any competitor.

### 8. Planning and branding have drifted

Earlier documents described removed screens, conflicting scope, obsolete test
counts, and contradictory tool-action priorities. The main README still leads
with a broad maintenance CLI because that implementation remains present.
Native simplification did not remove the backend's maintenance surface. This
creates implementation distraction and contradictory expectations. One plan,
one active delivery slice, and explicit legacy documentation are required.

## Where an edge is credible

- **Already present, narrowly:** accountable Trash recovery with exact recorded
  destinations and collision-safe restore; cached descendant navigation; native
  SwiftUI presentation. These are useful foundations, not overall superiority.
- **Worth building:** a single understandable review for Cargo output, app caches,
  configured package stores, and personal files, explaining rebuild/download cost
  and recoverability per action. This combines developer context with a focused UI.
- **Worth differentiating:** whole-inventory search, transparent coverage, strong
  exclusion enforcement, and honest measured outcomes. None is a unique moat by
  itself; their integration could make the app preferable.
- **Do not compete on:** feature count against Mole, sunburst polish against
  DaisyDisk, or unmeasured scanner speed against Reclaim. Black/red colors and a
  microchip motif are identity, not a reason to trust a cleanup tool.

## Scanner decision: fastest, deepest, and architecture

**Fastest: unproven. Deep traversal: implemented for readable selected scope.
Best scanner implementation: no. Keep the application architecture; improve the engine.**

The native full scan recursively enumerates descendants even after visible_depth
reaches zero; that parameter limits the older presentation tree, not traversal.
The flat inventory retains small and empty entries as well. This is useful
completeness, but permissions and enumeration errors still produce partial scans.
Symlink targets are not recursively followed. Unlimited recursion depth is not a
robustness guarantee: extreme nesting needs explicit-stack/queue validation.
Neither a complete inventory nor readable coverage guarantees complete cleanup
classification. The former candidate cap has been removed; unsupported rules and partial coverage can still leave cleanup opportunities unidentified.

The current scanner uses read_dir and per-entry symlink_metadata with serial
recursion. It has no demonstrated optimum worker count or comparative throughput.
The local Reclaim implementation already uses getattrlistbulk, openat and Rayon;
that is a stronger performance-oriented design, not proof of a matched speed win.
Its aggregation of small files also differs from our complete searchable-tree
requirement, so compare equivalent outputs before borrowing benchmark claims.

The chosen engine direction is:

1. Retain the delivered allocated/logical accounting and device+inode hard-link
   identity; finish explicit mount boundaries and coverage validation while
   preserving an exhaustive inventory.
2. A validated macOS bulk-metadata adapter with a conventional fallback and
   descriptor-relative traversal. Avoid repeated path resolution and metadata calls.
3. A bounded directory work queue and worker pool, descriptor/memory budgets,
   cancellation checkpoints and measured worker quiescence. More threads alone
   are not a performance strategy, especially on external or network storage.
4. Compact node IDs and parent references; complete inventory queries for search,
   classification and navigation; bounded result pages and coalesced progress.
   Benchmark the existing process/JSON transfer before replacing it. A loaded
   full tree must not require materializing every row during each UI update.
5. Same-machine, same-scope correctness and performance comparisons covering full
   scan, time to usable results, search/drill-down latency, cancellation and peak
   engine/UI memory. Include cold/warm conditions and partial results.

Rust plus SwiftUI and a subprocess boundary can support this design. Tauri, FFI,
SQLite, a daemon or a different language does not inherently make metadata scans
faster. Change a boundary only when profiling identifies it as the bottleneck.
No “fastest,” “deepest,” or “all space accounted for” marketing claim is justified
until equivalent workloads and coverage measurements support it.

## Target architecture and contracts

One generation-bound inventory supplies navigation, global search, large files,
and project candidates. Explicit global-cache discovery can scan additional
scopes; deduplicate overlapping observations rather than promising literally one
traversal for unrelated user requests. No automatic startup home/tool scans.
Keep the complete searchable tree; virtualize presentation. If memory benchmarks
require it, move queries to a bounded backend index without hiding small files.

One backend rule registry supplies evidence, allowed action, scope/configuration,
activity requirements, exclusions and consequence. Every action is revalidated
at execution. Persist exclusions in backend-owned settings and protect their
ancestors as well as descendants. Preserve original filesystem identity/path
representation; display strings must not authorize a different filesystem path.

Three action types: manual Trash, verified cache/output Trash, supported owner-tool
command. Review freezes the selected generation and deduplicates parent/child and
shared-store actions. Scans must stop before mutation; stale events cannot replace
new state. Record per-action outcomes and invalidate affected observations.

Owner-tool adapters use verified executable paths, fixed argument arrays, explicit
working directories, configuration/version detection, bounded output and explicit
permanent-action acknowledgement. Never auto-run project-defined clean scripts.

| Provider | Initial supported scope | Required distinctions |
|---|---|---|
| Cargo | Validated workspace and resolved build outputs; cargo clean | Version-supported dry-run; custom/shared targets; busy builds; permanent removal |
| npm | Configured cache; cache verify or explicitly selected cache clean --force | Garbage collection versus full removal; neither cleans node_modules |
| pnpm | Detected store; store prune | Unreferenced content only; total store size is not predicted savings |
| Yarn | Version/configuration-specific cache clean | Classic/modern scopes differ; protect tracked/project cache data |
| Stremio/editor caches | Exact supported paths via Trash | Preserve settings, credentials, sessions; activity checks |

Implement providers incrementally. Unsupported/uncertain scope produces a clear
skip, not a best-guess shell command. Cancellation cannot undo permanent work.
History distinguishes recoverable and permanent actions. Trash moves generally do
not immediately free volume space; direct the user to Finder Trash when appropriate,
without quietly adding automatic emptying of Trash.

References: [Cargo](https://doc.rust-lang.org/cargo/commands/cargo-clean.html),
[npm](https://docs.npmjs.com/cli/v11/commands/npm-cache/),
[pnpm](https://pnpm.io/cli/store), [Yarn](https://yarnpkg.com/cli/cache/clean).

## One ordered roadmap

No completion percentage: current code has meaningful foundations but misses
release-critical workflows. Complete one slice before the next.

| Order | Deliverable and current status | Acceptance gate |
|---|---|---|
| 1 | Trustworthy exploration — inventory/navigation, accounting, capacity, search, proportional branches and uncapped candidate discovery delivered locally; coverage/scale validation remains open | Correct byte semantics + hard-link fixtures; OS capacity and scope distinction; partial/stale states; proportional branch bars; whole-inventory search finds deep items without enumeration; no silent candidate truncation |
| 2 | Shared policy + manual Trash — identity/recovery exists, policy incomplete | Backend exclusions; consistent classification/rejection; deliberate personal-file/folder collection; roots/library internals protected; replacement/symlink/overlap/active-work tests; exact receipt and collision-safe restore |
| 3 | Guided Free up space — fragmented cache lookup exists | App/package/project groups in one place; saved explicit project scopes; one batch review; Cargo output visible; excluded/busy/unsupported reasons; no automatic personal-file selection |
| 4 | Owner-tool execution — not implemented | Cargo first, then npm/pnpm, then Yarn; configured scope and version fixtures, custom/shared paths, process failure/cancel tests, honest savings, permanent-action acknowledgement and journal; end-to-end packaged fixture per provider |
| 5 | Measured speed and lean distribution — pending | Profile complete workflow, implement bulk metadata only against correctness oracle; bounded workers/FDs, cancellation, large-directory queries; prune retired CLI/dependencies while preserving receipt compatibility |
| 6 | Release validation — local build/test infrastructure exists | Real-data read-only dogfood, disposable cleanup fixtures, compact/wide keyboard/VoiceOver flows, permission failure, upgrades, target devices, stable signing/notarization and documented supported providers |

If a preceding slice cannot meet interactive limits, resolve that blocking
performance issue then; do not postpone unusable behavior until slice 5. Do not
bundle a scanner rewrite, process-boundary migration, and UI redesign in one patch.

Measure engine/UI RSS and latency at 100K and 1M entries, deep paths, extreme
fan-out, sparse/hard-linked files, invalid filenames, denied reads, mount boundaries,
cancellation and concurrent changes. Five matched runs minimum for performance
claims; report cold/warm conditions, medians/tails, coverage and device details.
Initial targets, not achieved claims: cached navigation p95 <100ms, useful cached
view <500ms after window readiness, cancel feedback <250ms and local worker stop
p95 <1s. Establish memory budget on target hardware before claiming scalability.

## Implementation log: accounting and scanner baseline, 2026-10-25

Delivered locally in the first development slice:

- Full scans preserve allocated blocks, include directory metadata, expose
  logical totals separately and count each hard-linked file once by device/inode.
  Symlinks are measured as entries without descending into targets.
- Flat inventory, candidate items and Swift inspector retain the measurements;
  duplicate-link entries remain present with an attribution explanation.
- Previous deep-scan caches are rejected through a versioned accounting marker;
  receipt/history storage is unaffected.
- The benchmark harness now requests --deep. Previously it timed the bounded
  overview, which could not establish full-inventory performance. It retains
  per-run scan JSON, coverage/accounting, exact binary hash, platform/build-state
  metadata, elapsed time and child peak RSS. A deterministic fixture generator
  creates small files, wide/deep branches, hard links, symlinks and a sparse file.

The 11,222-entry fixture previously reported 1,114,841,201 allocated bytes.
Corrected output is 40,976,384 allocated bytes and 1,092,170,877 logical bytes,
with ten duplicate hard links. The corrected allocated total agrees with an
independent Python metadata walk and du -sk (40,016 KiB). This is an accounting
correction, not space actually cleaned or a scanner speedup.

The corrected 101,222-entry fixture completed five local release runs with zero
coverage errors: median traversal 681 ms, maximum/five-run nearest-rank p95
705 ms; median complete backend process 723 ms; maximum backend peak RSS
19,824,640 bytes (~18.9 MiB). Cache state was uncontrolled initially and later
runs normally warm. These figures exclude Swift decoding, indexing, UI memory
and rendering. They establish a small synthetic baseline, not a whole-disk,
competitor, cold-cache or million-entry performance claim.

Reproduce on a new fixture directory:

```sh
cargo build --release --offline
python3 scripts/scanner-fixture.py /private/tmp/macclean-scan-fixture --files 100000
scripts/benchmark-scanner.sh --runs 5 --output-dir /private/tmp/macclean-scan-results /private/tmp/macclean-scan-fixture
```

Validation: 93 Rust tests passed; 37 standard Swift tests passed, and all three
rendered Swift tests passed separately (40 Swift tests total). Clippy passed with
warnings denied. The locally signed packaged app passed scan/Trash/restore smoke
checks on disposable fixtures. No user cache cleanup was performed.

Captured summaries, run records and binary/platform metadata are in
`dist/benchmarks/accounting-20261025/`; full scan payloads remain in the benchmark
output directories under /private/tmp. The dist captures are generated artifacts,
not a second plan. The 11K after-correction timing overlapped fixture preparation,
so it must not be used as a controlled before/after speed comparison.

The harness measures the selected binary as built; rebuild explicitly after
source changes. Each output directory must be new. Raw scan JSON contains the
names/paths of the scanned scope. Use disposable fixtures for shareable captures.

The later logs below complete proportional branch bars and remove count-based
candidate truncation. Bulk traversal remains
an independent later engine change. One-million-entry and UI memory/latency
measurements remain outstanding.

## Implementation log: capacity and whole-tree search, 2026-10-25

Delivered locally in the second development slice:

- OS-reported volume total, available and used/unavailable figures are visible
  before scanning, with a thin capacity bar. The scanned scope/bytes are separate;
  nothing infers whole-disk usage from a selected-folder scan. A details popover
  explains APFS sharing, reserved/purgeable space and differences from Finder.
- Capacity reads run off the main actor, refresh with scope/scan/mutation changes
  or explicit refresh, reject superseded results, and clear old figures on error.
  This is a point-in-time read, not continuous monitoring or hidden-space analysis.
- Complete-tree name/path search and kind/allocated-size filters query loaded
  observations without rescanning. Exact match count, largest-first bounded rows,
  explicit expansion, cancellation, empty states and containing-folder navigation
  are connected in the native UI. No arbitrary manual deletion was added.
- Capacity details and search filters use compact controls to preserve room for
  the actual folder branches at 860x600. Rendered checks cover compact/wide
  exploration, search, review and History.

Validation: 45 standard Swift tests passed and four rendered tests passed
separately (49 total). A synthetic 100K-file query verifies full counts and a
100-row response; the debug run observed ~101 ms fixture construction/indexing
and ~78 ms broad matching. These are diagnostic single-run values, not release
performance targets or device-independent guarantees. The rebuilt locally signed
app passed the packaged scan/Trash/restore smoke check on disposable fixtures.
The prior Rust accounting baseline remains unchanged.

The following slice completes proportional branch encoding and removes silent
cleanup-candidate truncation. Remaining exploration gates include mount/coverage
edge cases and larger inventory/UI memory validation. Policy/manual Trash and owner-tool execution retain their
later roadmap positions.

## Implementation log: proportional branches and complete candidate counts, 2026-10-25

Delivered locally in the third development slice:

- Each circuit branch shows a thin allocation bar and percentage of the current
  measured folder. Byte values remain visible, tiny shares are labeled <0.1%, and
  missing/inconsistent denominators show an unavailable percentage. Accessible
  labels and help identify the denominator and partial/reclaimability limitations.
- Removed both independent 512-candidate limits. A full scan retains all nonzero
  discovered cleanup roots, independent of presentation depth/row limits. Selecting
  a classified cache scope no longer suppresses all but its visible descendants.
- Added a discovery-version marker so stale truncated deep-scan caches cannot
  present themselves as current observations. Receipt/history compatibility stays.
- Native Caches/Opportunities display the complete matched count and expand from
  100 visible rows. Filtering resets the visible batch. Bulk selection explicitly
  includes undisplayed matching rows. The immutable scan catalog is sorted during
  off-main inventory indexing rather than on every pure-scan view render.

Validation: two 640-root Rust fixtures exercise project output and direct cache
scopes with a one-row presentation limit. Round-trip serialization retains all
640 candidates. Swift tests confirm beyond-512 cleanup evidence in navigation and
search, plus percentage behavior for 0/100/tiny/unknown/inconsistent totals. Native
checks exercise real list expansion and compact/wide branch/search/review layouts.
95 Rust tests, 48 standard Swift tests and all five rendered tests passed
(53 Swift tests total). The rebuilt locally signed app passed its packaged
scan/Trash/restore smoke check on disposable fixtures. Clippy passes with
warnings denied.

Limits: all inventory and candidate records remain resident; visible batching
only bounds row construction. Classification still requires the shared-policy
work in slice 2; no new rule types, manual personal-folder deletion or tool-command
execution were introduced. Mount-boundary/coverage and larger-scale memory
validation remain open before claiming the exploration engine production-ready.

## Scope boundaries

Retain recents and optional read-only activity; do not make activity checks startup
work. Defer duplicates/content hashing, app uninstall, growth history, SQLite and
FSEvents unless measured needs justify them. Retire maintenance dashboards, RAM
purge, DNS/reset routines, security/network tools, schedules and menu-bar monitoring
from the lean distribution. No privileged helper, daemon, cloud service or arbitrary
script runner. Docker/simulators/toolchains/models remain inspect/advice only.

Preserve legacy receipts and settings through migrations. Disposable inventory
caches may be invalidated. Existing CLI features are legacy implementation, not
promised lean-release scope; removal requires migration notes and explicit errors,
never silently changing a destructive command's meaning.

## Audit source anchors

- UI/filter/action availability: MacCleanApp/Sources/MacCleanApp/LeanWorkspace.swift
- Navigation and fallback classification: MacCleanApp/Sources/MacCleanApp/LoadedTree.swift
- Circuit hierarchy and proportional branch encoding: MacCleanApp/Sources/MacCleanApp/CircuitNavigator.swift
- Complete candidate discovery/cache contract: src/core/{storage,app_scan}.rs
- Traversal, classifiers and accounting: src/core/storage.rs
- Search orchestration/query index: MacCleanApp/Sources/MacCleanApp/{InventorySearch,LoadedTree}.swift
- OS volume readings and capacity display: MacCleanApp/Sources/MacCleanApp/VolumeCapacity.swift
- Exact cache roots: src/core/cache_catalog.rs
- Identity, mutation and recovery: src/core/safety.rs, src/core/trash.rs, src/cli.rs
- Reclaim: ../reclaim/src/app.js and ../reclaim/src-tauri/src/{scan,fsread,rules,safety,lib}.rs

Implementation logs above record delivered local behavior. Other missing features
remain missing until their acceptance gates are implemented and verified.
