import AppKit
import SwiftUI
import XCTest
@testable import MacCleanApp

/// Run with MACCLEAN_RENDERED_UI_TESTS=1 in a logged-in macOS session.
/// These checks host the actual views and exercise their native scroll views.
final class RenderedScrollingTests: XCTestCase {
    @MainActor
    func testCleanupScrollsAfterInspectingAndSelectingRows() async throws {
        try requireWindowSession()
        let model = fixtureModel()
        model.selectedTab = .safe
        let (window, host) = await mount(model, size: NSSize(width: 860, height: 600))
        defer { window.close() }
        let scroll = try pageScroll(in: host)
        XCTAssertGreaterThan(scroll.documentView?.bounds.height ?? 0, scroll.contentView.bounds.height + 500)
        try click(in: scroll, x: 260, yFromTop: 275)
        await settle(host)
        XCTAssertEqual(model.selectedItem?.path, model.safeOpportunities()[0].path)
        try click(in: scroll, x: 46, yFromTop: 275)
        await settle(host)
        XCTAssertEqual(model.selectedCleanupPaths.count, 1)
        try await assertWheelScrolls(scroll, host: host)
        try snapshot(host, named: "safe-compact-scrolled")
    }

    @MainActor
    func testPagesHaveOneMainScrollSurfaceAtCompactAndWideSizes() async throws {
        try requireWindowSession()
        for size in [NSSize(width: 860, height: 600), NSSize(width: 1440, height: 900)] {
            for tab in [AppTab.dashboard, .safe, .map, .clean, .developer, .duplicates, .history, .uninstall, .temporary, .versions] {
                let model = fixtureModel(temporary: tab == .temporary)
                model.selectedTab = tab
                let (window, host) = await mount(model, size: size)
                let scroll = try pageScroll(in: host)
                let mainScrolls = descendants(of: host).compactMap { $0 as? NSScrollView }
                    .filter { $0.hasVerticalScroller && $0.bounds.width > 400 }
                XCTAssertEqual(mainScrolls.count, 1, "\(tab.title) must not contain competing vertical scroll views")
                XCTAssertLessThanOrEqual(scroll.documentView?.bounds.width ?? 0, scroll.contentView.bounds.width + 2,
                                         "\(tab.title) must fit the window without horizontal clipping")
                try snapshot(host, named: "\(tab.rawValue)-\(Int(size.width))")
                if (scroll.documentView?.bounds.height ?? 0) > scroll.contentView.bounds.height + 100 {
                    try await assertWheelScrolls(scroll, host: host)
                }
                window.close()
            }
        }
    }

    @MainActor
    func testMapSelectionDoesNotStartAScanAndPageStillScrolls() async throws {
        try requireWindowSession()
        let model = fixtureModel()
        model.selectedTab = .map
        let (window, host) = await mount(model, size: NSSize(width: 860, height: 600))
        defer { window.close() }
        try click(in: pageScroll(in: host), x: 160, yFromTop: 360)
        await settle(host)
        XCTAssertNotNil(model.selectedItem)
        XCTAssertFalse(model.isScanning, "A single click should inspect, without starting an expensive scan")
        try await assertWheelScrolls(pageScroll(in: host), host: host)
    }

    @MainActor
    func testTemporaryBuildsRequireManualSelectionAndScroll() async throws {
        try requireWindowSession()
        let model = fixtureModel(temporary: true)
        model.selectedTab = .temporary
        let (window, host) = await mount(model, size: NSSize(width: 860, height: 600))
        defer { window.close() }
        let items = try XCTUnwrap(model.scan).cleanupCandidates
        model.selectSafeCandidates(from: items)
        XCTAssertTrue(model.selectedCleanupPaths.isEmpty, "Build output must never be bulk-selected as safe cache")
        model.toggleCleanup(items[0])
        XCTAssertEqual(model.selectedCleanupPaths.count, 1)
        await settle(host)
        try await assertWheelScrolls(pageScroll(in: host), host: host)
    }

    @MainActor
    func testVersionsProtectPinnedEntriesAndRemainScrollable() async throws {
        try requireWindowSession()
        let model = fixtureModel()
        model.selectedTab = .versions
        let (window, host) = await mount(model, size: NSSize(width: 860, height: 600))
        defer { window.close() }
        let entries = try XCTUnwrap(model.versionReport).entries
        model.toggleCleanup(entries[1].cleanupItem)
        XCTAssertTrue(model.selectedCleanupPaths.isEmpty)
        model.selectSafeCandidates(from: entries.map(\.cleanupItem))
        XCTAssertTrue(model.selectedCleanupPaths.isEmpty, "No versions should be bulk selected")
        model.toggleCleanup(entries[0].cleanupItem)
        XCTAssertEqual(model.selectedCleanupPaths.count, 1)
        await settle(host)
        try await assertWheelScrolls(pageScroll(in: host), host: host)
    }

