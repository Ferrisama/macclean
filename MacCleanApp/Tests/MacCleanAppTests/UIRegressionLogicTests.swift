import XCTest
@testable import MacCleanApp

final class UIRegressionLogicTests: XCTestCase {
    @MainActor
    func testDashboardRecipeSelectionOpensCleanupReviewAndClearResetsIt() {
        let model = AppModel()
        let recipe = CleanupRecipe(
            id: "cache",
            title: "Cache",
            subtitle: "",
            safety: .safe,
            totalBytes: 10,
            itemCount: 1,
            selectedByDefault: false,
            command: "cache",
            items: [recipeItem(path: "/eligible", removable: true, appEligible: true)]
        )

        model.selectRecipePaths(recipe)

        XCTAssertEqual(model.selectedTab, .clean)
        XCTAssertEqual(model.selectedCleanupPaths, ["/eligible"])
        XCTAssertEqual(model.selectedRecipe?.id, "cache")

        model.clearCleanupSelection()

        XCTAssertTrue(model.selectedCleanupPaths.isEmpty)
        XCTAssertNil(model.selectedRecipe)
    }

    func testNewScanRejectsResultsFromPreviousGeneration() {
        var ownership = ScanGenerationOwnership()
        let first = ownership.begin(jobID: UUID())
        let second = ownership.begin(jobID: UUID())

        XCTAssertFalse(ownership.accepts(first))
        XCTAssertTrue(ownership.accepts(second))
    }

    func testCancellationInvalidatesActiveScan() {
        var ownership = ScanGenerationOwnership()
        let ticket = ownership.begin(jobID: UUID())

        ownership.cancel()

        XCTAssertFalse(ownership.accepts(ticket))
        XCTAssertNil(ownership.activeJobID)
    }

    func testOnlyTheCurrentScanCanCompleteOwnership() {
        var ownership = ScanGenerationOwnership()
        let stale = ownership.begin(jobID: UUID())
        let current = ownership.begin(jobID: UUID())

        XCTAssertFalse(ownership.complete(stale))
        XCTAssertTrue(ownership.complete(current))
        XCTAssertNil(ownership.activeJobID)
    }

    func testUnselectableCleanupItemCannotBeToggled() {
        let original: Set<String> = ["/eligible"]
        let unchanged = CleanupSelectionRules.toggling(
            path: "/protected",
            isSelectable: false,
            in: original
        )
        XCTAssertEqual(unchanged, original)

        let selected = CleanupSelectionRules.toggling(
            path: "/another-eligible",
            isSelectable: true,
            in: original
        )
        XCTAssertEqual(selected, ["/eligible", "/another-eligible"])
    }

    func testBulkSafeSelectionExcludesUnavailableItems() {
        let items = [
            scanItem(path: "/safe", safety: .safe, action: "Move to Trash"),
            scanItem(path: "/safe-cli-only", safety: .safe, action: "Unavailable in app"),
            scanItem(path: "/review", safety: .review, action: "Move to Trash")
        ]

        XCTAssertEqual(CleanupSelectionRules.safeSelectablePaths(in: items), ["/safe"])
    }

    func testRecipeSelectionOnlyIncludesAppEligibleRemovablePaths() {
        let recipe = CleanupRecipe(
            id: "cache",
            title: "Cache",
            subtitle: "",
            safety: .safe,
            totalBytes: 30,
            itemCount: 4,
            selectedByDefault: false,
            command: "cache",
            items: [
                recipeItem(path: "/eligible", removable: true, appEligible: true),
                recipeItem(path: "/not-removable", removable: false, appEligible: true),
                recipeItem(path: "/cli-only", removable: true, appEligible: false),
                recipeItem(path: "/dev/null", removable: true, appEligible: true)
            ]
        )

        XCTAssertEqual(CleanupSelectionRules.selectableRecipePaths(in: recipe), ["/eligible"])
    }

    func testPreflightRetainsOnlySuccessfulSelectedPaths() {
        let selected: Set<String> = ["/ok", "/rejected", "/not-returned"]
        let outcomes = [
            AppTrashOutcome(path: "/ok", moved: false, trashPath: nil, error: nil),
            AppTrashOutcome(path: "/rejected", moved: false, trashPath: nil, error: "protected")
        ]

        XCTAssertEqual(
            CleanupSelectionRules.retainingEligible(selected, outcomes: outcomes),
            ["/ok"]
        )
    }

    func testOpeningDirectoryScansItWhileOpeningFileOnlySelectsIt() {
        XCTAssertEqual(
            NavigationRules.opening(path: "/Users/test/Documents", isDirectory: true),
            .scanDirectory(path: "/Users/test/Documents")
        )
        XCTAssertEqual(
            NavigationRules.opening(path: "/Users/test/file.zip", isDirectory: false),
            .selectFile(path: "/Users/test/file.zip")
        )
    }

    func testParentNavigationStopsAtFilesystemRoot() {
        XCTAssertEqual(
            NavigationRules.parent(of: "/Users/test/Documents"),
            .scanDirectory(path: "/Users/test")
        )
        XCTAssertEqual(NavigationRules.parent(of: "/"), .stay)
        XCTAssertEqual(NavigationRules.navigating(to: ""), .stay)
    }

    private func scanItem(
        path: String,
        safety: StorageSafety,
        action: String
    ) -> AppScanItem {
        AppScanItem(
            name: URL(fileURLWithPath: path).lastPathComponent,
            path: path,
            sizeBytes: 10,
            percentOfRoot: 1,
            isDir: true,
            partial: false,
            safety: safety,
            cleanKind: "cache",
            cleanupAction: action,
            cleanupReason: "test"
        )
    }

    private func recipeItem(
        path: String,
        removable: Bool,
        appEligible: Bool
    ) -> RecipeItem {
        RecipeItem(
            label: URL(fileURLWithPath: path).lastPathComponent,
            path: path,
            sizeBytes: 10,
            kind: "cache",
            risk: "safe",
            reason: "test",
            removable: removable,
            safety: .safe,
            appEligible: appEligible
        )
    }
}
