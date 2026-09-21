import AppKit
import SwiftUI

@main
struct MacCleanApp: App {
    var body: some Scene {
        WindowGroup {
            RootView()
                .frame(minWidth: 1100, minHeight: 720)
        }
        .windowStyle(.titleBar)
    }
}

@MainActor
final class AppModel: ObservableObject {
    @Published var scan: AppScan?
    @Published var selectedTab: AppTab = .dashboard
    @Published var selectedItem: AppScanItem?
    @Published var path = FileManager.default.homeDirectoryForCurrentUser.path
    @Published var isScanning = false
    @Published var errorMessage: String?
    @Published var includeSystemData = false
    @Published var includeHealth = false
    @Published var selectedCleanupPaths = Set<String>()
    @Published var isCleaning = false
    @Published var cleanupMessage: String?
    @Published var cleanupOutcomes: [AppTrashOutcome] = []
    @Published var history: [HistorySession] = []
    @Published var isLoadingHistory = false
    @Published var isRestoring = false
    @Published var restoreMessage: String?
    @Published var scanStartedAt: Date?
    @Published var scanElapsedSeconds = 0
    @Published var scanStage = "Idle"
    @Published var scanProgress = 0.0
    @Published var recipes: [CleanupRecipe] = []
    @Published var isLoadingRecipes = false
    @Published var selectedRecipe: CleanupRecipe?
    @Published var streamingItems: [AppScanItem] = []
    @Published var fullDiskAccessStatus: FullDiskAccessStatus = .checking
    @Published var accessCheckedLocations: [String] = []
    @Published var accessDeniedLocations: [String] = []
    @Published var installedApplications: [InstalledApplication] = []
    @Published var selectedInstalledApplication: InstalledApplication?
    @Published var isLoadingInstalledApplications = false
    @Published var uninstallListError: String?
    @Published var uninstallPlan: UninstallPlan?
    @Published var isLoadingUninstallPlan = false
    @Published var uninstallPlanError: String?
    @Published var duplicateReport: DuplicateReport?
    @Published var isScanningDuplicates = false
    @Published var duplicateError: String?
    @Published var duplicateMinMB: UInt64 = 10
    @Published var duplicateProgress: DuplicateScanProgress?
    @Published var duplicateSelections: [String: DuplicateGroupSelection] = [:]
    @Published var duplicateCleanupMessage: String?
    @Published var isCleaningDuplicates = false
    @Published var duplicateCleanupOutcomes: [DuplicateCleanupItemOutcome] = []

    private let service = MacCleanService()
    private var scanTask: Task<Void, Never>?
    private var progressTask: Task<Void, Never>?
    private var scanOwnership = ScanGenerationOwnership()
    private var duplicateTask: Task<Void, Never>?
    private var duplicateGeneration: UInt64 = 0
    private var duplicateJobID: UUID?
    private var cleanupReviewTokens: [String: String] = [:]
    private var duplicateReviewTokens: [String: String] = [:]
    private var activeScanIsDeep = false
    private var activeScanDepth = 0
    private var activeScanLimit = 0

    func loadStartupData() {
        // Access is informational until the user chooses to configure it from
        // the Access screen. Development builds are re-signed frequently, so
        // repeatedly presenting this guidance is noisy and not actionable.
        checkFullDiskAccess()
        loadCachedScan()
        loadRecipes()
    }

    func checkFullDiskAccess() {
        fullDiskAccessStatus = .checking
        Task {
            let result = await Task.detached(priority: .utility) {
                FullDiskAccessChecker.check()
            }.value
            fullDiskAccessStatus = result.status
            accessCheckedLocations = result.checkedLocations
            accessDeniedLocations = result.deniedLocations
        }
    }

