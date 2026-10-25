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
        let object = try JSONSerialization.jsonObject(with: result) as? [String: Any]
        XCTAssertEqual(object?["type"] as? String, "result")
        XCTAssertEqual((object?["data"] as? [String: Int])?["value"], 42)
    }

    func testFramingRetainsLargeSplitRecordAndFinalUnterminatedLine() {
        let buffer = ProcessOutputBuffer()
        let first = Data(repeating: 65, count: 200_000)
        XCTAssertTrue(buffer.appendAndTakeLines(first).isEmpty)
        XCTAssertTrue(buffer.appendAndTakeLines(Data("tail".utf8)).isEmpty)
        let lines = buffer.appendAndTakeLines(Data("\nnext\nlast".utf8))
        XCTAssertEqual(lines.count, 2)
        XCTAssertEqual(lines[0], first + Data("tail".utf8))
        XCTAssertEqual(String(decoding: lines[1], as: UTF8.self), "next")
        XCTAssertEqual(buffer.appendAndTakeLines(Data(), finish: true), [Data("last".utf8)])
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
