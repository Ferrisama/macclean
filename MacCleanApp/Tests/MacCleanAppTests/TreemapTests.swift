import CoreGraphics
import XCTest
@testable import MacCleanApp

final class TreemapTests: XCTestCase {
    func testTilesConserveAreaAndDoNotOverlap() {
        let items = [item("large", 60), item("medium", 30), item("small", 10)]
        let bounds = CGRect(x: 0, y: 0, width: 800, height: 500)
        let entries = treemapRects(items: items, in: bounds)

        XCTAssertEqual(entries.count, items.count)
        XCTAssertEqual(entries.reduce(0) { $0 + $1.rect.width * $1.rect.height }, bounds.width * bounds.height, accuracy: 0.01)
        for entry in entries {
            XCTAssertTrue(bounds.contains(entry.rect))
        }
        for (index, entry) in entries.enumerated() {
            for other in entries.dropFirst(index + 1) {
                XCTAssertLessThanOrEqual(entry.rect.intersection(other.rect).width * entry.rect.intersection(other.rect).height, 0.001)
            }
        }
    }

    func testTileAreasTrackByteWeights() {
        let entries = treemapRects(items: [item("one", 1), item("three", 3)], in: CGRect(x: 0, y: 0, width: 400, height: 400))
        let oneArea = entries.first { $0.item.name == "one" }!.rect.width * entries.first { $0.item.name == "one" }!.rect.height
        let threeArea = entries.first { $0.item.name == "three" }!.rect.width * entries.first { $0.item.name == "three" }!.rect.height
        XCTAssertEqual(threeArea / oneArea, 3, accuracy: 0.001)
    }

    private func item(_ name: String, _ size: UInt64) -> AppScanItem {
        AppScanItem(name: name, path: "/tmp/\(name)", sizeBytes: size, percentOfRoot: 0, isDir: true, partial: false, safety: .safe, cleanKind: "cache", cleanupAction: "Move to Trash", cleanupReason: "test")
    }
}