    func openFullDiskAccessSettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles") else { return }
        NSWorkspace.shared.open(url)
    }

    func loadInstalledApplications() {
        guard !isLoadingInstalledApplications else { return }
        isLoadingInstalledApplications = true
        uninstallListError = nil
        Task {
            do {
                let apps = try await service.installedApplications()
                installedApplications = apps
                if let selectedInstalledApplication,
                   !apps.contains(where: { $0.path == selectedInstalledApplication.path }) {
                    self.selectedInstalledApplication = nil
                }
            } catch {
                uninstallListError = error.localizedDescription
            }
            isLoadingInstalledApplications = false
        }
    }

    func selectInstalledApplication(_ app: InstalledApplication?) {
        guard selectedInstalledApplication?.path != app?.path else { return }
        selectedInstalledApplication = app
        uninstallPlan = nil
        uninstallPlanError = nil
    }

    func loadUninstallPlan(deep: Bool) {
        guard let app = selectedInstalledApplication, !app.protected, !isLoadingUninstallPlan else { return }
        isLoadingUninstallPlan = true
        uninstallPlan = nil
        uninstallPlanError = nil
        let requestedPath = app.path
        Task {
            do {
                let plan = try await service.uninstallPlan(path: requestedPath, deep: deep)
                if selectedInstalledApplication?.path == requestedPath {
                    uninstallPlan = plan
                }
            } catch {
                if selectedInstalledApplication?.path == requestedPath {
                    uninstallPlanError = error.localizedDescription
                }
            }
            isLoadingUninstallPlan = false
        }
    }

    func clearUninstallPlan() {
        uninstallPlan = nil
        uninstallPlanError = nil
    }

    func loadCachedScan() {
        let generation = scanOwnership.generation
        Task {
            do {
                if let cached = try await service.cachedScan() {
                    guard scanOwnership.generation == generation,
                          !isScanning,
                          scanOwnership.activeJobID == nil else { return }
                    scan = cached
                    selectedItem = cached.largestItems.first
                }
            } catch {
                if scanOwnership.generation == generation {
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    func loadRecipes() {
        if isLoadingRecipes {
            return
        }
        isLoadingRecipes = true
        Task {
            do {
                let response = try await service.recipes()
                recipes = response.recipes
            } catch {
                errorMessage = error.localizedDescription
            }
            isLoadingRecipes = false
        }
    }

    func refresh(fast: Bool = true) {
        if let activeJobID = scanOwnership.activeJobID {
            service.cancelScan(jobID: activeJobID)
            scanTask?.cancel()
        }
        let ticket = scanOwnership.begin()
        let jobID = ticket.jobID
        isScanning = true
        errorMessage = nil
        scanStartedAt = Date()
        scanElapsedSeconds = 0
        scanProgress = 0.04
        streamingItems = []
        activeScanIsDeep = !fast
        activeScanDepth = fast ? 2 : 4
        activeScanLimit = fast ? 40 : 80
        scanStage = fast ? "Fast scan starting" : "Deep scan starting"
        progressTask?.cancel()
        progressTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 500_000_000)
                await MainActor.run {
                    self?.tickScanProgress()
                }
            }
        }
        let requestedPath = path
        let requestedDepth = activeScanDepth
        let requestedLimit = activeScanLimit
        scanTask = Task { [self] in
            do {
                let result = try await service.appScan(
                    path: requestedPath,
                    depth: requestedDepth,
                    limit: requestedLimit,
                    deep: !fast,
                    includeSystemData: includeSystemData,
                    includeHealth: includeHealth,
                    jobID: jobID,
                    onProgress: { [weak self] progress in
                        Task { @MainActor [weak self] in
                            guard self?.scanOwnership.accepts(ticket) == true else { return }
                            self?.apply(progress)
                        }
                    }
                )
                guard scanOwnership.accepts(ticket) else { return }
                scan = result
                selectedItem = result.largestItems.first
                selectedCleanupPaths = selectedCleanupPaths.intersection(Set(result.cleanupCandidates.map(\.path)))
                scanStage = result.rootScan.partial ? "Complete (partial)" : "Complete"
                scanProgress = 1.0
            } catch {
                if scanOwnership.accepts(ticket), !Task.isCancelled {
                    errorMessage = error.localizedDescription
                    scanStage = "Failed"
                }
            }
            guard scanOwnership.complete(ticket) else { return }
            progressTask?.cancel()
            progressTask = nil
            scanTask = nil
            isScanning = false
        }
    }

    func cancelScan() {
        guard let jobID = scanOwnership.activeJobID else { return }
        service.cancelScan(jobID: jobID)
        scanOwnership.cancel()
        scanTask?.cancel()
        progressTask?.cancel()
        scanTask = nil
        progressTask = nil
        isScanning = false
        scanStage = "Cancelled"
        streamingItems = []
    }

    func scanDuplicates(preservingCleanupResults: Bool = false) {
        duplicateTask?.cancel()
        duplicateGeneration &+= 1
        duplicateReviewTokens.removeAll()
        duplicateSelections.removeAll()
        if !preservingCleanupResults {
            duplicateCleanupOutcomes.removeAll()
        }
        let generation = duplicateGeneration
        isScanningDuplicates = true
        duplicateError = nil
        duplicateReport = nil
        duplicateProgress = nil
        let requestedPath = path
        let requestedMinimum = duplicateMinMB
        let jobID = UUID()
        duplicateJobID = jobID
        duplicateTask = Task { [self] in
            defer {
                if duplicateGeneration == generation {
                    isScanningDuplicates = false
                    duplicateJobID = nil
                    duplicateTask = nil
                }
            }
            do {
                let report = try await service.duplicateScan(
                    path: requestedPath,
                    minMB: requestedMinimum,
                    jobID: jobID,
                    onProgress: { [weak self] progress in
                        Task { @MainActor [weak self] in
                            guard self?.duplicateGeneration == generation,
                                  self?.duplicateJobID == jobID else { return }
                            self?.duplicateProgress = progress
                        }
                    }
                )
                guard !Task.isCancelled, duplicateGeneration == generation else { return }
                duplicateReport = report
                duplicateSelections = report.groups.reduce(into: [:]) { selections, group in
                    if let selection = try? DuplicateGroupSelection(
                        group: group,
                        strategy: .newest
                    ) {
                        selections[group.id] = selection
                    }
                }
            } catch is CancellationError {
                // Cancellation is an expected user action.
            } catch {
                guard !Task.isCancelled, duplicateGeneration == generation else { return }
                duplicateError = error.localizedDescription
            }
        }
    }

    func cancelDuplicateScan() {
        duplicateGeneration &+= 1
        if let duplicateJobID {
            service.cancelScan(jobID: duplicateJobID)
        }
        duplicateJobID = nil
        duplicateTask?.cancel()
        duplicateTask = nil
        isScanningDuplicates = false
    }

    func setDuplicateStrategy(_ strategy: DuplicateKeepStrategy, for group: DuplicateGroup) {
        duplicateReviewTokens.removeAll()
        duplicateCleanupOutcomes.removeAll()
        let manualKeeper = strategy == .manual
            ? duplicateSelections[group.id]?.keeperPath
            : nil
        guard let selection = try? DuplicateGroupSelection(
            group: group,
            strategy: strategy,
            manualKeeperPath: manualKeeper
        ) else { return }
        duplicateSelections[group.id] = selection
        duplicateCleanupMessage = nil
    }

    func keepDuplicate(path: String, in group: DuplicateGroup) {
        duplicateReviewTokens.removeAll()
        duplicateCleanupOutcomes.removeAll()
        guard let selection = try? DuplicateGroupSelection(
            group: group,
            strategy: .manual,
            manualKeeperPath: path
        ) else { return }
        duplicateSelections[group.id] = selection
        duplicateCleanupMessage = nil
    }

    func toggleDuplicateDeletion(path: String, in group: DuplicateGroup) {
        duplicateReviewTokens.removeAll()
        duplicateCleanupOutcomes.removeAll()
        guard var selection = duplicateSelections[group.id],
              selection.isCurrent(for: group) else { return }
        _ = selection.toggleDeletion(path: path)
        duplicateSelections[group.id] = selection
        duplicateCleanupMessage = nil
    }

    func selectDuplicateCopies(in group: DuplicateGroup) {
        duplicateReviewTokens.removeAll()
        duplicateCleanupOutcomes.removeAll()
        guard var selection = duplicateSelections[group.id],
              selection.isCurrent(for: group) else { return }
        selection.selectAllDeletable()
        duplicateSelections[group.id] = selection
        duplicateCleanupMessage = nil
    }

    func clearDuplicateCopies(in group: DuplicateGroup) {
        duplicateReviewTokens.removeAll()
        duplicateCleanupOutcomes.removeAll()
        guard var selection = duplicateSelections[group.id] else { return }
        selection.clearSelection()
        duplicateSelections[group.id] = selection
        duplicateCleanupMessage = nil
    }

    var selectedDuplicateCount: Int {
        duplicateSelections.values.reduce(0) {
            $0 + $1.selectedDeletionPaths.count
        }
    }

    var selectedDuplicateBytes: UInt64 {
        guard let duplicateReport else { return 0 }
        let sizes = Dictionary(uniqueKeysWithValues: duplicateReport.groups.flatMap {
            $0.files.map { ($0.path, $0.sizeBytes) }
        })
        return duplicateSelections.values
            .flatMap(\.selectedDeletionPaths)
            .reduce(0) { total, path in
                let size = sizes[path] ?? 0
                return total > UInt64.max - size ? UInt64.max : total + size
            }
    }

    private func duplicateCleanupRequest() throws -> DuplicateCleanupRequest {
        guard let duplicateReport else {
            throw DuplicateSelectionError.groupMembershipChanged(groupID: "scan")
        }
        return try DuplicateCleanupRequest.make(
            selections: Array(duplicateSelections.values),
            currentGroups: duplicateReport.groups
        )
    }

    func preflightDuplicateCleanup(_ completion: @escaping (Bool) -> Void) {
        guard selectedDuplicateCount > 0, !isCleaningDuplicates else {
            completion(false)
            return
        }
        let request: DuplicateCleanupRequest
        do {
            request = try duplicateCleanupRequest()
        } catch {
            duplicateCleanupMessage = "Duplicate selection changed; rescan before cleanup."
            completion(false)
            return
        }

        isCleaningDuplicates = true
        duplicateCleanupMessage = nil
        duplicateCleanupOutcomes = []
        Task {
            defer { isCleaningDuplicates = false }
            do {
                let response = try await service.duplicateCleanup(
                    request: request,
                    dryRun: true
                )
                duplicateCleanupOutcomes = response.outcomes
                duplicateReviewTokens = Dictionary(
                    uniqueKeysWithValues: response.outcomes.compactMap {
                        guard $0.error == nil, let token = $0.reviewToken else { return nil }
                        return ($0.path, token)
                    }
                )
                let requestedCount = request.groups.reduce(0) { $0 + $1.deletePaths.count }
                guard requestedCount > 0,
                      duplicateReviewTokens.count == requestedCount,
                      response.failedCount == 0 else {
                    duplicateReviewTokens.removeAll()
                    duplicateCleanupMessage = response.groups
                        .compactMap(\.error)
                        .first
                        ?? response.outcomes.compactMap(\.error).first
                        ?? "Duplicate cleanup review failed; rescan and try again."
                    completion(false)
                    return
                }
                duplicateCleanupMessage = "Review passed for \(requestedCount) duplicate copy/copies."
                completion(true)
            } catch {
                duplicateReviewTokens.removeAll()
                duplicateCleanupMessage = error.localizedDescription
                completion(false)
            }
        }
    }

    func cleanSelectedDuplicates() {
        guard selectedDuplicateCount > 0, !isCleaningDuplicates else { return }
        let request: DuplicateCleanupRequest
        do {
            request = try duplicateCleanupRequest()
        } catch {
            duplicateCleanupMessage = "Duplicate selection changed; rescan before cleanup."
            return
        }
        let requestedCount = request.groups.reduce(0) { $0 + $1.deletePaths.count }
        guard duplicateReviewTokens.count == requestedCount else {
            duplicateCleanupMessage = "Duplicate review expired. Review selected copies again."
            return
        }

        isCleaningDuplicates = true
        duplicateCleanupMessage = nil
        duplicateCleanupOutcomes = []
        Task {
            defer { isCleaningDuplicates = false }
            do {
                let response = try await service.duplicateCleanup(
                    request: request,
                    reviewTokens: duplicateReviewTokens,
                    dryRun: false
                )
                duplicateCleanupOutcomes = response.outcomes
                duplicateReviewTokens.removeAll()
                let session = response.sessionId.map { " Session: \($0)." } ?? ""
                let receiptWarning = response.receiptError.map {
                    " Receipt warning: \($0) Use Finder’s Trash for recovery."
                } ?? ""
                duplicateCleanupMessage =
                    "Moved \(response.movedCount) duplicate copy/copies (\(formatBytes(response.movedBytes))) to Trash; \(response.failedCount) failed.\(session)\(receiptWarning)"
                loadHistory()
                scanDuplicates(preservingCleanupResults: true)
            } catch {
                duplicateReviewTokens.removeAll()
                duplicateCleanupMessage = error.localizedDescription
            }
        }
    }

    private func tickScanProgress() {
        guard let scanStartedAt, isScanning else {
            return
        }
        scanElapsedSeconds = Int(Date().timeIntervalSince(scanStartedAt))
    }

    private func apply(_ progress: AppScanProgress) {
        guard isScanning else { return }
        scanStage = progress.stage
        scanProgress = progress.progress
        if let item = progress.item {
            streamingItems.removeAll { $0.path == item.path }
            streamingItems.append(item)
            streamingItems.sort { $0.sizeBytes > $1.sizeBytes }
        }
    }

    func mapItems(in scan: AppScan) -> [AppScanItem] {
        isScanning && !streamingItems.isEmpty ? streamingItems : scan.largestItems
    }

    var displayScan: AppScan? {
        guard isScanning, !streamingItems.isEmpty else { return scan }
        let totalBytes = streamingItems.reduce(UInt64(0)) { total, item in
            total > UInt64.max - item.sizeBytes ? UInt64.max : total + item.sizeBytes
        }
        let root = StorageNode(
            name: URL(fileURLWithPath: path).lastPathComponent.isEmpty ? path : URL(fileURLWithPath: path).lastPathComponent,
            path: path,
            sizeBytes: totalBytes,
            percentOfParent: 100,
            isDir: true,
            partial: true,
            safety: .unknown,
            cleanKind: "unknown",
            cleanupAction: "",
            cleanupReason: "Live scan results are still being collected.",
            children: []
        )
        return AppScan(
            schemaVersion: 1,
            rootScan: StorageScan(
                root: path,
                mode: activeScanIsDeep ? "deep" : "fast",
                depth: activeScanDepth,
                limit: activeScanLimit,
                scannedAt: UInt64(Date().timeIntervalSince1970),
                elapsedMs: UInt64(scanElapsedSeconds * 1_000),
                partial: true,
                incompleteReason: "Live results are incomplete until the scan finishes.",
                metrics: nil,
                tree: root
            ),
            systemData: nil,
            largestItems: streamingItems,
            cleanupCandidates: [],
            safetyTotals: [],
            health: nil
        )
    }

    func open(_ item: AppScanItem) {
        switch NavigationRules.opening(path: item.path, isDirectory: item.isDir) {
        case .selectFile:
            selectedItem = item
        case .scanDirectory(let target):
            beginNavigation(to: target)
        case .stay:
            break
        }
    }

    func openInMap(_ item: AppScanItem) {
        selectedTab = .map
        open(item)
    }

    func goUp() {
        if case .scanDirectory(let target) = NavigationRules.parent(of: path) {
            beginNavigation(to: target)
        }
    }

    func navigate(to newPath: String) {
        if case .scanDirectory(let target) = NavigationRules.navigating(to: newPath) {
            beginNavigation(to: target)
        }
    }

    private func beginNavigation(to target: String) {
        path = target
        scan = nil
        selectedItem = nil
        refresh()
    }

    func selectedCleanupTotal(in scan: AppScan) -> UInt64 {
        selectedCleanupTotal(scanCandidates: scan.cleanupCandidates)
    }

    func selectedCleanupTotalForRecipeOnly() -> UInt64 {
        selectedCleanupTotal(scanCandidates: [])
    }

    private func selectedCleanupTotal(scanCandidates: [AppScanItem]) -> UInt64 {
        var seen = Set<String>()
        let scannedTotal = scanCandidates
            .filter { selectedCleanupPaths.contains($0.path) }
            .reduce(UInt64(0)) { total, item in
                seen.insert(item.path)
                return total + item.sizeBytes
            }
        let recipeTotal = selectedRecipe?.items
            .filter { selectedCleanupPaths.contains($0.path) && !seen.contains($0.path) }
            .reduce(UInt64(0)) { $0 + $1.sizeBytes } ?? 0
        return scannedTotal + recipeTotal
    }

    func cleanupReviewItems(in scan: AppScan) -> [AppScanItem] {
        let recipeItems = recipeCandidateItems().filter { recipeItem in
            !scan.cleanupCandidates.contains { $0.path == recipeItem.path }
        }
        return scan.cleanupCandidates + recipeItems
    }

    func recipeCandidateItems() -> [AppScanItem] {
        guard let selectedRecipe else {
            return []
        }
        return selectedRecipe.items
            .filter { $0.removable && $0.path != "/dev/null" }
            .map { AppScanItem(recipeItem: $0, recipe: selectedRecipe) }
    }

    func toggleCleanup(_ item: AppScanItem) {
        cleanupReviewTokens.removeAll()
        selectedCleanupPaths = CleanupSelectionRules.toggling(
            path: item.path,
            isSelectable: item.canMoveToTrash,
            in: selectedCleanupPaths
        )
    }

    func selectSafeCandidates(in scan: AppScan) {
        cleanupReviewTokens.removeAll()
        selectedCleanupPaths = CleanupSelectionRules.safeSelectablePaths(in: scan.cleanupCandidates)
    }

    func selectSafeCandidates(from items: [AppScanItem]) {
        cleanupReviewTokens.removeAll()
        selectedRecipe = nil
        selectedCleanupPaths = CleanupSelectionRules.safeSelectablePaths(in: items)
    }

    func clearCleanupSelection() {
        cleanupReviewTokens.removeAll()
        selectedCleanupPaths.removeAll()
        selectedRecipe = nil
    }

    func selectRecipePaths(_ recipe: CleanupRecipe) {
        cleanupReviewTokens.removeAll()
        selectedRecipe = recipe
        selectedCleanupPaths = CleanupSelectionRules.selectableRecipePaths(in: recipe)
        selectedItem = recipeCandidateItems().first
        selectedTab = .clean
    }

    func cleanSelected() {
        guard !selectedCleanupPaths.isEmpty, !isCleaning else {
            return
        }
        isCleaning = true
        cleanupMessage = nil
        cleanupOutcomes = []
        let paths = Array(selectedCleanupPaths)
        let tokens = paths.compactMap { cleanupReviewTokens[$0] }
        guard tokens.count == paths.count else {
            isCleaning = false
            cleanupMessage = "Cleanup review expired. Review the selected paths again."
            return
        }
        Task {
            do {
                let response = try await service.trash(paths: paths, reviewTokens: tokens)
                let failureDetail = response.outcomes
                    .compactMap(\.error)
                    .first
                    .map { " \($0)" } ?? ""
                let sessionDetail = response.sessionId.map { " Session: \($0)." } ?? ""
                let receiptDetail = response.receiptError.map { " \($0)" } ?? ""
                cleanupMessage = "Moved \(response.movedCount) item(s) (\(formatBytes(response.movedBytes))) to Trash; \(formatBytes(response.reclaimedBytes)) reclaimed until Trash is emptied. \(response.failedCount) failed.\(sessionDetail)\(failureDetail)\(receiptDetail)"
                cleanupOutcomes = response.outcomes
                selectedCleanupPaths.removeAll()
                cleanupReviewTokens.removeAll()
                selectedRecipe = nil
                loadHistory()
                loadRecipes()
                // The cached scan describes the files before the Trash move.
                // Scan again so successfully removed candidates disappear
                // instead of looking as though cleanup did nothing.
                refresh(fast: !activeScanIsDeep)
            } catch {
                cleanupMessage = error.localizedDescription
            }
            isCleaning = false
        }
    }

    func preflightCleanup(_ completion: @escaping (Bool) -> Void) {
        guard !selectedCleanupPaths.isEmpty, !isCleaning else {
            completion(false)
            return
        }
        isCleaning = true
        cleanupMessage = nil
        cleanupOutcomes = []
        let paths = Array(selectedCleanupPaths)
        Task {
            defer { isCleaning = false }
            do {
                let response = try await service.trash(paths: paths, dryRun: true)
                cleanupReviewTokens = Dictionary(uniqueKeysWithValues: response.outcomes.compactMap {
                    guard $0.error == nil, let token = $0.reviewToken else { return nil }
                    return ($0.path, token)
                })
                selectedCleanupPaths.formIntersection(cleanupReviewTokens.keys)
                let eligiblePaths = selectedCleanupPaths
                let failures = response.outcomes.filter { $0.error != nil }
                cleanupOutcomes = failures
                if eligiblePaths.isEmpty {
                    cleanupMessage = failures.first?.error ?? "No selected paths passed cleanup preflight."
                    completion(false)
                } else {
                    let excluded = failures.count
                    cleanupMessage = excluded == 0
                        ? "Preflight passed for \(eligiblePaths.count) item(s)."
                        : "Preflight excluded \(excluded) item(s). \(failures.first?.error ?? "")"
                    completion(true)
                }
            } catch {
                cleanupMessage = error.localizedDescription
                completion(false)
            }
        }
    }

    func loadHistory() {
        if isLoadingHistory {
            return
        }
        isLoadingHistory = true
        Task {
            do {
                history = try await service.history()
            } catch {
                errorMessage = error.localizedDescription
            }
            isLoadingHistory = false
        }
    }

    func restore(_ session: HistorySession) {
        guard !isRestoring else {
            return
        }
        isRestoring = true
        restoreMessage = nil
        Task {
            do {
                let response = try await service.restore(sessionId: session.sessionId)
                restoreMessage = "Restored \(response.restoredCount) item(s), \(response.failedCount) failed."
                loadHistory()
                loadCachedScan()
            } catch {
                restoreMessage = error.localizedDescription
            }
            isRestoring = false
        }
    }
}

