import Foundation
import XCTest
@testable import MacCleanApp

final class ProcessPipelineTests: XCTestCase {
    func testRunDrainsStdoutThroughEOF() async throws {
        let service = MacCleanService()
        let data = try await service.run(
            executable: URL(fileURLWithPath: "/bin/sh"),
            arguments: ["-c", "printf 'first'; printf 'last'"]
        )
        XCTAssertEqual(String(decoding: data, as: UTF8.self), "firstlast")
    }

    func testStreamingReturnsFinalEnvelopeWithoutRetainingProgressAsResult() async throws {
        let service = MacCleanService()
        let result = try await service.runStreaming(
            executable: URL(fileURLWithPath: "/bin/sh"),
            arguments: [
                "-c",
                "printf '%s\\n' '{\"type\":\"progress\",\"data\":{}}' '{\"type\":\"result\",\"data\":{\"value\":42}}'"
            ],
            jobID: UUID(),
            onLine: { _ in }
        )
        let object = try JSONSerialization.jsonObject(with: result) as? [String: Int]
        XCTAssertEqual(object?["value"], 42)
    }

    func testCancellingRunTerminatesTheChild() async {
        let service = MacCleanService()
        let task = Task {
            try await service.run(
                executable: URL(fileURLWithPath: "/bin/sh"),
                arguments: ["-c", "sleep 30"]
            )
        }
        task.cancel()
        do {
            _ = try await task.value
            XCTFail("Expected cancellation")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }
}
