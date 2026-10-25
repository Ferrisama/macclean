import XCTest
@testable import MacCleanApp

final class LoadedTreeTests: XCTestCase {
    func testCompleteInventoryContainsUnlistedBranchesAndEmptyFiles() throws {
        let tree = try XCTUnwrap(LoadedTree(inventory: inventory(), candidates: []))
        XCTAssertEqual(tree.folderCount, 4)
        XCTAssertEqual(tree.fileCount, 2)
        XCTAssertEqual(tree.children(of: "/fixture")?.map(\.name), ["project", "empty-folder"])
        XCTAssertEqual(tree.children(of: "/fixture/project/nested")?.map(\.name), ["data", "empty-file"])
        XCTAssertEqual(tree.children(of: "/fixture/empty-folder")?.count, 0)
        XCTAssertNil(tree.children(of: "/outside"))
        XCTAssertFalse(tree.children(of: "/fixture/project/nested")!.contains { $0.canMoveToTrash })
    }

    func testMalformedParentsAndUnsafeNamesRejectInventory() {
        let root = InventoryNode(name: "fixture", parent: nil, sizeBytes: 1, isDir: true, partial: false)
        for node in [InventoryNode(name: "bad", parent: 1, sizeBytes: 1, isDir: true, partial: false),
                     InventoryNode(name: "../escape", parent: 0, sizeBytes: 1, isDir: true, partial: false)] {
            XCTAssertNil(LoadedTree(inventory: ScanInventory(root: "/fixture", nodes: [root, node]), candidates: []))
        }
    }

    @MainActor
    func testDeepNavigationAndBackUseOnlyLoadedObservations() async throws {
        let model = AppModel()
        await model.installLoadedScan(scan())
        let project = try XCTUnwrap(model.mapItems(in: XCTUnwrap(model.scan)).first)
        model.open(project)
        XCTAssertEqual(model.path, "/fixture/project")
        XCTAssertFalse(model.isScanning)
        let nested = try XCTUnwrap(model.mapItems(in: XCTUnwrap(model.scan)).first)
        model.open(nested)
        XCTAssertEqual(model.path, "/fixture/project/nested")
        XCTAssertEqual(model.mapItems(in: try XCTUnwrap(model.scan)).count, 2)
        XCTAssertFalse(model.isScanning, "Navigation must work even though /fixture does not exist on disk")
        model.goBack()
        XCTAssertEqual(model.path, "/fixture/project")
        model.goForward()
        XCTAssertEqual(model.path, "/fixture/project/nested")
        model.goToScanRoot()
        model.goUp()
        XCTAssertEqual(model.path, "/fixture")
        XCTAssertFalse(model.isScanning)
    }

    func testLoadedMeasurementsPreserveSparseAndHardLinkAttribution() throws {
        let observations = ScanInventory(root: "/fixture", nodes: [
            InventoryNode(name: "fixture", parent: nil, sizeBytes: 4096, isDir: true, partial: false,
                          logicalSizeBytes: 1_073_741_824, hardLinkDuplicate: false),
            InventoryNode(name: "sparse", parent: 0, sizeBytes: 0, isDir: false, partial: false,
                          logicalSizeBytes: 1_073_741_824, hardLinkDuplicate: false),
            InventoryNode(name: "alias", parent: 0, sizeBytes: 0, isDir: false, partial: false,
                          logicalSizeBytes: 0, hardLinkDuplicate: true)
        ])
        let tree = try XCTUnwrap(LoadedTree(inventory: observations, candidates: []))
        let children = try XCTUnwrap(tree.children(of: "/fixture"))
        let sparse = try XCTUnwrap(children.first { $0.name == "sparse" })
        XCTAssertEqual(sparse.sizeBytes, 0)
        XCTAssertEqual(sparse.logicalSizeBytes, 1_073_741_824)
        XCTAssertEqual(children.first { $0.name == "alias" }?.hardLinkDuplicate, true)
        XCTAssertEqual(tree.folder("/fixture")?.logicalSizeBytes, 1_073_741_824)
    }

    func testSearchFindsDeepNamesAndPathsRegardlessOfCurrentFolder() throws {
        let tree = try XCTUnwrap(LoadedTree(inventory: inventory(), candidates: []))
        XCTAssertEqual(try tree.search(query: "DATA").items.map(\.path), ["/fixture/project/nested/data"])
        XCTAssertEqual(try tree.search(query: "project/nested").totalCount, 3)
        let empty = try tree.search(query: "empty", kind: .files)
        XCTAssertEqual(empty.items.map(\.name), ["empty-file"])
        XCTAssertFalse(empty.items[0].canMoveToTrash)
        XCTAssertEqual(try tree.search(query: "", kind: .files, minimumBytes: 100).items.map(\.name), ["data"])
    }

    func testSearchReportsFullCountWhileReturningBoundedLargestResults() throws {
        let tree = try XCTUnwrap(LoadedTree(inventory: inventory(), candidates: []))
        let first = try tree.search(query: "", limit: 2)
        let expanded = try tree.search(query: "", limit: 4)
        XCTAssertEqual(first.totalCount, 5)
        XCTAssertEqual(first.items.count, 2)
        XCTAssertEqual(expanded.totalCount, first.totalCount)
        XCTAssertEqual(Array(expanded.items.prefix(2)).map(\.path), first.items.map(\.path))
        XCTAssertEqual(try tree.search(query: "not-present").totalCount, 0)
    }

