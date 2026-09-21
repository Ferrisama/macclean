import Foundation

enum DuplicateKeepStrategy: String, Codable, CaseIterable {
    case newest
    case oldest
    case shortestPath = "shortest-path"
    case manual
}

enum DuplicateSelectionError: Error, Equatable {
    case emptyGroup(groupID: String)
    case manualKeeperRequired(groupID: String)
    case keeperNotInGroup(groupID: String, path: String)
    case groupMembershipChanged(groupID: String)
}

/// Pure review state for one duplicate group. The member snapshot makes a
/// selection valid only for the exact group that the user reviewed.
struct DuplicateGroupSelection: Equatable {
    let groupID: String
    let strategy: DuplicateKeepStrategy
    let keeperPath: String
    let reviewedMemberPaths: [String]
    private(set) var selectedDeletionPaths: Set<String>

    init(
        group: DuplicateGroup,
        strategy: DuplicateKeepStrategy,
        manualKeeperPath: String? = nil
    ) throws {
        let members = Self.sortedUniquePaths(in: group)
        guard !members.isEmpty else {
            throw DuplicateSelectionError.emptyGroup(groupID: group.id)
        }

        let keeper: String
        switch strategy {
        case .newest:
            keeper = Self.orderedFiles(in: group) {
                if $0.modifiedAt != $1.modifiedAt {
                    return $0.modifiedAt > $1.modifiedAt
                }
                return $0.path < $1.path
            }[0].path
        case .oldest:
            keeper = Self.orderedFiles(in: group) {
                if $0.modifiedAt != $1.modifiedAt {
                    return $0.modifiedAt < $1.modifiedAt
                }
                return $0.path < $1.path
            }[0].path
        case .shortestPath:
            keeper = Self.orderedFiles(in: group) {
                let lhsLength = $0.path.utf8.count
                let rhsLength = $1.path.utf8.count
                if lhsLength != rhsLength { return lhsLength < rhsLength }
                return $0.path < $1.path
            }[0].path
        case .manual:
            guard let manualKeeperPath else {
                throw DuplicateSelectionError.manualKeeperRequired(groupID: group.id)
            }
            guard members.contains(manualKeeperPath) else {
                throw DuplicateSelectionError.keeperNotInGroup(
                    groupID: group.id,
                    path: manualKeeperPath
                )
            }
            keeper = manualKeeperPath
        }

        groupID = group.id
        self.strategy = strategy
        keeperPath = keeper
        reviewedMemberPaths = members
        selectedDeletionPaths = []
    }

    var deletablePaths: Set<String> {
        Set(reviewedMemberPaths.lazy.filter { $0 != keeperPath })
    }

    func isCurrent(for group: DuplicateGroup) -> Bool {
        group.id == groupID && Self.sortedUniquePaths(in: group) == reviewedMemberPaths
    }

    @discardableResult
    mutating func toggleDeletion(path: String) -> Bool {
        guard path != keeperPath, deletablePaths.contains(path) else { return false }
        if selectedDeletionPaths.remove(path) == nil {
            selectedDeletionPaths.insert(path)
        }
        return true
    }

    mutating func selectAllDeletable() {
        selectedDeletionPaths = deletablePaths
    }

    mutating func clearSelection() {
        selectedDeletionPaths.removeAll()
    }

    /// Clears reviewed deletion choices when the scanner reports a different
    /// membership. The caller should then create a fresh state so that the
    /// keeper is explicitly chosen for the new group.
    @discardableResult
    mutating func invalidateIfMembershipChanged(to group: DuplicateGroup) -> Bool {
        guard !isCurrent(for: group) else { return false }
        selectedDeletionPaths.removeAll()
        return true
    }

    private static func sortedUniquePaths(in group: DuplicateGroup) -> [String] {
        Array(Set(group.files.map(\.path))).sorted()
    }

    private static func orderedFiles(
        in group: DuplicateGroup,
        by areInIncreasingOrder: (DuplicateFile, DuplicateFile) -> Bool
    ) -> [DuplicateFile] {
        group.files.sorted(by: areInIncreasingOrder)
    }
}

struct DuplicateCleanupRequest: Codable, Equatable {
    let schemaVersion: Int
    let groups: [DuplicateCleanupGroupRequest]

    static func make(
        selections: [DuplicateGroupSelection],
        currentGroups: [DuplicateGroup]
    ) throws -> DuplicateCleanupRequest {
        let groupsByID = Dictionary(uniqueKeysWithValues: currentGroups.map { ($0.id, $0) })
        let requests = try selections.compactMap { selection -> DuplicateCleanupGroupRequest? in
            guard let group = groupsByID[selection.groupID], selection.isCurrent(for: group) else {
                throw DuplicateSelectionError.groupMembershipChanged(groupID: selection.groupID)
            }

            let deletions = selection.selectedDeletionPaths.sorted()
            guard !deletions.isEmpty else { return nil }
            return DuplicateCleanupGroupRequest(
                groupID: selection.groupID,
                strategy: selection.strategy,
                keeperPath: selection.keeperPath,
                reviewedMemberPaths: selection.reviewedMemberPaths,
                deletePaths: deletions
            )
        }

        return DuplicateCleanupRequest(
            schemaVersion: 1,
            groups: requests.sorted {
                if $0.groupID != $1.groupID { return $0.groupID < $1.groupID }
                return $0.keeperPath < $1.keeperPath
            }
        )
    }
}

struct DuplicateCleanupGroupRequest: Codable, Equatable {
    let groupID: String
    let strategy: DuplicateKeepStrategy
    let keeperPath: String
    let reviewedMemberPaths: [String]
    let deletePaths: [String]
}
