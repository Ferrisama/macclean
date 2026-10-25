import Foundation

enum InventorySearchKind: String, CaseIterable, Identifiable, Sendable {
    case all = "Files & folders", files = "Files", folders = "Folders"
    var id: String { rawValue }
}

enum InventorySearchSize: UInt64, CaseIterable, Identifiable, Sendable {
    case any = 0, hundredMB = 100_000_000, oneGB = 1_000_000_000
    var id: UInt64 { rawValue }
    var title: String {
        switch self {
        case .any: "Any size"
        case .hundredMB: "At least 100 MB"
        case .oneGB: "At least 1 GB"
        }
    }
}

struct InventorySearchRequest: Equatable {
    let tree: ObjectIdentifier?
    let query: String
    let kind: InventorySearchKind
    let minimumSize: InventorySearchSize
    let limit: Int
    let scanning: Bool
}

struct InventorySearchPage {
    let items: [AppScanItem]
    let totalCount: Int
}

extension AppModel {
    var isInventorySearchActive: Bool {
        !inventorySearchQuery.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        || inventorySearchKind != .all || inventorySearchSize != .any
    }

    var inventorySearchRequest: InventorySearchRequest {
        InventorySearchRequest(tree: loadedTree.map(ObjectIdentifier.init),
                               query: inventorySearchQuery, kind: inventorySearchKind,
                               minimumSize: inventorySearchSize, limit: inventorySearchLimit,
                               scanning: isScanning)
    }

    func clearInventorySearch() {
        inventorySearchQuery = ""
        inventorySearchKind = .all
        inventorySearchSize = .any
        inventorySearchLimit = 100
        inventorySearchPage = nil
    }

    func searchInventory(debounce: Bool = true) async {
        let ticket = UUID()
        inventorySearchTicket = ticket
        inventorySearchPage = nil
        isSearchingInventory = false
        guard isInventorySearchActive, !isScanning, let tree = loadedTree else { return }
        let request = inventorySearchRequest
        isSearchingInventory = true
        defer { if inventorySearchTicket == ticket { isSearchingInventory = false } }
        do {
            if debounce { try await Task.sleep(nanoseconds: 180_000_000) }
            try Task.checkCancellation()
            guard inventorySearchTicket == ticket, inventorySearchRequest == request else { return }
            let worker = Task.detached(priority: .userInitiated) {
                try tree.search(query: request.query, kind: request.kind,
                                minimumBytes: request.minimumSize.rawValue, limit: request.limit)
            }
            let result = try await withTaskCancellationHandler {
                try await worker.value
            } onCancel: {
                worker.cancel()
            }
            guard !Task.isCancelled, inventorySearchTicket == ticket,
                  inventorySearchRequest == request else { return }
            inventorySearchPage = result
        } catch is CancellationError {
            // A superseded query never publishes into the current generation.
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func openSearchResult(_ item: AppScanItem, containingFolder: Bool = false) {
        guard !busy, let tree = loadedTree else { return }
        let target = item.isDir && !containingFolder ? item.path
            : URL(fileURLWithPath: item.path).deletingLastPathComponent().path
        guard tree.containsDirectory(target) else { return }
        clearInventorySearch()
        navigate(to: target)
        if containingFolder || !item.isDir { selectedItem = item }
    }
}
