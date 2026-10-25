import XCTest
@testable import MacCleanApp

final class RecentFoldersTests: XCTestCase {
    func testRecordingNormalizesDeduplicatesAndBoundsRecentScopes() {
        var folders: [RecentFolder] = []
        for index in 0..<12 { folders = RecentFolder.recording("/project/\(index)", in: folders) }
        folders = RecentFolder.recording("/project/11/../10", in: folders, at: Date(timeIntervalSince1970: 100))
        XCTAssertEqual(folders.count, 8)
        XCTAssertEqual(folders.first?.path, "/project/10")
        XCTAssertEqual(folders.filter { $0.path == "/project/10" }.count, 1)
        XCTAssertEqual(folders.first?.scannedAt, Date(timeIntervalSince1970: 100))
        XCTAssertEqual(RecentFolder.recording("relative", in: folders), folders)
    }
    func testRecentsPersistAndClearWithoutTouchingScanOrReceiptState() {
        let suite = "macclean-recents-test-\(UUID())"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        defaults.set("preserve", forKey: "receipt")
        let folders = RecentFolder.recording("/project", in: [])
        RecentFolder.save(folders, defaults: defaults)
        XCTAssertEqual(RecentFolder.load(defaults: defaults), folders)
        RecentFolder.save([], defaults: defaults)
        XCTAssertEqual(RecentFolder.load(defaults: defaults), [])
        XCTAssertEqual(defaults.string(forKey: "receipt"), "preserve")
    }
    @MainActor
    func testSharedTrayRetainsMetadataAndRemovesOverlappingSelections() {
        let model = AppModel()
        let parent = item("/project/cache")
        let child = item("/project/cache/child")
        model.toggleCleanup(parent)
        model.toggleCleanup(child)
        XCTAssertEqual(model.collection.map(\.path), [child.path])
        XCTAssertEqual(model.collectionBytes, child.sizeBytes)
        model.clearCleanupSelection()
        XCTAssertTrue(model.collection.isEmpty)
        XCTAssertTrue(model.collectedItems.isEmpty)
    }
    @MainActor
    func testBulkCacheAdditionPreservesTrayAndSkipsReviewOnlyItems() {
        let model = AppModel()
        model.toggleCleanup(item("/existing/cache"))
        let review = AppScanItem(name: "sources", path: "/cargo/sources", sizeBytes: 20, percentOfRoot: 0, isDir: true, partial: false, safety: .review, cleanKind: "cache", cleanupAction: "Move to Trash", cleanupReason: "Local edits may exist")
        model.addSafeCaches(from: [item("/new/cache"), review])
        XCTAssertEqual(model.selectedCleanupPaths, ["/existing/cache", "/new/cache"])
        XCTAssertEqual(model.collectionBytes, 20)
    }

    private func item(_ path: String) -> AppScanItem {
        AppScanItem(name: path, path: path, sizeBytes: 10, percentOfRoot: 0, isDir: true, partial: false, safety: .safe, cleanKind: "cache", cleanupAction: "Move to Trash", cleanupReason: "fixture")
    }
}