struct RootView: View {
    @StateObject private var model = AppModel()

    var body: some View {
        NavigationSplitView {
            List(AppTab.allCases, selection: $model.selectedTab) { tab in
                Label(tab.title, systemImage: tab.icon)
                    .tag(tab)
            }
            .navigationTitle("macclean")
        } detail: {
            VStack(spacing: 0) {
                ToolbarView(model: model)
                Divider()
                content
            }
        }
        .task {
            model.loadStartupData()
        }
    }

    @ViewBuilder
    private var content: some View {
        switch model.selectedTab {
        case .dashboard:
            DashboardView(model: model)
        case .map:
            MapView(model: model)
        case .duplicates:
            DuplicatesView(model: model)
        case .clean:
            CleanReviewView(model: model)
        case .developer:
            DeveloperView(model: model)
        case .monitor:
            MonitorView(model: model)
        case .history:
            HistoryView(model: model)
        case .uninstall:
            UninstallView(model: model)
        case .access:
            FullDiskAccessView(model: model)
        }
    }
}

enum AppTab: String, CaseIterable, Identifiable {
    case dashboard
    case map
    case duplicates
    case clean
    case developer
    case monitor
    case history
    case uninstall
    case access

    var id: String { rawValue }

    var title: String {
        switch self {
        case .dashboard: "Dashboard"
        case .map: "Map"
        case .duplicates: "Duplicates"
        case .clean: "Clean"
        case .developer: "Developer"
        case .monitor: "Monitor"
        case .history: "History"
        case .uninstall: "Uninstall"
        case .access: "Access"
        }
    }