    @MainActor
    func testCancelledInventorySearchDoesNotReturnResults() async throws {
        let tree = try XCTUnwrap(LoadedTree(inventory: inventory(), candidates: []))
        let task = Task { @MainActor in try tree.search(query: "") }
        task.cancel()
        do {
            _ = try await task.value
            XCTFail("A cancelled query must not return a page")
        } catch is CancellationError {}
    }

    @MainActor
    func testNewSearchSupersedesOldQueryAndContainingFolderNeedsNoScan() async throws {
        let model = AppModel()
        await model.installLoadedScan(scan())
        model.inventorySearchQuery = "empty"
        let earlier = Task { await model.searchInventory() }
        await Task.yield()
        model.inventorySearchQuery = "data"
        await model.searchInventory(debounce: false)
        await earlier.value
        let result = try XCTUnwrap(model.inventorySearchPage?.items.first)
        XCTAssertEqual(result.path, "/fixture/project/nested/data")
        XCTAssertFalse(model.isSearchingInventory)
        model.openSearchResult(result, containingFolder: true)
        XCTAssertEqual(model.path, "/fixture/project/nested")
        XCTAssertEqual(model.selectedItem?.path, result.path)
        XCTAssertFalse(model.isInventorySearchActive)
        XCTAssertFalse(model.isScanning, "The fixture does not exist; navigation must use observations only")
    }

    func testWholeTreeSearchAcrossOneHundredThousandEntriesIsBounded() throws {
        let start = DispatchTime.now().uptimeNanoseconds
        let nodes = [InventoryNode(name: "fixture", parent: nil, sizeBytes: 100_000, isDir: true, partial: false)]
            + (0..<100_000).map { InventoryNode(name: "file-\($0).bin", parent: 0, sizeBytes: UInt64($0), isDir: false, partial: false) }
        let tree = try XCTUnwrap(LoadedTree(inventory: ScanInventory(root: "/nonexistent-search-fixture", nodes: nodes), candidates: []))
        let indexed = DispatchTime.now().uptimeNanoseconds
        let page = try tree.search(query: "file-", kind: .files, limit: 100)
        let searched = DispatchTime.now().uptimeNanoseconds
        XCTAssertEqual(page.totalCount, 100_000)
        XCTAssertEqual(page.items.count, 100)
        XCTAssertEqual(page.items.first?.name, "file-99999.bin")
        XCTAssertEqual(try tree.search(query: "file-99999.bin").totalCount, 1)
        print("Inventory fixture: 100K files; construction/index \(Double(indexed - start) / 1_000_000) ms; broad search \(Double(searched - indexed) / 1_000_000) ms (debug build, not a release benchmark)")
    }

    func testCandidateEvidenceBeyond512SurvivesInventoryIndexingAndSearch() throws {
        let candidates = (0..<640).map { index in
            AppScanItem(name: "cache-\(index)", path: "/fixture/cache-\(index)", sizeBytes: UInt64(index + 1),
                        percentOfRoot: 0, isDir: true, partial: false, safety: .safe, cleanKind: "cache",
                        cleanupAction: "Move to Trash", cleanupReason: "Fixture evidence")
        }
        let nodes = [InventoryNode(name: "fixture", parent: nil, sizeBytes: 205_120, isDir: true, partial: false)]
            + candidates.map { InventoryNode(name: $0.name, parent: 0, sizeBytes: $0.sizeBytes, isDir: true, partial: false) }
        let tree = try XCTUnwrap(LoadedTree(inventory: ScanInventory(root: "/fixture", nodes: nodes), candidates: candidates))
        XCTAssertEqual(tree.cleanupItems.count, 640)
        XCTAssertEqual(tree.cleanupItems.first?.name, "cache-639")
        XCTAssertTrue(try XCTUnwrap(tree.folder("/fixture/cache-639")).canMoveToTrash)
        let search = try tree.search(query: "cache-639")
        XCTAssertEqual(search.totalCount, 1)
        XCTAssertTrue(try XCTUnwrap(search.items.first).canMoveToTrash)
        XCTAssertEqual(Set(tree.cleanupItems.map(\.path)).count, 640)
    }

    private func inventory() -> ScanInventory {
        ScanInventory(root: "/fixture", nodes: [
            InventoryNode(name: "fixture", parent: nil, sizeBytes: 100, isDir: true, partial: false),
            InventoryNode(name: "project", parent: 0, sizeBytes: 100, isDir: true, partial: false),
            InventoryNode(name: "nested", parent: 1, sizeBytes: 100, isDir: true, partial: false),
            InventoryNode(name: "data", parent: 2, sizeBytes: 100, isDir: false, partial: false),
            InventoryNode(name: "empty-file", parent: 2, sizeBytes: 0, isDir: false, partial: false),
            InventoryNode(name: "empty-folder", parent: 0, sizeBytes: 0, isDir: true, partial: false)
        ])
    }

    private func scan() -> AppScan {
        let root = StorageNode(name: "fixture", path: "/fixture", sizeBytes: 100, percentOfParent: 100,
                               isDir: true, partial: false, safety: .unknown, cleanKind: "unknown",
                               cleanupAction: "Inspect", cleanupReason: "fixture", children: [])
        var storage = StorageScan(root: root.path, mode: "deep", depth: 1, limit: 1, scannedAt: 0,
                                  elapsedMs: 1, partial: false, incompleteReason: nil, metrics: nil, tree: root)
        storage.inventory = inventory()
        return AppScan(schemaVersion: 1, rootScan: storage, systemData: nil, largestItems: [],
                       cleanupCandidates: [], safetyTotals: [], health: nil)
    }
}
