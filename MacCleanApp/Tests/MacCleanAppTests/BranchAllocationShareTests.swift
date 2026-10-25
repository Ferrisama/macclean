import XCTest
@testable import MacCleanApp

final class BranchAllocationShareTests: XCTestCase {
    func testEveryBranchUsesTheSameFolderDenominator() throws {
        XCTAssertEqual(try XCTUnwrap(BranchAllocationShare(bytes: 80, folderBytes: 100).fraction), 0.8, accuracy: 0.000001)
        XCTAssertEqual(try XCTUnwrap(BranchAllocationShare(bytes: 15, folderBytes: 100).fraction), 0.15, accuracy: 0.000001)
        XCTAssertEqual(BranchAllocationShare(bytes: 100, folderBytes: 100).fraction, 1)
        XCTAssertEqual(BranchAllocationShare(bytes: 0, folderBytes: 100).fraction, 0)
        XCTAssertEqual(BranchAllocationShare(bytes: 1, folderBytes: UInt64.max).label, "<0.1%")
    }

    func testUnknownEmptyAndInconsistentTotalsDoNotInventAProportion() {
        XCTAssertNil(BranchAllocationShare(bytes: 10, folderBytes: nil).fraction)
        XCTAssertNil(BranchAllocationShare(bytes: 0, folderBytes: 0).fraction)
        XCTAssertNil(BranchAllocationShare(bytes: 101, folderBytes: 100).fraction)
        XCTAssertEqual(BranchAllocationShare(bytes: 0, folderBytes: 0).label, "—")
        XCTAssertEqual(BranchAllocationShare(bytes: UInt64.max, folderBytes: UInt64.max).fraction, 1)
    }
}