    private func requireWindowSession() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["MACCLEAN_RENDERED_UI_TESTS"] == "1",
                          "Opt in with scripts/test-ui.sh in a logged-in macOS session")
    }

    @MainActor
    func testLongUninstallReviewScrollsInLightAppearance() async throws {
        try requireWindowSession()
        let model = fixtureModel()
        model.selectedInstalledApplication = model.installedApplications[0]
        model.uninstallPlan = UninstallPlan(
            appName: "Example App", bundleId: "dev.fixture.app", bundleIdGuessed: false,
            appPath: "/tmp/macclean-ui/App.app",
            items: (0..<50).map {
                UninstallTraceItem(path: "/tmp/macclean-ui/Library/Application Support/Example App/Trace-\($0)",
                                   sizeBytes: 10_000_000, reason: "Application-owned data requiring review.", risk: "medium")
            }, totalSize: 500_000_000, deep: true, runningProcesses: [], canExecute: true, preflightError: nil)
        // Supply the light sheet background when capturing the content view alone.
        let content = WorkspacePage { UninstallInspector(model: model, deepReview: .constant(true)) }
            .background(Color.white)
            .environment(\.colorScheme, .light)
        let (window, host) = await mountView(content, size: NSSize(width: 680, height: 488))
        defer { window.close() }
        host.appearance = NSAppearance(named: .aqua)
        await settle(host)
        let scroll = try pageScroll(in: host)
        XCTAssertGreaterThan(scroll.documentView?.bounds.height ?? 0, scroll.contentView.bounds.height + 500)
        XCTAssertLessThanOrEqual(scroll.documentView?.bounds.width ?? 0, scroll.contentView.bounds.width + 2)
        try await assertWheelScrolls(scroll, host: host)
        try snapshot(host, named: "uninstall-review-light-scrolled")
    }

    @MainActor
    private func mount(_ model: AppModel, size: NSSize) async -> (NSWindow, NSHostingView<RootView>) {
        await mountView(RootView(model: model, loadsStartupData: false), size: size)
    }

    @MainActor
    private func mountView<Content: View>(_ content: Content, size: NSSize) async -> (NSWindow, NSHostingView<Content>) {
        _ = NSApplication.shared
        let host = NSHostingView(rootView: content)
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size),
                              styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = host
        window.setContentSize(size)
        window.setFrameOrigin(NSPoint(x: -10000, y: -10000))
        window.orderFront(nil)
        await settle(host)
        return (window, host)
    }

    @MainActor
    private func settle(_ view: NSView) async {
        for _ in 0..<3 {
            view.layoutSubtreeIfNeeded()
            try? await Task.sleep(nanoseconds: 40_000_000)
        }
    }

    @MainActor
    private func pageScroll(in view: NSView) throws -> NSScrollView {
        try XCTUnwrap(descendants(of: view).compactMap { $0 as? NSScrollView }
            .filter { $0.hasVerticalScroller && $0.bounds.width > 400 }
            .max { $0.bounds.width < $1.bounds.width }, "The page needs a native vertical scroll view")
    }

    @MainActor
    private func assertWheelScrolls(_ scroll: NSScrollView, host: NSView) async throws {
        let before = scroll.contentView.bounds.origin.y
        let event = try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1,
                                         wheel1: -180, wheel2: 0, wheel3: 0))
        scroll.scrollWheel(with: try XCTUnwrap(NSEvent(cgEvent: event)))
        await settle(host)
        XCTAssertGreaterThan(scroll.contentView.bounds.origin.y, before, "Wheel scrolling must advance the page")
    }

    @MainActor
    private func descendants(of view: NSView) -> [NSView] {
        [view] + view.subviews.flatMap { descendants(of: $0) }
    }

    @MainActor
    private func click(in scroll: NSScrollView, x: CGFloat, yFromTop: CGFloat) throws {
        let document = try XCTUnwrap(scroll.documentView)
        let window = try XCTUnwrap(scroll.window)
        let y = document.isFlipped ? yFromTop : document.bounds.height - yFromTop
        let point = document.convert(NSPoint(x: x, y: y), to: nil)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let event = try XCTUnwrap(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [],
                                timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                context: nil, eventNumber: 1, clickCount: 1, pressure: 1))
            window.sendEvent(event)
        }
    }

    @MainActor
    private func snapshot(_ view: NSView, named name: String) throws {
        guard let directory = ProcessInfo.processInfo.environment["MACCLEAN_UI_SNAPSHOTS"] else { return }
        let root = URL(fileURLWithPath: directory)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let bitmap = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: bitmap)
        let data = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
        try data.write(to: root.appendingPathComponent(name + ".png"))
    }

    @MainActor
    private func fixtureModel(temporary: Bool = false) -> AppModel {
        let model = AppModel()
        let items = (0..<60).map { index in
            AppScanItem(name: "Application Cache \(index + 1)", path: temporary ? "/private/tmp/fixture-\(index + 1)-target" : "/tmp/macclean-ui/Library/Caches/Application-\(index + 1)",
                        sizeBytes: UInt64(60 - index) * 100_000_000, percentOfRoot: 2, isDir: true, partial: false,
                        safety: temporary ? .review : .safe, cleanKind: temporary ? "dev artifact" : "cache", cleanupAction: "Move to Trash",
                        cleanupReason: "Regenerable application cache. The application recreates it when needed.")
        }
        let tree = StorageNode(name: "Fixture", path: temporary ? "/private/tmp" : "/tmp/macclean-ui", sizeBytes: 8_000_000_000,
                               percentOfParent: 100, isDir: true, partial: false, safety: .unknown, cleanKind: "unknown",
                               cleanupAction: "Inspect", cleanupReason: "Test fixture", children: [])
        model.path = tree.path
        model.scan = AppScan(schemaVersion: 1, rootScan: StorageScan(root: tree.path, mode: "deep", depth: 4, limit: 80,
                            scannedAt: 0, elapsedMs: 10, partial: false, incompleteReason: nil, metrics: nil, tree: tree),
                            systemData: nil, largestItems: items, cleanupCandidates: items,
                            safetyTotals: [SafetyTotal(safety: .safe, sizeBytes: tree.sizeBytes, percentOfRoot: 100, itemCount: items.count)], health: nil)
        model.recipes = (0..<6).map { index in
            CleanupRecipe(id: "recipe-\(index)", title: "Cache recipe \(index + 1)",
                          subtitle: "Regenerable application and developer cache data. Review the paths before removal.",
                          safety: .safe, totalBytes: 500_000_000, itemCount: 1, selectedByDefault: false, command: "cache",
                          items: [RecipeItem(label: items[index].name, path: items[index].path, sizeBytes: items[index].sizeBytes,
                                             kind: "cache", risk: "low", reason: items[index].cleanupReason,
                                             removable: true, safety: .safe, appEligible: true)])
        }
        model.isLoadingHistory = true // Keep fixture rendering independent of backend processes.
        model.history = (0..<40).map { index in
            HistorySession(sessionId: "fixture-session-\(index)", timestamp: 0, cleaner: "Safe Cache Cleanup",
                           itemCount: 5, totalBytes: 500_000_000, method: "trash", restorableCount: 5)
        }
        model.installedApplications = (0..<40).map { index in
            InstalledApplication(name: "Example App \(index)", path: "/tmp/macclean-ui/App-\(index).app",
                                 bundleId: "dev.fixture.app\(index)", protected: false)
        }
        let groups = (0..<12).map { index in
            DuplicateGroup(id: "group-\(index)", sizeBytes: 20_000_000, wastedBytes: 20_000_000,
                           files: [DuplicateFile(path: "/tmp/macclean-ui/duplicates/\(index)/original.dat", sizeBytes: 20_000_000, modifiedAt: 1),
                                   DuplicateFile(path: "/tmp/macclean-ui/duplicates/\(index)/copy.dat", sizeBytes: 20_000_000, modifiedAt: 2)])
        }
        model.duplicateReport = DuplicateReport(schemaVersion: 1, root: tree.path, minBytes: 1, scannedFiles: 24,
                            hashedFiles: 24, partial: false, errorCount: 0, totalWastedBytes: 240_000_000, elapsedMs: 10, groups: groups)
        var versions: [InstalledVersionEntry] = []
        for index in 0..<40 {
            let removable = index % 2 == 0
            let reason = removable ? "No project pins matched in the checked roots." : "Required by /tmp/macclean-ui/project/rust-toolchain.toml"
            let entry = InstalledVersionEntry(path: "/tmp/macclean-ui/.rustup/toolchains/fixture-\(index)",
                                              name: "Rust nightly fixture \(index)", family: "Rust", version: "nightly-2025-01-\(index)",
                                              sizeBytes: UInt64(40 - index) * UInt64(10_000_000), removable: removable,
                                              reasons: [reason])
            versions.append(entry)
        }
        model.versionReport = InstalledVersionReport(entries: versions, projectRoots: ["/tmp/macclean-ui"],
                                                     checkedFiles: 12, complete: true, warnings: [])
        model.fullDiskAccessStatus = .granted
        return model
    }
}
