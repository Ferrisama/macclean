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

    private let service = MacCleanService()
    private var scanTask: Task<Void, Never>?
    private var progressTask: Task<Void, Never>?
    private var scanJobID: UUID?
    private var scanGeneration: UInt64 = 0
    private var duplicateTask: Task<Void, Never>?
    private var duplicateGeneration: UInt64 = 0
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
        let generation = scanGeneration
        Task {
            do {
                if let cached = try await service.cachedScan() {
                    guard scanGeneration == generation, !isScanning, scanJobID == nil else { return }
                    scan = cached
                    selectedItem = cached.largestItems.first
                }
            } catch {
                if scanGeneration == generation {
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
        scanGeneration &+= 1
        let generation = scanGeneration
        if let activeJobID = scanJobID {
            service.cancelScan(jobID: activeJobID)
            scanTask?.cancel()
        }
        let jobID = UUID()
        scanJobID = jobID
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
                            guard self?.scanJobID == jobID, self?.scanGeneration == generation else { return }
                            self?.apply(progress)
                        }
                    }
                )
                guard scanJobID == jobID, scanGeneration == generation else { return }
                scan = result
                selectedItem = result.largestItems.first
                selectedCleanupPaths = selectedCleanupPaths.intersection(Set(result.cleanupCandidates.map(\.path)))
                scanStage = result.rootScan.partial ? "Complete (partial)" : "Complete"
                scanProgress = 1.0
            } catch {
                if scanJobID == jobID, scanGeneration == generation, !Task.isCancelled {
                    errorMessage = error.localizedDescription
                    scanStage = "Failed"
                }
            }
            guard scanJobID == jobID, scanGeneration == generation else { return }
            progressTask?.cancel()
            progressTask = nil
            scanTask = nil
            scanJobID = nil
            isScanning = false
        }
    }

    func cancelScan() {
        guard let jobID = scanJobID else { return }
        service.cancelScan(jobID: jobID)
        scanGeneration &+= 1
        scanTask?.cancel()
        progressTask?.cancel()
        scanJobID = nil
        scanTask = nil
        progressTask = nil
        isScanning = false
        scanStage = "Cancelled"
        streamingItems = []
    }

    func scanDuplicates() {
        duplicateTask?.cancel()
        duplicateGeneration &+= 1
        let generation = duplicateGeneration
        isScanningDuplicates = true
        duplicateError = nil
        duplicateReport = nil
        let requestedPath = path
        let requestedMinimum = duplicateMinMB
        duplicateTask = Task { [self] in
            defer {
                if duplicateGeneration == generation {
                    isScanningDuplicates = false
                    duplicateTask = nil
                }
            }
            do {
                let report = try await service.duplicateScan(
                    path: requestedPath,
                    minMB: requestedMinimum
                )
                guard !Task.isCancelled, duplicateGeneration == generation else { return }
                duplicateReport = report
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
        duplicateTask?.cancel()
        duplicateTask = nil
        isScanningDuplicates = false
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
        guard item.isDir else {
            selectedItem = item
            return
        }
        path = item.path
        // Do not leave the old folder's map visible while the new scope is
        // being measured; that made successful navigation look like a no-op.
        scan = nil
        selectedItem = nil
        refresh()
    }

    func openInMap(_ item: AppScanItem) {
        selectedTab = .map
        open(item)
    }

    func goUp() {
        let parent = URL(fileURLWithPath: path).deletingLastPathComponent().path
        if parent != path, !parent.isEmpty {
            path = parent
            scan = nil
            selectedItem = nil
            refresh()
        }
    }

    func navigate(to newPath: String) {
        path = newPath
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
        guard item.canMoveToTrash else { return }
        if selectedCleanupPaths.contains(item.path) {
            selectedCleanupPaths.remove(item.path)
        } else {
            selectedCleanupPaths.insert(item.path)
        }
    }

    func selectSafeCandidates(in scan: AppScan) {
        selectedCleanupPaths = Set(scan.cleanupCandidates.filter { $0.safety == .safe }.map(\.path))
    }

    func selectSafeCandidates(from items: [AppScanItem]) {
        selectedRecipe = nil
        selectedCleanupPaths = Set(items.filter { $0.safety == .safe }.map(\.path))
    }

    func clearCleanupSelection() {
        selectedCleanupPaths.removeAll()
        selectedRecipe = nil
    }

    func selectRecipePaths(_ recipe: CleanupRecipe) {
        selectedRecipe = recipe
        selectedCleanupPaths = Set(recipe.items.filter { item in
            item.appEligible && item.path != "/dev/null"
        }.map(\.path))
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
        Task {
            do {
                let response = try await service.trash(paths: paths)
                let failureDetail = response.outcomes
                    .compactMap(\.error)
                    .first
                    .map { " \($0)" } ?? ""
                let sessionDetail = response.sessionId.map { " Session: \($0)." } ?? ""
                let receiptDetail = response.receiptError.map { " \($0)" } ?? ""
                cleanupMessage = "Moved \(response.movedCount) item(s) (\(formatBytes(response.movedBytes))) to Trash; \(formatBytes(response.reclaimedBytes)) reclaimed until Trash is emptied. \(response.failedCount) failed.\(sessionDetail)\(failureDetail)\(receiptDetail)"
                cleanupOutcomes = response.outcomes
                selectedCleanupPaths.removeAll()
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
                let eligiblePaths = Set(response.outcomes
                    .filter { $0.error == nil }
                    .map(\.path))
                selectedCleanupPaths.formIntersection(eligiblePaths)
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
