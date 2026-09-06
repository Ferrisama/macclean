import Foundation

enum FullDiskAccessStatus: String, Sendable {
    case checking
    case granted
    case denied
    case unavailable

    var title: String {
        switch self {
        case .checking: "Checking…"
        case .granted: "Full Disk Access enabled"
        case .denied: "Full Disk Access needed"
        case .unavailable: "Full Disk Access unverified"
        }
    }

    var detail: String {
        switch self {
        case .checking:
            "Checking access to macOS-protected storage locations."
        case .granted:
            "MacClean can inspect protected developer and application storage."
        case .denied:
            "Scans can miss Mail, Messages, Safari, containers, and other protected data."
        case .unavailable:
            "No protected probe location exists on this Mac, so permission could not be verified."
        }
    }
}

struct FullDiskAccessResult: Sendable {
    let status: FullDiskAccessStatus
    let checkedLocations: [String]
    let deniedLocations: [String]
}

enum FullDiskAccessChecker {
    static func check() -> FullDiskAccessResult {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let probes = [
            home.appendingPathComponent("Library/Messages"),
            home.appendingPathComponent("Library/Mail"),
            home.appendingPathComponent("Library/Safari")
        ]

        var checked: [String] = []
        var denied: [String] = []
        var readable = 0

        for url in probes where FileManager.default.fileExists(atPath: url.path) {
            checked.append(url.path)
            do {
                _ = try FileManager.default.contentsOfDirectory(
                    at: url,
                    includingPropertiesForKeys: nil,
                    options: [.skipsHiddenFiles]
                )
                readable += 1
            } catch {
                denied.append(url.path)
            }
        }

        if readable > 0 {
            return FullDiskAccessResult(status: .granted, checkedLocations: checked, deniedLocations: denied)
        }
        if !denied.isEmpty {
            return FullDiskAccessResult(status: .denied, checkedLocations: checked, deniedLocations: denied)
        }
        return FullDiskAccessResult(status: .unavailable, checkedLocations: checked, deniedLocations: denied)
    }
}
