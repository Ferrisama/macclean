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

    func append(_ newData: Data) {
        lock.lock()
        data.append(newData)
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
    private var scanProcesses: [UUID: Process] = [:]

    func cancelScan(jobID: UUID) {
        processLock.lock()
        let process = scanProcesses[jobID]
        processLock.unlock()
        if process?.isRunning == true {
            process?.terminate()
        }
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

    func trash(paths: [String], dryRun: Bool = false) async throws -> AppTrashResponse {
        let executable = try resolveBinary()
        var arguments: [String] = []
        if dryRun {
            arguments.append("--dry-run")
        }
        arguments.append("app-trash")
        arguments.append(contentsOf: paths)
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

    private func run(executable: URL, arguments: [String]) async throws -> Data {
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
            let errorBuffer = ProcessOutputBuffer()
            output.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                if !data.isEmpty {
                    outputBuffer.append(data)
                }
            }
            errorOutput.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                if !data.isEmpty {
                    errorBuffer.append(data)
                }
            }

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
                output.fileHandleForReading.readabilityHandler = nil
                errorOutput.fileHandleForReading.readabilityHandler = nil
                let data = outputBuffer.snapshot()
                if process.terminationStatus == 0 {
                    finish(.success(data))
                } else {
                    let text = String(data: errorBuffer.snapshot(), encoding: .utf8) ?? ""
                    finish(.failure(MacCleanServiceError.failedStatus(process.terminationStatus, text)))
                }
            }

            do {
                try process.run()
            } catch {
                finish(.failure(MacCleanServiceError.launchFailed(error.localizedDescription)))
            }
        }
    }

    private func runStreaming(
        executable: URL,
        arguments: [String],
        jobID: UUID,
        onLine: @escaping @Sendable (String) -> Void
    ) async throws -> Data {
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
            let lineBuffer = ProcessOutputBuffer()
            let errorBuffer = ProcessOutputBuffer()
            output.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                guard !data.isEmpty else { return }
                outputBuffer.append(data)
                let combined = lineBuffer.snapshot() + data
                let lines = combined.split(separator: 0x0A, omittingEmptySubsequences: true)
                let remainder = combined.last == 0x0A ? Data() : Data(lines.last ?? Data())
                lineBuffer.replace(with: remainder)
                let complete = combined.last == 0x0A ? lines : lines.dropLast()
                for line in complete {
                    if let text = String(data: line, encoding: .utf8) {
                        onLine(text)
                    }
                }
            }
            errorOutput.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                if !data.isEmpty {
                    errorBuffer.append(data)
                }
            }
            @Sendable func finish(_ result: Result<Data, Error>) {
                guard gate.tryComplete() else { return }
                continuation.resume(with: result)
            }
            process.terminationHandler = { process in
                self.processLock.lock()
                self.scanProcesses.removeValue(forKey: jobID)
                self.processLock.unlock()
                output.fileHandleForReading.readabilityHandler = nil
                errorOutput.fileHandleForReading.readabilityHandler = nil
                let data = outputBuffer.snapshot()
                if process.terminationStatus == 0 {
                    let lines = data.split(separator: 0x0A, omittingEmptySubsequences: true)
                    if let resultLine = lines.last,
                       let resultData = String(data: resultLine, encoding: .utf8)?.data(using: .utf8),
                       let object = try? JSONSerialization.jsonObject(with: resultData),
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
            do {
                try process.run()
                processLock.lock()
                scanProcesses[jobID] = process
                processLock.unlock()
            } catch { finish(.failure(MacCleanServiceError.launchFailed(error.localizedDescription))) }
        }
    }
}
