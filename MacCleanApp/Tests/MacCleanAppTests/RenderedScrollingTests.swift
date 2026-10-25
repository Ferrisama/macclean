import AppKit
import SwiftUI
import XCTest
@testable import MacCleanApp

/// Run with MACCLEAN_RENDERED_UI_TESTS=1 in a logged-in macOS session.
/// These checks host the actual views and exercise their native scroll views.
final class RenderedScrollingTests: XCTestCase {
    @MainActor
    func testLeanPagesFitAndScrollAtCompactAndWideSizes() async throws {
        try requireWindowSession()
        for size in [NSSize(width: 860, height: 600), NSSize(width: 1440, height: 900)] {
            for tab in AppTab.allCases {
                let model = fixtureModel()
                model.selectedTab = tab
                let (window, host) = await mount(model, size: size)
                defer { window.close() }
                let scroll = try pageScroll(in: host)
                XCTAssertLessThanOrEqual(scroll.documentView?.bounds.width ?? 0, scroll.contentView.bounds.width + 2)
                try snapshot(host, named: "lean-\(tab.rawValue)-\(Int(size.width))-top")
                if (scroll.documentView?.bounds.height ?? 0) > scroll.contentView.bounds.height + 100 {
                    try await assertWheelScrolls(scroll, host: host)
                }
                try snapshot(host, named: "lean-\(tab.rawValue)-\(Int(size.width))")
            }
        }
    }

    @MainActor
    func testCircuitSelectionDoesNotStartAScanAndScrolls() async throws {
        try requireWindowSession()
        let model = fixtureModel()
        let (window, host) = await mount(model, size: NSSize(width: 860, height: 600))
        defer { window.close() }
        let scroll = try pageScroll(in: host)
        model.selectedItem = nil
        await settle(host)
        try click(in: scroll, x: 160, yFromTop: 390)
        await settle(host)
        XCTAssertNotNil(model.selectedItem)
        XCTAssertFalse(model.isScanning)
        try await assertWheelScrolls(scroll, host: host)
    }

    @MainActor
    func testWholeTreeSearchFitsCompactAndWideWindows() async throws {
        try requireWindowSession()
        for size in [NSSize(width: 860, height: 600), NSSize(width: 1440, height: 900)] {
            let model = fixtureModel()
            model.inventorySearchQuery = "Application"
            let (window, host) = await mount(model, size: size)
            defer { window.close() }
            await model.searchInventory(debounce: false)
            await settle(host)
            XCTAssertEqual(model.inventorySearchPage?.totalCount, 60)
            XCTAssertFalse(model.isScanning)
            let scroll = try pageScroll(in: host)
            XCTAssertLessThanOrEqual(scroll.documentView?.bounds.width ?? 0, scroll.contentView.bounds.width + 2)
            try snapshot(host, named: "lean-search-\(Int(size.width))-top")
            try await assertWheelScrolls(scroll, host: host)
        }
    }

    @MainActor
    func testLargeOpportunityCatalogExpandsPastInitialVisibleBatch() async throws {
        try requireWindowSession()
        let model = fixtureModel(itemCount: 640)
        model.recipes = []
        await model.installLoadedScan(try XCTUnwrap(model.scan))
        XCTAssertEqual(model.opportunities.count, 640)
        let (window, host) = await mountView(
            ExploreWorkspace(model: model, filter: .constant(.opportunities))
                .background(AeroTheme.background).tint(AeroTheme.accent).preferredColorScheme(.dark),
            size: NSSize(width: 860, height: 600))
        defer { window.close() }
        let scroll = try pageScroll(in: host)
        try snapshot(host, named: "lean-opportunities-640-top")
        for _ in 0..<2 {
            let height = try XCTUnwrap(scroll.documentView).bounds.height
            scroll.contentView.scroll(to: NSPoint(x: 0, y: max(0, height - scroll.contentView.bounds.height)))
            scroll.reflectScrolledClipView(scroll.contentView)
            await settle(host)
        }
        let height = try XCTUnwrap(scroll.documentView).bounds.height
        try snapshot(host, named: "lean-opportunities-640-first-batch")
        try click(in: scroll, x: 100, yFromTop: height - 31)
        await settle(host)
        XCTAssertGreaterThan(try XCTUnwrap(scroll.documentView).bounds.height, height + 2000,
                             "Show next must expose more loaded candidates, not discard the remainder")
        XCTAssertEqual(model.opportunities.count, 640)
        XCTAssertFalse(model.isScanning)
    }

