import XCTest
@testable import MacCleanApp

final class DuplicateSelectionPolicyTests: XCTestCase {
    func testAutomaticStrategiesChooseOneDeterministicKeeper() throws {
        let group = duplicateGroup(
            id: "digest",
            files: [
                ("/long/path/old", 10),
                ("/b", 30),
                ("/a", 30)
            ]
        )

        XCTAssertEqual(
            try DuplicateGroupSelection(group: group, strategy: .newest).keeperPath,
            "/a"
        )
        XCTAssertEqual(
            try DuplicateGroupSelection(group: group, strategy: .oldest).keeperPath,
            "/long/path/old"
        )
        XCTAssertEqual(
            try DuplicateGroupSelection(group: group, strategy: .shortestPath).keeperPath,
            "/a"
        )
    }

    func testManualStrategyRequiresAGroupMember() throws {
        let group = duplicateGroup(id: "digest", files: [("/a", 1), ("/b", 2)])

        XCTAssertThrowsError(
            try DuplicateGroupSelection(group: group, strategy: .manual)
        ) { error in
            XCTAssertEqual(
                error as? DuplicateSelectionError,
                .manualKeeperRequired(groupID: "digest")
            )
        }
        XCTAssertThrowsError(
            try DuplicateGroupSelection(
                group: group,
                strategy: .manual,
                manualKeeperPath: "/missing"
            )
        ) { error in
            XCTAssertEqual(
                error as? DuplicateSelectionError,
                .keeperNotInGroup(groupID: "digest", path: "/missing")
            )
        }

        let selection = try DuplicateGroupSelection(
            group: group,
            strategy: .manual,
            manualKeeperPath: "/b"
        )
        XCTAssertEqual(selection.keeperPath, "/b")
    }

    func testKeeperCannotBeSelectedAndSelectingAllAlwaysLeavesIt() throws {
        let group = duplicateGroup(
            id: "digest",
            files: [("/a", 1), ("/b", 2), ("/c", 3)]
        )
        var selection = try DuplicateGroupSelection(group: group, strategy: .newest)

        XCTAssertFalse(selection.toggleDeletion(path: selection.keeperPath))
        selection.selectAllDeletable()

        XCTAssertFalse(selection.selectedDeletionPaths.contains(selection.keeperPath))
        XCTAssertEqual(selection.selectedDeletionPaths.count, group.files.count - 1)
    }

    func testMembershipChangeInvalidatesSelectionAndPayload() throws {
        let original = duplicateGroup(id: "digest", files: [("/a", 1), ("/b", 2)])
        let changed = duplicateGroup(
            id: "digest",
            files: [("/a", 1), ("/b", 2), ("/c", 3)]
        )
        var selection = try DuplicateGroupSelection(group: original, strategy: .oldest)
        selection.selectAllDeletable()

        XCTAssertThrowsError(
            try DuplicateCleanupRequest.make(
                selections: [selection],
                currentGroups: [changed]
            )
        ) { error in
            XCTAssertEqual(
                error as? DuplicateSelectionError,
                .groupMembershipChanged(groupID: "digest")
            )
        }

        XCTAssertTrue(selection.invalidateIfMembershipChanged(to: changed))
        XCTAssertTrue(selection.selectedDeletionPaths.isEmpty)
    }

    func testRequestPayloadIsDeterministicAndOmitsEmptyGroups() throws {
        let alpha = duplicateGroup(
            id: "alpha",
            files: [("/z", 1), ("/a", 2), ("/m", 3)]
        )
        let beta = duplicateGroup(id: "beta", files: [("/d", 1), ("/c", 2)])
        let empty = duplicateGroup(id: "empty", files: [("/only", 1)])

        var alphaSelection = try DuplicateGroupSelection(group: alpha, strategy: .newest)
        XCTAssertTrue(alphaSelection.toggleDeletion(path: "/z"))
        XCTAssertTrue(alphaSelection.toggleDeletion(path: "/a"))

        var betaSelection = try DuplicateGroupSelection(
            group: beta,
            strategy: .manual,
            manualKeeperPath: "/c"
        )
        betaSelection.selectAllDeletable()
        let emptySelection = try DuplicateGroupSelection(group: empty, strategy: .oldest)

        let request = try DuplicateCleanupRequest.make(
            selections: [betaSelection, emptySelection, alphaSelection],
            currentGroups: [empty, alpha, beta]
        )

        XCTAssertEqual(request.schemaVersion, 1)
        XCTAssertEqual(request.groups.map(\.groupID), ["alpha", "beta"])
        XCTAssertEqual(request.groups[0].keeperPath, "/m")
        XCTAssertEqual(request.groups[0].reviewedMemberPaths, ["/a", "/m", "/z"])
        XCTAssertEqual(request.groups[0].deletePaths, ["/a", "/z"])
        XCTAssertEqual(request.groups[1].deletePaths, ["/d"])
    }

    private func duplicateGroup(
        id: String,
        files: [(path: String, modifiedAt: UInt64)]
    ) -> DuplicateGroup {
        let duplicateFiles = files.map {
            DuplicateFile(path: $0.path, sizeBytes: 100, modifiedAt: $0.modifiedAt)
        }
        return DuplicateGroup(
            id: id,
            sizeBytes: 100,
            wastedBytes: UInt64(max(0, duplicateFiles.count - 1) * 100),
            files: duplicateFiles
        )
    }
}
