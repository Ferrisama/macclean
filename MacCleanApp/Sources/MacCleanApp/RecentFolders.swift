import Foundation

struct RecentFolder: Codable, Identifiable, Equatable {
    var id: String { path }
    let path: String
    let scannedAt: Date
    static let key = "macclean.recentScanFolders.v1"

    static func recording(_ path: String, in folders: [RecentFolder], at date: Date = Date()) -> [RecentFolder] {
        guard path.hasPrefix("/") else { return folders }
        let normalized = URL(fileURLWithPath: path).standardizedFileURL.path
        return Array(([RecentFolder(path: normalized, scannedAt: date)] + folders.filter { $0.path != normalized }).prefix(8))
    }
    static func load(defaults: UserDefaults = .standard) -> [RecentFolder] {
        guard let data = defaults.data(forKey: key), let folders = try? JSONDecoder().decode([RecentFolder].self, from: data) else { return [] }
        return Array(folders.filter { $0.path.hasPrefix("/") }.prefix(8))
    }
    static func save(_ folders: [RecentFolder], defaults: UserDefaults = .standard) {
        if let data = try? JSONEncoder().encode(folders) { defaults.set(data, forKey: key) }
    }
}

struct ItemUsage {
    let modified: Date?
    let lastOpened: String?
    let appName: String?
    let appLastOpened: String?
    let appRunning: Bool?
}

struct CacheCatalog: Codable {
    let items: [AppScanItem]
    let warnings: [String]
}

struct ApplicationActivity: Identifiable {
    var id: String { path }
    let path: String
    let name: String
    let lastOpened: Date?
    let running: Bool
}
