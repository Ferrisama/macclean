import Foundation
import SwiftUI

enum StorageSafety: String, Codable, CaseIterable, Identifiable {
    case safe
    case review
    case userData = "user-data"
    case protected
    case unknown

    var id: String { rawValue }

    var label: String {
        switch self {
        case .safe: "Safe"
        case .review: "Review"
        case .userData: "User Data"
        case .protected: "Protected"
        case .unknown: "Unknown"
        }
    }

    var color: Color {
        switch self {
        case .safe: .green
        case .review: .orange
        case .userData: .red
        case .protected: .purple
        case .unknown: .secondary
        }
    }

    var sortOrder: Int {
        switch self {
        case .safe: 0
        case .review: 1
        case .userData: 2
        case .protected: 3
        case .unknown: 4
        }
    }
}

struct AppScan: Codable {
    let schemaVersion: Int
    let rootScan: StorageScan
    let systemData: SystemDataScan?
    let largestItems: [AppScanItem]
    let cleanupCandidates: [AppScanItem]
    let safetyTotals: [SafetyTotal]
    let health: HealthSnapshot?
}

struct AppScanProgress: Codable {
    let stage: String
    let currentPath: String?
    let scannedItems: Int
    let estimatedItems: Int
    let progress: Double
    let partial: Bool
    let item: AppScanItem?
}

struct AppRecipes: Codable {
    let schemaVersion: Int
    let scannedAt: UInt64
    let elapsedMs: UInt64
    let recipes: [CleanupRecipe]
}

struct CleanupRecipe: Codable, Identifiable {
    let id: String
    let title: String
    let subtitle: String
    let safety: StorageSafety
    let totalBytes: UInt64
    let itemCount: Int
    let selectedByDefault: Bool
    let command: String
    let items: [RecipeItem]
}

struct RecipeItem: Codable, Identifiable {
    var id: String { path }
    let label: String
    let path: String
    let sizeBytes: UInt64
    let kind: String
    let risk: String
    let reason: String
    let removable: Bool
    let safety: StorageSafety
    let appEligible: Bool
}

struct StorageScan: Codable {
    let root: String
    let mode: String
    let depth: Int
    let limit: Int
    let scannedAt: UInt64
    let elapsedMs: UInt64
    let partial: Bool
    let incompleteReason: String?
    let metrics: ScanMetrics?
    let tree: StorageNode
}

struct ScanMetrics: Codable {
    let implementation: String
    let entriesSeen: UInt64
    let directoriesSeen: UInt64
    let filesSeen: UInt64
    let metadataErrors: UInt64
}

struct StorageNode: Codable, Identifiable {
    var id: String { path }
    let name: String
    let path: String
    let sizeBytes: UInt64
    let percentOfParent: Double
    let isDir: Bool
    let partial: Bool
    let safety: StorageSafety
    let cleanKind: String
    let cleanupAction: String
    let cleanupReason: String
    let children: [StorageNode]
}

struct SystemDataScan: Codable {
    let scannedAt: UInt64
    let elapsedMs: UInt64
    let totalBytes: UInt64
    let partial: Bool
    let categories: [StorageCategory]
    let note: String
}

struct StorageCategory: Codable, Identifiable {
    var id: String { name }
    let name: String
    let sizeBytes: UInt64
    let percentOfTotal: Double
    let safety: StorageSafety
    let cleanKind: String
    let paths: [String]
    let why: String
    let cleanWith: String
}

struct AppScanItem: Codable, Identifiable {
    var id: String { path }
    let name: String
    let path: String
    let sizeBytes: UInt64
    let percentOfRoot: Double
    let isDir: Bool
    let partial: Bool
    let safety: StorageSafety
    let cleanKind: String
    let cleanupAction: String
    let cleanupReason: String
}

extension AppScanItem {
    init(recipeItem: RecipeItem, recipe: CleanupRecipe) {
        name = recipeItem.label
        path = recipeItem.path
        sizeBytes = recipeItem.sizeBytes
        percentOfRoot = 0
        isDir = true
        partial = false
        safety = recipeItem.safety
        cleanKind = recipeItem.kind
        cleanupAction = recipeItem.appEligible ? "Move to Trash" : "Unavailable in app"
        cleanupReason = recipeItem.appEligible
            ? recipeItem.reason
            : "\(recipeItem.reason) This path is not eligible for direct app cleanup; use its named cleaner from the CLI instead."
    }

    var canMoveToTrash: Bool {
        cleanupAction == "Move to Trash"
    }
}

struct SafetyTotal: Codable, Identifiable {
    var id: StorageSafety { safety }
    let safety: StorageSafety
    let sizeBytes: UInt64
    let percentOfRoot: Double
    let itemCount: Int
}

struct HealthSnapshot: Codable {
    let diskTotal: UInt64
    let diskUsed: UInt64
    let diskFree: UInt64
    let memTotal: UInt64
    let memUsed: UInt64
    let ncpu: String
    let loadAvg: String
    let battery: String
    let filevault: Bool
    let firewall: Bool
    let sip: Bool
    let topSpace: [[TopSpaceValue]]

    var topSpaceRows: [(String, UInt64)] {
        topSpace.compactMap { row in
            guard row.count == 2,
                  case let .string(name) = row[0],
                  case let .number(size) = row[1]
            else {
                return nil
            }
            return (name, UInt64(size))
        }
    }
}

struct HistorySession: Codable, Identifiable {
    var id: String { sessionId }
    let sessionId: String
    let timestamp: UInt64
    let cleaner: String
    let itemCount: Int
    let totalBytes: UInt64
    let method: String
    let restorableCount: Int
}

struct AppTrashResponse: Codable {
    let dryRun: Bool
    let sessionId: String?
    let receiptError: String?
    let movedCount: Int
    let failedCount: Int
    let totalBytes: UInt64
    let outcomes: [AppTrashOutcome]
}

struct AppTrashOutcome: Codable, Identifiable {
    var id: String { path }
    let path: String
    let moved: Bool
    let trashPath: String?
    let error: String?
}

struct AppRestoreResponse: Codable {
    let restoredCount: Int
    let failedCount: Int
    let outcomes: [AppRestoreOutcome]
}

struct AppRestoreOutcome: Codable, Identifiable {
    var id: String { path }
    let sessionId: String
    let label: String
    let path: String
    let restored: Bool
    let error: String?
}

struct InstalledApplication: Codable, Identifiable, Equatable {
    var id: String { path }
    let name: String
    let path: String
    let bundleId: String?
    let protected: Bool
}

struct UninstallPlan: Codable {
    let appName: String
    let bundleId: String
    let bundleIdGuessed: Bool
    let appPath: String
    let items: [UninstallTraceItem]
    let totalSize: UInt64
    let deep: Bool
    let runningProcesses: [RunningApplicationProcess]
    let canExecute: Bool
}

struct RunningApplicationProcess: Codable, Identifiable {
    var id: Int { pid }
    let pid: Int
    let command: String
}

struct UninstallTraceItem: Codable, Identifiable {
    var id: String { path }
    let path: String
    let sizeBytes: UInt64
    let reason: String
    let risk: String
}

enum TopSpaceValue: Codable {
    case string(String)
    case number(Double)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let string = try? container.decode(String.self) {
            self = .string(string)
        } else {
            self = .number(try container.decode(Double.self))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let value):
            try container.encode(value)
        case .number(let value):
            try container.encode(value)
        }
    }
}

extension JSONDecoder {
    static var macclean: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }
}

func formatBytes(_ bytes: UInt64) -> String {
    ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
}
