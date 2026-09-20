import Foundation

/// Pure state used to decide whether an asynchronous scan callback still owns
/// the visible result. Keeping this rule outside `AppModel` makes the stale
/// result and cancellation contract directly testable.
struct ScanGenerationOwnership: Equatable {
    struct Ticket: Equatable {
        let generation: UInt64
        let jobID: UUID
    }

    private(set) var generation: UInt64 = 0
    private(set) var activeJobID: UUID?

    mutating func begin(jobID: UUID = UUID()) -> Ticket {
        generation &+= 1
        activeJobID = jobID
        return Ticket(generation: generation, jobID: jobID)
    }

    mutating func cancel() {
        generation &+= 1
        activeJobID = nil
    }

    mutating func complete(_ ticket: Ticket) -> Bool {
        guard accepts(ticket) else { return false }
        activeJobID = nil
        return true
    }

    func accepts(_ ticket: Ticket) -> Bool {
        activeJobID == ticket.jobID && generation == ticket.generation
    }
}

enum CleanupSelectionRules {
    static func toggling(
        path: String,
        isSelectable: Bool,
        in selection: Set<String>
    ) -> Set<String> {
        guard isSelectable else { return selection }
        var result = selection
        if result.remove(path) == nil {
            result.insert(path)
        }
        return result
    }

    static func safeSelectablePaths(in items: [AppScanItem]) -> Set<String> {
        Set(items.lazy
            .filter { $0.safety == .safe && $0.canMoveToTrash }
            .map(\.path))
    }

    static func selectableRecipePaths(in recipe: CleanupRecipe) -> Set<String> {
        Set(recipe.items.lazy
            .filter { $0.removable && $0.appEligible && $0.path != "/dev/null" }
            .map(\.path))
    }

    static func retainingEligible(
        _ selection: Set<String>,
        outcomes: [AppTrashOutcome]
    ) -> Set<String> {
        let eligible = Set(outcomes.lazy
            .filter { $0.error == nil }
            .map(\.path))
        return selection.intersection(eligible)
    }
}

enum NavigationDecision: Equatable {
    case selectFile(path: String)
    case scanDirectory(path: String)
    case stay
}

enum NavigationRules {
    static func opening(path: String, isDirectory: Bool) -> NavigationDecision {
        isDirectory ? .scanDirectory(path: path) : .selectFile(path: path)
    }

    static func parent(of path: String) -> NavigationDecision {
        let parent = URL(fileURLWithPath: path).deletingLastPathComponent().path
        guard parent != path, !parent.isEmpty else { return .stay }
        return .scanDirectory(path: parent)
    }

    static func navigating(to path: String) -> NavigationDecision {
        path.isEmpty ? .stay : .scanDirectory(path: path)
    }
}
