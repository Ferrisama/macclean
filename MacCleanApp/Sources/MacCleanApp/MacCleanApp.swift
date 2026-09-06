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
    @Published var selectedItem: AppScanItem?
    @Published var path = FileManager.default.homeDirectoryForCurrentUser.path
    @Published var isScanning = false
    @Published var errorMessage: String?
    @Published var includeSystemData = false
    @Published var includeHealth = false
    @Published var selectedCleanupPaths = Set<String>()
    @Published var isCleaning = false
    @Published var cleanupMessage: String?
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
    @Published var showAccessOnboarding = false
    @Published var installedApplications: [InstalledApplication] = []
    @Published var selectedInstalledApplication: InstalledApplication?
    @Published var isLoadingInstalledApplications = false
    @Published var uninstallListError: String?
    @Published var uninstallPlan: UninstallPlan?
    @Published var isLoadingUninstallPlan = false
    @Published var uninstallPlanError: String?

    private let service = MacCleanService()
    private var scanTask: Task<Void, Never>?
    private var progressTask: Task<Void, Never>?
    private var scanJobID: UUID?
    private var activeScanIsDeep = false
    private var activeScanDepth = 0
    private var activeScanLimit = 0

    func loadStartupData() {
        checkFullDiskAccess(showOnboardingIfNeeded: true)
        loadCachedScan()
        loadRecipes()
    }

    func checkFullDiskAccess(showOnboardingIfNeeded: Bool = false) {
        fullDiskAccessStatus = .checking
        Task {
            let result = await Task.detached(priority: .utility) {
                FullDiskAccessChecker.check()
            }.value
            fullDiskAccessStatus = result.status
            accessCheckedLocations = result.checkedLocations
            accessDeniedLocations = result.deniedLocations
            if showOnboardingIfNeeded,
               result.status != .granted,
               !UserDefaults.standard.bool(forKey: "fullDiskAccessOnboardingSeen") {
                showAccessOnboarding = true
            }
        }
    }

    func openFullDiskAccessSettings() {
        UserDefaults.standard.set(true, forKey: "fullDiskAccessOnboardingSeen")
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles") else { return }
        NSWorkspace.shared.open(url)
    }

    func dismissAccessOnboarding() {
        UserDefaults.standard.set(true, forKey: "fullDiskAccessOnboardingSeen")
        showAccessOnboarding = false
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
        Task {
            do {
                if let cached = try await service.cachedScan() {
                    scan = cached
                    selectedItem = cached.largestItems.first
                }
            } catch {
                errorMessage = error.localizedDescription
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
                            guard self?.scanJobID == jobID else { return }
                            self?.apply(progress)
                        }
                    }
                )
                guard scanJobID == jobID else { return }
                scan = result
                selectedItem = result.largestItems.first
                selectedCleanupPaths = selectedCleanupPaths.intersection(Set(result.cleanupCandidates.map(\.path)))
                scanStage = result.rootScan.partial ? "Complete (partial)" : "Complete"
                scanProgress = 1.0
            } catch {
                if scanJobID == jobID, !Task.isCancelled {
                    errorMessage = error.localizedDescription
                    scanStage = "Failed"
                }
            }
            guard scanJobID == jobID else { return }
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
        scanTask?.cancel()
        progressTask?.cancel()
        scanJobID = nil
        scanTask = nil
        progressTask = nil
        isScanning = false
        scanStage = "Cancelled"
        streamingItems = []
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
        refresh()
    }

    func goUp() {
        let parent = URL(fileURLWithPath: path).deletingLastPathComponent().path
        if parent != path, !parent.isEmpty {
            path = parent
            refresh()
        }
    }

    func navigate(to newPath: String) {
        path = newPath
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
            item.removable && item.path != "/dev/null"
        }.map(\.path))
        selectedItem = recipeCandidateItems().first
    }

    func cleanSelected() {
        guard !selectedCleanupPaths.isEmpty, !isCleaning else {
            return
        }
        isCleaning = true
        cleanupMessage = nil
        let paths = Array(selectedCleanupPaths)
        Task {
            do {
                let response = try await service.trash(paths: paths)
                cleanupMessage = "Moved \(response.movedCount) item(s) to Trash, \(response.failedCount) failed. \(formatBytes(response.totalBytes)) reviewed."
                selectedCleanupPaths.removeAll()
                selectedRecipe = nil
                loadHistory()
                loadRecipes()
                loadCachedScan()
            } catch {
                cleanupMessage = error.localizedDescription
            }
            isCleaning = false
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
    @State private var selection: AppTab = .dashboard
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        NavigationSplitView {
            List(AppTab.allCases, selection: $selection) { tab in
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
        .onChange(of: scenePhase) { newPhase in
            if newPhase == .active {
                model.checkFullDiskAccess()
            }
        }
        .sheet(isPresented: $model.showAccessOnboarding) {
            FullDiskAccessOnboardingView(model: model)
                .interactiveDismissDisabled()
        }
    }

    @ViewBuilder
    private var content: some View {
        switch selection {
        case .dashboard:
            DashboardView(model: model)
        case .map:
            MapView(model: model)
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
