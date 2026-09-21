import Foundation

final class CompletionGate: @unchecked Sendable {
    private let lock = NSLock()
    private var completed = false

    var isPending: Bool {
        lock.lock()
        defer { lock.unlock() }
        return !completed
    }

    func tryComplete() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if completed {
            return false
        }
        completed = true
        return true
    }
}

final class ProcessOutputBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var data = Data()
    private let limit: Int?

    init(limit: Int? = nil) {
        self.limit = limit
    }

    func append(_ newData: Data) {
        lock.lock()
        data.append(newData)
        if let limit, data.count > limit {
            data.removeFirst(data.count - limit)
        }
        lock.unlock()
    }

    func snapshot() -> Data {
        lock.lock()
        defer { lock.unlock() }
        return data
    }

    func replace(with newData: Data) {
        lock.lock()
        data = newData
        lock.unlock()
    }

    func appendAndTakeLines(_ newData: Data, finish: Bool = false) -> [Data] {
        lock.lock()
        defer { lock.unlock() }
        data.append(newData)
        var lines: [Data] = []
        while let newline = data.firstIndex(of: 0x0A) {
            lines.append(Data(data[..<newline]))
            data.removeSubrange(...newline)
        }
        if finish, !data.isEmpty {
            lines.append(data)
            data.removeAll(keepingCapacity: false)
        }
        return lines
    }
}

final class ProcessReference: @unchecked Sendable {
    private let lock = NSLock()
    private var process: Process?
    private var cancelled = false

    func attach(_ process: Process) -> Bool {
        lock.lock()
        self.process = process
        let shouldCancel = cancelled
        lock.unlock()
        return shouldCancel
    }

    func cancel() {
        lock.lock()
        cancelled = true
        let process = process
        lock.unlock()
        if process?.isRunning == true {
            process?.terminate()
        }
    }

    var wasCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }
}

enum MacCleanServiceError: LocalizedError {
    case binaryNotFound
    case launchFailed(String)
    case failedStatus(Int32, String)
    case invalidOutput(String)

    var errorDescription: String? {
        switch self {
        case .binaryNotFound:
            "Could not find macclean. Build the Rust binary or set MACCLEAN_BIN."
        case .launchFailed(let message):
            message
        case .failedStatus(let status, let output):
            "macclean exited with status \(status): \(output)"
        case .invalidOutput(let message):
            message
        }
    }
}

final class MacCleanService {
    private let processLock = NSLock()
    private var scanProcesses: [UUID: ProcessReference] = [:]

    func cancelScan(jobID: UUID) {
        processLock.lock()
        let process = scanProcesses[jobID]
        processLock.unlock()
        process?.cancel()
    }

    private func registerScan(_ reference: ProcessReference, jobID: UUID) {
        processLock.lock()
        scanProcesses[jobID] = reference
        processLock.unlock()
    }

    private func unregisterScan(jobID: UUID) {
        processLock.lock()
        scanProcesses.removeValue(forKey: jobID)
        processLock.unlock()
    }

    func appScan(
        path: String,
        depth: Int = 2,
        limit: Int = 40,
        deep: Bool = false,
        includeSystemData: Bool = true,
        includeHealth: Bool = true,
        jobID: UUID,
        onProgress: (@Sendable (AppScanProgress) -> Void)? = nil
    ) async throws -> AppScan {
        let executable = try resolveBinary()
        var arguments = [
            "app-scan",
            path,
            "--depth",
            String(depth),
            "--limit",
            String(limit)
        ]
        if deep {
            arguments.append("--deep")
        }
        if !includeSystemData {
            arguments.append("--no-system-data")
        }
        if !includeHealth {
            arguments.append("--no-health")
        }

        if let onProgress {
            arguments.append("--progress")
            let data = try await runStreaming(executable: executable, arguments: arguments, jobID: jobID) { line in
                guard let lineData = line.data(using: .utf8),
                      let envelope = try? JSONDecoder.macclean.decode(StreamProgressEnvelope.self, from: lineData),
                      envelope.type == "progress",
                      let decoded = envelope.data
                else { return }
                onProgress(decoded)
            }
            return try decodeScan(data)
        }
        let data = try await run(executable: executable, arguments: arguments)
        return try decodeScan(data)
    }

