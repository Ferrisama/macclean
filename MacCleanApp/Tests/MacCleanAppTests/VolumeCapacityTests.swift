import Foundation
import XCTest
@testable import MacCleanApp

final class VolumeCapacityTests: XCTestCase {
    func testCapacityKeepsOSVolumeTotalsSeparateFromScannedBytes() throws {
        let capacity = try VolumeCapacity(name: "Fixture", volumePath: "/", totalBytes: 1000, availableBytes: 250)
        XCTAssertEqual(capacity.occupiedBytes, 750)
        XCTAssertEqual(capacity.occupiedFraction, 0.75)
        XCTAssertThrowsError(try VolumeCapacity(name: "Invalid", volumePath: "/", totalBytes: 0, availableBytes: 0))
        XCTAssertThrowsError(try VolumeCapacity(name: "Invalid", volumePath: "/", totalBytes: 1, availableBytes: 2))
        XCTAssertThrowsError(try VolumeCapacity(name: "Invalid", volumePath: "/", totalBytes: 1, availableBytes: -1))
    }

    func testReadsActualVolumeWithoutAStorageScan() throws {
        let capacity = try VolumeCapacity.read(at: FileManager.default.temporaryDirectory.path)
        XCTAssertGreaterThan(capacity.totalBytes, 0)
        XCTAssertLessThanOrEqual(capacity.availableBytes, capacity.totalBytes)
        XCTAssertTrue(capacity.volumePath.hasPrefix("/"))
    }

    @MainActor
    func testCapacityAvailableBeforeFirstScanAndFailureClearsOldFigures() async throws {
        let reader = FailingAfterFirstRead()
        let model = AppModel(capacityReader: { try reader.read($0) })
        await model.refreshVolumeCapacity()
        XCTAssertNil(model.scan)
        XCTAssertEqual(model.volumeCapacity?.availableBytes, 250)
        await model.refreshVolumeCapacity()
        XCTAssertNil(model.volumeCapacity)
        XCTAssertNotNil(model.volumeCapacityError)
        XCTAssertFalse(model.isLoadingVolumeCapacity)
        XCTAssertFalse(model.isScanning)
    }
}

private final class FailingAfterFirstRead: @unchecked Sendable {
    private let lock = NSLock()
    private var calls = 0
    func read(_ path: String) throws -> VolumeCapacity {
        lock.lock()
        defer { lock.unlock() }
        calls += 1
        guard calls == 1 else { throw CocoaError(.fileReadNoPermission) }
        return try VolumeCapacity(name: "Fixture", volumePath: "/", totalBytes: 1000, availableBytes: 250)
    }
}
