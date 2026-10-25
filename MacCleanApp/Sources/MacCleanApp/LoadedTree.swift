import Foundation

struct ScanInventory: Codable, Sendable {
    let root: String
    let nodes: [InventoryNode]
}

struct InventoryNode: Codable, Sendable {
    let name: String
    let parent: Int?
    let sizeBytes: UInt64
    let isDir: Bool
    let partial: Bool
    var logicalSizeBytes: UInt64? = nil
    var hardLinkDuplicate: Bool? = nil
}

/// Immutable scan observations. Navigation does not stat or enumerate the filesystem.
final class LoadedTree: @unchecked Sendable {
    let inventory: ScanInventory
    private let directoryIDs: [String: Int]
    private let directoryPaths: [Int: String]
    private let childIDs: [Int: [Int]]
    private let searchOrder: [Int]
    private let candidates: [String: AppScanItem]
    let fileCount: Int
    let folderCount: Int
    let cleanupItems: [AppScanItem]

    init?(inventory: ScanInventory, candidates: [AppScanItem]) {
        guard let first = inventory.nodes.first, first.parent == nil, first.isDir,
              inventory.root.hasPrefix("/") else { return nil }
        var ids = [inventory.root: 0]
        var paths = [0: inventory.root]
        var children: [Int: [Int]] = [:]
        var files = 0
        var folders = 1
        for (id, node) in inventory.nodes.enumerated().dropFirst() {
            guard let parent = node.parent, parent >= 0, parent < id,
                  inventory.nodes[parent].isDir, let parentPath = paths[parent],
                  !node.name.isEmpty, node.name != ".", node.name != "..", !node.name.contains("/") else { return nil }
            children[parent, default: []].append(id)
            if node.isDir {
                let path = parentPath == "/" ? "/" + node.name : parentPath + "/" + node.name
                guard ids[path] == nil else { return nil }
                ids[path] = id
                paths[id] = path
                folders += 1
            } else { files += 1 }
        }
        for parent in Array(children.keys) {
            children[parent]?.sort {
                let left = inventory.nodes[$0], right = inventory.nodes[$1]
                return left.sizeBytes == right.sizeBytes ? left.name.localizedStandardCompare(right.name) == .orderedAscending : left.sizeBytes > right.sizeBytes
            }
        }
        self.inventory = inventory
        directoryIDs = ids
        directoryPaths = paths
        childIDs = children
        let catalog = Dictionary(candidates.map { ($0.path, $0) }, uniquingKeysWith: { a, _ in a })
        self.candidates = catalog
        cleanupItems = catalog.values.sorted {
            $0.sizeBytes == $1.sizeBytes ? $0.path < $1.path : $0.sizeBytes > $1.sizeBytes
        }
        searchOrder = Array(inventory.nodes.indices.dropFirst()).sorted {
            let left = inventory.nodes[$0], right = inventory.nodes[$1]
            return left.sizeBytes == right.sizeBytes ? $0 < $1 : left.sizeBytes > right.sizeBytes
        }
        fileCount = files
        folderCount = folders
    }

    func containsDirectory(_ path: String) -> Bool { directoryIDs[path] != nil }
    func folder(_ path: String) -> AppScanItem? { directoryIDs[path].map(item) }
    func children(of path: String) -> [AppScanItem]? {
        guard let id = directoryIDs[path] else { return nil }
        return (childIDs[id] ?? []).map(item)
    }
    /// Queries immutable observations, never the filesystem. Results are bounded;
    /// the count still covers every matching node, including empty/sparse files.
    func search(query: String, kind: InventorySearchKind = .all, minimumBytes: UInt64 = 0,
                limit: Int = 100) throws -> InventorySearchPage {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
        var total = 0
        var items: [AppScanItem] = []
        for (position, id) in searchOrder.enumerated() {
            if position % 256 == 0 { try Task.checkCancellation() }
            let node = inventory.nodes[id]
            guard node.sizeBytes >= minimumBytes,
                  kind == .all || (kind == .folders ? node.isDir : !node.isDir) else { continue }
            guard needle.isEmpty || node.name.localizedCaseInsensitiveContains(needle)
                    || path(of: id).localizedCaseInsensitiveContains(needle) else { continue }
            total += 1
            if items.count < max(0, limit) { items.append(item(id)) }
        }
        try Task.checkCancellation()
        return InventorySearchPage(items: items, totalCount: total)
    }

    private func path(of id: Int) -> String {
        if let known = directoryPaths[id] { return known }
        if let parent = inventory.nodes[id].parent, let base = directoryPaths[parent] {
            return base == "/" ? "/" + inventory.nodes[id].name : base + "/" + inventory.nodes[id].name
        }
        return inventory.root
    }

    private func item(_ id: Int) -> AppScanItem {
        let node = inventory.nodes[id]
        let path = path(of: id)
        if let candidate = candidates[path] { return candidate }
        let parentSize = node.parent.map { inventory.nodes[$0].sizeBytes } ?? node.sizeBytes
        return AppScanItem(name: node.name, path: path, sizeBytes: node.sizeBytes,
                           percentOfRoot: parentSize > 0 ? Double(node.sizeBytes) / Double(parentSize) * 100 : 0,
                           isDir: node.isDir, partial: node.partial, safety: .unknown, cleanKind: "unknown",
                           cleanupAction: "Inspect", cleanupReason: "Measured in the loaded tree. Size or age alone does not establish cleanup eligibility.",
                           logicalSizeBytes: node.logicalSizeBytes, hardLinkDuplicate: node.hardLinkDuplicate)
    }
}