    var icon: String {
        switch self {
        case .dashboard: "gauge.with.dots.needle.67percent"
        case .map: "square.grid.3x3"
        case .duplicates: "doc.on.doc"
        case .clean: "checklist"
        case .developer: "hammer"
        case .monitor: "waveform.path.ecg"
        case .history: "clock.arrow.circlepath"
        case .uninstall: "app.badge"
        case .access: "lock.shield"
        }
    }
}

struct ToolbarView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        VStack(spacing: 8) {
            HStack(spacing: 12) {
                TextField("Path", text: $model.path)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 420)

                Toggle("System Data", isOn: $model.includeSystemData)
                Toggle("Health", isOn: $model.includeHealth)

                Button {
                    model.refresh()
                } label: {
                    Label("Fast Scan", systemImage: "bolt.fill")
                }
                .buttonStyle(.borderedProminent)
                .disabled(model.isScanning)

                Button {
                    model.refresh(fast: false)
                } label: {
                    Label("Deep Scan", systemImage: "magnifyingglass")
                }
                .disabled(model.isScanning)

                if model.isScanning {
                    Button {
                        model.cancelScan()
                    } label: {
                        Label("Stop", systemImage: "stop.fill")
                    }
                }

                Spacer()
            }

            if model.fullDiskAccessStatus == .denied || model.fullDiskAccessStatus == .unavailable {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.shield")
                        .foregroundStyle(.orange)
                    Text(model.fullDiskAccessStatus.detail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Button("Open Full Disk Access") {
                        model.openFullDiskAccessSettings()
                    }
                    .buttonStyle(.link)
                    Spacer()
                }
            }

            if model.isScanning {
                HStack(spacing: 10) {
                    ProgressView(value: model.scanProgress)
                        .frame(width: 220)
                    Text(model.scanStage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("\(model.scanElapsedSeconds)s")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                    if model.includeSystemData {
                        Text("System Data can slow scans")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }
                    Spacer()
                }
            }
        }
        .padding(12)
    }
}
