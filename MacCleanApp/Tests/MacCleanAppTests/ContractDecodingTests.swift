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

    func testDuplicateCleanupResponseDecodesBackendSnakeCaseContract() throws {
        let json = """
        {
          "dry_run": true,
          "moved_count": 0,
          "failed_count": 0,
          "total_bytes": 10,
          "moved_bytes": 0,
          "reclaimed_bytes": 0,
          "groups": [{
            "id": "abc123",
            "keeper_path": "/tmp/fixture/a",
            "valid": true,
            "error": null
          }],
          "outcomes": [{
            "group_id": "abc123",
            "keeper_path": "/tmp/fixture/a",
            "path": "/tmp/fixture/b",
            "moved": false,
            "error": null,
            "review_token": "review-token"
          }]
        }
        """

        let response = try JSONDecoder.macclean.decode(
            DuplicateCleanupResponse.self,
            from: Data(json.utf8)
        )

        XCTAssertTrue(response.dryRun)
        XCTAssertEqual(response.groups.first?.id, "abc123")
        XCTAssertEqual(response.groups.first?.keeperPath, "/tmp/fixture/a")
        XCTAssertEqual(response.outcomes.first?.path, "/tmp/fixture/b")
        XCTAssertEqual(response.outcomes.first?.reviewToken, "review-token")
    }
}