    private func decodeScan(_ data: Data) throws -> AppScan {
        do {
            return try JSONDecoder.macclean.decode(AppScan.self, from: data)
        } catch {
            let text = String(data: data, encoding: .utf8) ?? "<non-utf8 output>"
            throw MacCleanServiceError.invalidOutput("\(error)\n\n\(text.prefix(1200))")
        }
    }

    private struct StreamProgressEnvelope: Decodable {
        let type: String
        let data: AppScanProgress?

        enum CodingKeys: String, CodingKey { case type, data }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            type = try container.decode(String.self, forKey: .type)
            if type == "progress" {
                data = try container.decodeIfPresent(AppScanProgress.self, forKey: .data)
            } else {
                data = nil
            }
        }
    }

    func history(limit: Int = 30) async throws -> [HistorySession] {
        let executable = try resolveBinary()
        let data = try await run(executable: executable, arguments: ["app-history", "--limit", String(limit)])
        return try JSONDecoder.macclean.decode([HistorySession].self, from: data)
    }

    func cachedScan() async throws -> AppScan? {
        let executable = try resolveBinary()
        let data = try await run(executable: executable, arguments: ["app-cache"])
        return try JSONDecoder.macclean.decode(AppScan?.self, from: data)
    }

    func recipes() async throws -> AppRecipes {
        let executable = try resolveBinary()
        let data = try await run(executable: executable, arguments: ["app-recipes"])
        return try JSONDecoder.macclean.decode(AppRecipes.self, from: data)
    }

    func installedApplications() async throws -> [InstalledApplication] {
        let executable = try resolveBinary()
        let data = try await run(executable: executable, arguments: ["app-uninstall-list"])
        return try JSONDecoder.macclean.decode([InstalledApplication].self, from: data)
    }

    func uninstallPlan(path: String, deep: Bool) async throws -> UninstallPlan {
        let executable = try resolveBinary()
        var arguments = ["app-uninstall-plan", path]
        if deep { arguments.append("--deep") }
        let data = try await run(executable: executable, arguments: arguments)
        return try JSONDecoder.macclean.decode(UninstallPlan.self, from: data)
    }

    func duplicateScan(
        path: String,
        minMB: UInt64,
        jobID: UUID,
        onProgress: @escaping @Sendable (DuplicateScanProgress) -> Void
    ) async throws -> DuplicateReport {
        let executable = try resolveBinary()
        let data = try await runStreaming(
            executable: executable,
            arguments: ["app-dupes", path, "--min", String(minMB), "--progress"],
            jobID: jobID
        ) { line in
            guard let lineData = line.data(using: .utf8),
                  let envelope = try? JSONDecoder.macclean.decode(
                    DuplicateProgressEnvelope.self,
                    from: lineData
                  ),
                  envelope.type == "progress",
                  let progress = envelope.data
            else { return }
            onProgress(progress)
        }
        return try JSONDecoder.macclean.decode(DuplicateReport.self, from: data)
    }

    private struct DuplicateProgressEnvelope: Decodable {
        let type: String
        let data: DuplicateScanProgress?
    }

    func duplicateCleanup(
        request: DuplicateCleanupRequest,
        reviewTokens: [String: String] = [:],
        dryRun: Bool
    ) async throws -> DuplicateCleanupResponse {
        struct BackendSelection: Encodable {
            let path: String
            let reviewToken: String?
        }
        struct BackendGroup: Encodable {
            let id: String
            let keeperPath: String
            let files: [String]
            let selected: [BackendSelection]
        }
        struct BackendRequest: Encodable {
            let groups: [BackendGroup]
        }

        let backendRequest = BackendRequest(groups: request.groups.map { group in
            BackendGroup(
                id: group.groupID,
                keeperPath: group.keeperPath,
                files: group.reviewedMemberPaths,
                selected: group.deletePaths.map {
                    BackendSelection(path: $0, reviewToken: reviewTokens[$0])
                }
            )
        })
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let requestData = try encoder.encode(backendRequest)
        let requestURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("macclean-duplicate-review-\(UUID().uuidString).json")
        try requestData.write(to: requestURL, options: .atomic)
        defer { try? FileManager.default.removeItem(at: requestURL) }

        let executable = try resolveBinary()
        var arguments: [String] = []
        if dryRun { arguments.append("--dry-run") }
        arguments.append(contentsOf: [
            "app-dupes-trash",
            "--request-file",
            requestURL.path
        ])
        let data = try await run(executable: executable, arguments: arguments)
        return try JSONDecoder.macclean.decode(DuplicateCleanupResponse.self, from: data)
    }

    func trash(
        paths: [String],
        reviewTokens: [String] = [],
        dryRun: Bool = false
    ) async throws -> AppTrashResponse {
        let executable = try resolveBinary()
        var arguments: [String] = []
        if dryRun {
            arguments.append("--dry-run")
        }
        arguments.append("app-trash")
        arguments.append(contentsOf: paths)
        for token in reviewTokens {
            arguments.append(contentsOf: ["--review-token", token])
        }
        let data = try await run(executable: executable, arguments: arguments)
        return try JSONDecoder.macclean.decode(AppTrashResponse.self, from: data)
    }

    func restore(sessionId: String?) async throws -> AppRestoreResponse {
        let executable = try resolveBinary()
        var arguments = ["app-restore"]
        if let sessionId {
            arguments.append(sessionId)
        }
        let data = try await run(executable: executable, arguments: arguments)
        return try JSONDecoder.macclean.decode(AppRestoreResponse.self, from: data)
    }

    private func resolveBinary() throws -> URL {
        let env = ProcessInfo.processInfo.environment
        if let configured = env["MACCLEAN_BIN"], FileManager.default.isExecutableFile(atPath: configured) {
            return URL(fileURLWithPath: configured)
        }

        if let bundled = Bundle.main.url(forResource: "macclean", withExtension: nil),
           FileManager.default.isExecutableFile(atPath: bundled.path) {
            return bundled
        }

        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let candidates = [
            cwd.appendingPathComponent("../target/release/macclean").standardizedFileURL,
            cwd.appendingPathComponent("../target/debug/macclean").standardizedFileURL,
            cwd.appendingPathComponent("target/release/macclean").standardizedFileURL,
            cwd.appendingPathComponent("target/debug/macclean").standardizedFileURL
        ]
        if let local = candidates.first(where: { FileManager.default.isExecutableFile(atPath: $0.path) }) {
            return local
        }

        let which = Process()
        which.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        which.arguments = ["macclean"]
        let pipe = Pipe()
        which.standardOutput = pipe
        try? which.run()
        which.waitUntilExit()
        if which.terminationStatus == 0,
           let path = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !path.isEmpty {
            return URL(fileURLWithPath: path)
        }

        throw MacCleanServiceError.binaryNotFound
    }

    func run(executable: URL, arguments: [String]) async throws -> Data {
        let reference = ProcessReference()
        return try await withTaskCancellationHandler(operation: {
            try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            process.executableURL = executable
            process.arguments = arguments

            let output = Pipe()
            let errorOutput = Pipe()
            process.standardOutput = output
            process.standardError = errorOutput
            let gate = CompletionGate()
            let outputBuffer = ProcessOutputBuffer()
            let errorBuffer = ProcessOutputBuffer(limit: 64 * 1024)
            let readers = DispatchGroup()
            readers.enter()
            readers.enter()

            @Sendable func finish(_ result: Result<Data, Error>) {
                guard gate.tryComplete() else { return }
                switch result {
                case .success(let data):
                    continuation.resume(returning: data)
                case .failure(let error):
                    continuation.resume(throwing: error)
                }
            }

            process.terminationHandler = { process in
                readers.notify(queue: .global(qos: .utility)) {
                    if reference.wasCancelled {
                        finish(.failure(CancellationError()))
                    } else if process.terminationStatus == 0 {
                        finish(.success(outputBuffer.snapshot()))
                    } else {
                        let text = String(data: errorBuffer.snapshot(), encoding: .utf8) ?? ""
                        finish(.failure(MacCleanServiceError.failedStatus(process.terminationStatus, text)))
                    }
                }
            }

            do {
                try process.run()
                output.fileHandleForWriting.closeFile()
                errorOutput.fileHandleForWriting.closeFile()
                _ = reference.attach(process)
                DispatchQueue.global(qos: .utility).async {
                    outputBuffer.append(output.fileHandleForReading.readDataToEndOfFile())
                    readers.leave()
                }
                DispatchQueue.global(qos: .utility).async {
                    errorBuffer.append(errorOutput.fileHandleForReading.readDataToEndOfFile())
                    readers.leave()
                }
                if reference.wasCancelled, process.isRunning {
                    process.terminate()
                }
            } catch {
                readers.leave()
                readers.leave()
                finish(.failure(MacCleanServiceError.launchFailed(error.localizedDescription)))
            }
            }
        }, onCancel: {
            reference.cancel()
        })
    }

    func runStreaming(
        executable: URL,
        arguments: [String],
        jobID: UUID,
        onLine: @escaping @Sendable (String) -> Void
    ) async throws -> Data {
        let reference = ProcessReference()
        registerScan(reference, jobID: jobID)
        return try await withTaskCancellationHandler(operation: {
            try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            process.executableURL = executable
            process.arguments = arguments
            let output = Pipe()
            let errorOutput = Pipe()
            process.standardOutput = output
            process.standardError = errorOutput
            let gate = CompletionGate()
            let lineBuffer = ProcessOutputBuffer()
            let resultBuffer = ProcessOutputBuffer()
            let errorBuffer = ProcessOutputBuffer(limit: 64 * 1024)
            let readers = DispatchGroup()
            readers.enter()
            readers.enter()
            @Sendable func finish(_ result: Result<Data, Error>) {
                guard gate.tryComplete() else { return }
                continuation.resume(with: result)
            }
            process.terminationHandler = { process in
                readers.notify(queue: .global(qos: .utility)) {
                    self.unregisterScan(jobID: jobID)
                    if reference.wasCancelled {
                        finish(.failure(CancellationError()))
                    } else if process.terminationStatus == 0 {
                        let resultData = resultBuffer.snapshot()
                        if let object = try? JSONSerialization.jsonObject(with: resultData),
                       let dict = object as? [String: Any],
                       dict["type"] as? String == "result",
                       let payload = dict["data"] {
                            finish(.success((try? JSONSerialization.data(withJSONObject: payload)) ?? Data()))
                        } else {
                            finish(.failure(MacCleanServiceError.invalidOutput("Streaming scan did not return a result.")))
                        }
                    } else {
                        let text = String(data: errorBuffer.snapshot(), encoding: .utf8) ?? ""
                        finish(.failure(MacCleanServiceError.failedStatus(process.terminationStatus, text)))
                    }
                }
            }
            do {
                try process.run()
                output.fileHandleForWriting.closeFile()
                errorOutput.fileHandleForWriting.closeFile()
                _ = reference.attach(process)
                DispatchQueue.global(qos: .utility).async {
                    while true {
                        let data = output.fileHandleForReading.availableData
                        if data.isEmpty { break }
                        for line in lineBuffer.appendAndTakeLines(data) {
                            guard !line.isEmpty else { continue }
                            if let text = String(data: line, encoding: .utf8) {
                                onLine(text)
                            }
                            if let object = try? JSONSerialization.jsonObject(with: line),
                               let dictionary = object as? [String: Any],
                               dictionary["type"] as? String == "result" {
                                resultBuffer.replace(with: line)
                            }
                        }
                    }
                    for line in lineBuffer.appendAndTakeLines(Data(), finish: true) where !line.isEmpty {
                        if let text = String(data: line, encoding: .utf8) { onLine(text) }
                        if let object = try? JSONSerialization.jsonObject(with: line),
                           let dictionary = object as? [String: Any],
                           dictionary["type"] as? String == "result" {
                            resultBuffer.replace(with: line)
                        }
                    }
                    readers.leave()
                }
                DispatchQueue.global(qos: .utility).async {
                    errorBuffer.append(errorOutput.fileHandleForReading.readDataToEndOfFile())
                    readers.leave()
                }
                if reference.wasCancelled, process.isRunning {
                    process.terminate()
                }
            } catch {
                readers.leave()
                readers.leave()
                unregisterScan(jobID: jobID)
                finish(.failure(MacCleanServiceError.launchFailed(error.localizedDescription)))
            }
            }
        }, onCancel: {
            reference.cancel()
        })
    }
}
