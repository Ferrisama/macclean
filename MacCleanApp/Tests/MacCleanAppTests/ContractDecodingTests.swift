import XCTest
@testable import MacCleanApp

final class ContractDecodingTests: XCTestCase {
    func testDuplicateReportDecodesBackendSnakeCaseContract() throws {
        let json = """
        {
          "schema_version": 1,
          "root": "/tmp/fixture",
          "min_bytes": 1,
          "scanned_files": 3,
          "hashed_files": 2,
          "partial": false,
          "error_count": 0,
          "total_wasted_bytes": 10,
          "groups": [{
            "id": "abc123",
            "size_bytes": 10,
            "wasted_bytes": 10,
            "files": [
              {"path": "/tmp/fixture/a", "size_bytes": 10, "modified_at": 1},
              {"path": "/tmp/fixture/b", "size_bytes": 10, "modified_at": 2}
            ]
          }]
        }
        """

        let report = try JSONDecoder.macclean.decode(
            DuplicateReport.self,
            from: Data(json.utf8)
        )

        XCTAssertEqual(report.groups.count, 1)
        XCTAssertEqual(report.groups[0].files.count, 2)
        XCTAssertEqual(report.totalWastedBytes, 10)
        XCTAssertFalse(report.partial)
    }
}