    @MainActor
    func testReviewSheetFitsAndScrollsLongSelection() async throws {
        try requireWindowSession()
        let model = fixtureModel()
        for item in try XCTUnwrap(model.scan).cleanupCandidates { model.toggleCleanup(item) }
        let (window, host) = await mountView(CleanupReviewSheet(model: model), size: NSSize(width: 620, height: 530))
        defer { window.close() }
        let scroll = try pageScroll(in: host)
        XCTAssertGreaterThan(scroll.documentView?.bounds.height ?? 0, scroll.contentView.bounds.height + 500)
        try await assertWheelScrolls(scroll, host: host)
        try snapshot(host, named: "lean-review-scrolled")
    }

    private func requireWindowSession() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["MACCLEAN_RENDERED_UI_TESTS"] == "1",
                          "Opt in with scripts/test-ui.sh in a logged-in macOS session")
    }

    @MainActor
    private func mount(_ model: AppModel, size: NSSize) async -> (NSWindow, NSHostingView<RootView>) {
        if let scan = model.scan { await model.installLoadedScan(scan) }
        return await mountView(RootView(model: model, loadsStartupData: false), size: size)
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
    private func fixtureModel(temporary: Bool = false, itemCount: Int = 60) -> AppModel {
        let model = AppModel(capacityReader: { _ in
            try VolumeCapacity(name: "Fixture volume", volumePath: "/fixture-volume",
                               totalBytes: 500_000_000_000, availableBytes: 200_000_000_000)
        })
        let items = (0..<itemCount).map { index in
            AppScanItem(name: "Application Cache \(index + 1)", path: temporary ? "/private/tmp/fixture-\(index + 1)-target" : "/tmp/macclean-ui/Library/Caches/Application-\(index + 1)",
                        sizeBytes: UInt64(max(1, 60 - index)) * 100_000_000, percentOfRoot: 2, isDir: true, partial: false,
                        safety: temporary ? .review : .safe, cleanKind: temporary ? "dev artifact" : "cache", cleanupAction: "Move to Trash",
                        cleanupReason: "Regenerable application cache. The application recreates it when needed.")
        }
        let tree = StorageNode(name: "Fixture", path: temporary ? "/private/tmp" : "/tmp/macclean-ui", sizeBytes: items.reduce(0) { $0 + $1.sizeBytes },
                               percentOfParent: 100, isDir: true, partial: false, safety: .unknown, cleanKind: "unknown",
                               cleanupAction: "Inspect", cleanupReason: "Test fixture", children: [])
        model.path = tree.path
        var storage = StorageScan(root: tree.path, mode: "deep", depth: 4, limit: 80,
                                  scannedAt: UInt64(Date().timeIntervalSince1970), elapsedMs: 10, partial: false,
                                  incompleteReason: nil, metrics: nil, tree: tree)
        storage.inventory = ScanInventory(root: tree.path, nodes: [
            InventoryNode(name: "Fixture", parent: nil, sizeBytes: tree.sizeBytes, isDir: true, partial: false)
        ] + items.map { InventoryNode(name: URL(fileURLWithPath: $0.path).lastPathComponent, parent: 0, sizeBytes: $0.sizeBytes, isDir: true, partial: false) })
        // Keep fixture children directly under the root, matching inventory parentage.
        let fixtureItems = items.map { item in
            AppScanItem(name: item.name, path: tree.path + "/" + URL(fileURLWithPath: item.path).lastPathComponent,
                        sizeBytes: item.sizeBytes, percentOfRoot: item.percentOfRoot, isDir: true, partial: false,
                        safety: item.safety, cleanKind: item.cleanKind, cleanupAction: item.cleanupAction, cleanupReason: item.cleanupReason)
        }
        model.scan = AppScan(schemaVersion: 1, rootScan: storage, systemData: nil, largestItems: fixtureItems,
                            cleanupCandidates: fixtureItems,
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
        model.fullDiskAccessStatus = .granted
        return model
    }
}
