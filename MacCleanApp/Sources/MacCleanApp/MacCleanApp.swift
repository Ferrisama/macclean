import AppKit
import SwiftUI

@main
struct MacCleanApp: App {
    var body: some Scene {
        WindowGroup {
            RootView()
                .frame(minWidth: 860, minHeight: 600)
        }
        .windowStyle(.hiddenTitleBar)
    }
}

@MainActor
final class AppModel: ObservableObject {
    private static let initialPath = FileManager.default.homeDirectoryForCurrentUser.path

    @Published private(set) var loadedTree: LoadedTree?
    @Published var inventorySearchQuery = ""
    @Published var inventorySearchKind: InventorySearchKind = .all
    @Published var inventorySearchSize: InventorySearchSize = .any
    @Published var inventorySearchLimit = 100
    @Published var inventorySearchPage: InventorySearchPage?
    @Published var isSearchingInventory = false
    var inventorySearchTicket = UUID()
    @Published var volumeCapacity: VolumeCapacity?
    @Published var volumeCapacityError: String?
    @Published var isLoadingVolumeCapacity = false
    var volumeCapacityTicket = UUID()
    var volumeCapacitySourcePath: String?
    let capacityReader: @Sendable (String) throws -> VolumeCapacity

    init(capacityReader: @escaping @Sendable (String) throws -> VolumeCapacity = { try VolumeCapacity.read(at: $0) }) {
        self.capacityReader = capacityReader
    }
    @Published var scan: AppScan?
    @Published var selectedTab: AppTab = .map
    @Published var selectedItem: AppScanItem?
    @Published var path = AppModel.initialPath
    @Published private(set) var directoryNavigation = DirectoryNavigation(
        rootPath: AppModel.initialPath
    )
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
    @Published var isFindingApps = false
    @Published var folderActivity: [ApplicationActivity] = []
    @Published var appActivity: [ApplicationActivity] = []
    @Published var cacheItems: [AppScanItem] = []
    @Published var cacheWarnings: [String] = []
    @Published var isLoadingRecipes = false
    @Published var selectedRecipe: CleanupRecipe?
    @Published var streamingItems: [AppScanItem] = []
    @Published var fullDiskAccessStatus: FullDiskAccessStatus = .checking
    @Published var accessCheckedLocations: [String] = []
    @Published var accessDeniedLocations: [String] = []
    @Published var recentFolders: [RecentFolder] = RecentFolder.load()
    @Published var collectedItems: [String: AppScanItem] = [:]

    var busy: Bool { isScanning || isLoadingRecipes || isCleaning || isRestoring || isFindingApps }
    var collection: [AppScanItem] {
        selectedCleanupPaths.compactMap { collectedItems[$0] }.sorted { $0.path < $1.path }
    }
    var collectionBytes: UInt64 { collection.reduce(0) { $0 &+ $1.sizeBytes } }
    var opportunities: [AppScanItem] {
        // The immutable scan catalog is indexed/sorted off the main actor once.
        if cacheItems.isEmpty, recipes.isEmpty, let loadedTree { return loadedTree.cleanupItems }
        var byPath = Dictionary((scan?.cleanupCandidates ?? []).map { ($0.path, $0) }, uniquingKeysWith: { a, _ in a })
        for item in cacheItems { byPath[item.path] = item }
        for recipe in recipes {
            for item in recipe.items where item.removable && item.appEligible {
                byPath[item.path] = AppScanItem(recipeItem: item, recipe: recipe)
            }
        }
        return byPath.values.sorted { $0.sizeBytes == $1.sizeBytes ? $0.path < $1.path : $0.sizeBytes > $1.sizeBytes }
    }

    func rememberRecentFolder(_ path: String) {
        recentFolders = RecentFolder.recording(path, in: recentFolders)
        RecentFolder.save(recentFolders)
    }

    func openRecentFolder(_ folder: RecentFolder) {
        guard !busy else { return }
        path = folder.path
        startRootScan()
    }

    func clearRecentFolders() {
        recentFolders = []
        RecentFolder.save([])
    }

    func loadAppActivity() async {
        guard !busy else { return }
        isFindingApps = true
        defer { isFindingApps = false }
        appActivity = await service.applicationActivity()
    }

    func loadFolderActivity() async {
        guard !busy, let scan else { return }
        isFindingApps = true
        defer { isFindingApps = false }
        folderActivity = await service.folderActivity(paths: mapItems(in: scan).filter(\.isDir).map(\.path))
    }

    func inspectUsage(_ item: AppScanItem) async -> ItemUsage {
        await service.usage(path: item.path)
    }

    private let service = MacCleanService()
    private var scanTask: Task<Void, Never>?
    private var progressTask: Task<Void, Never>?
    private var cacheTask: Task<Void, Never>?
    private var scanOwnership = ScanGenerationOwnership()
    private var cleanupReviewTokens: [String: String] = [:]
    private var cleanupProjectRoots: [String] = []
    private var activeScanIsDeep = false
    private var activeScanDepth = 0
    private var activeScanLimit = 0
    private var scansByPath: [String: AppScan] = [:]
    private var scanOrder: [String] = []

    private func rememberScan(_ result: AppScan) {
        let key = result.rootScan.root
        scansByPath[key] = result
        scanOrder.removeAll { $0 == key }
        scanOrder.append(key)
        if scanOrder.count > 8 {
            scansByPath.removeValue(forKey: scanOrder.removeFirst())
        }
    }

    private func forgetScans() {
        scansByPath.removeAll()
        scanOrder.removeAll()
    }

    func loadStartupData() {
        // Access is informational until the user chooses to configure it from
        // the Access screen. Development builds are re-signed frequently, so
        // repeatedly presenting this guidance is noisy and not actionable.
        checkFullDiskAccess()
        loadCachedScan()
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

    func chooseScanFolder() {
        let panel = NSOpenPanel()
        panel.title = "Choose a folder to scan"
        panel.prompt = "Choose"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = false
        panel.directoryURL = URL(fileURLWithPath: path, isDirectory: true)
        guard panel.runModal() == .OK, let selectedURL = panel.url else { return }
        path = selectedURL.standardizedFileURL.path
        startRootScan()
    }

    func revealCurrentPath() {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    func loadCachedScan() {
        let generation = scanOwnership.generation
        Task {
            do {
                if let cached = try await service.cachedScan() {
                    guard scanOwnership.generation == generation,
                          !busy, cacheItems.isEmpty, selectedCleanupPaths.isEmpty,
                          scanOwnership.activeJobID == nil else { return }
                    let index = await Self.indexTree(cached)
                    guard scanOwnership.generation == generation, !busy, cacheItems.isEmpty, selectedCleanupPaths.isEmpty else { return }
                    loadedTree = index
                    scan = cached
                    rememberScan(cached)
                    path = cached.rootScan.root
                    directoryNavigation.reset(rootPath: cached.rootScan.root)
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
        guard !busy else { return }
        isLoadingRecipes = true
        errorMessage = nil
        cleanupReviewTokens.removeAll()
        cacheTask = Task {
            defer { isLoadingRecipes = false; cacheTask = nil }
            do {
                let response = try await service.cacheCatalog()
                guard !Task.isCancelled else { return }
                cacheItems = response.items
                cacheWarnings = response.warnings
                selectedItem = response.items.first
            } catch {
                if !Task.isCancelled { errorMessage = error.localizedDescription }
            }
        }
    }

    func startRootScan(fast: Bool = false) {
        guard !busy else { return }
        var isDirectory: ObjCBool = false
        let requestedRoot = URL(fileURLWithPath: path).standardizedFileURL.path
        guard FileManager.default.fileExists(atPath: requestedRoot, isDirectory: &isDirectory),
              isDirectory.boolValue else {
            errorMessage = "Choose an existing folder before scanning."
            return
        }
        clearCleanupSelection()
        folderActivity = []
        loadedTree = nil
        scan = nil
        path = requestedRoot
        directoryNavigation.reset(rootPath: requestedRoot)
        forgetScans()
        refresh(fast: fast)
    }

    func refresh(
        fast: Bool = false,
        rollbackNavigation: DirectoryNavigation? = nil
    ) {
        guard !isCleaning, !isRestoring, !isLoadingRecipes else { return }
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
        cleanupReviewTokens.removeAll()
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
                scanStage = "Loading complete folder tree…"
                let index = await Self.indexTree(result)
                guard scanOwnership.accepts(ticket), !Task.isCancelled else { return }
                loadedTree = index
                scan = result
                if path != result.rootScan.root {
                    path = result.rootScan.root
                    directoryNavigation.reset(rootPath: path)
                }
                rememberScan(result)
                selectedItem = index?.children(of: path)?.first ?? result.largestItems.first
                rememberRecentFolder(requestedPath)
                scanStage = result.rootScan.partial ? "Complete (partial)" : "Complete"
                scanProgress = 1.0
            } catch {
                if scanOwnership.accepts(ticket), !Task.isCancelled {
                    if let rollbackNavigation {
                        directoryNavigation = rollbackNavigation
                        path = rollbackNavigation.currentPath
                    }
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
        let pending = scanTask
        service.cancelScan(jobID: jobID)
        scanOwnership.cancel()
        let cancelledGeneration = scanOwnership.generation
        pending?.cancel()
        progressTask?.cancel()
        scanStage = "Stopping…"
        Task {
            await pending?.value
            guard scanOwnership.generation == cancelledGeneration else { return }
            scanTask = nil
            progressTask = nil
            isScanning = false
            scanStage = "Cancelled"
            streamingItems = []
        }
    }

    func cancelCacheLookup() { cacheTask?.cancel() }

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

    private nonisolated static func indexTree(_ scan: AppScan) async -> LoadedTree? {
        guard let inventory = scan.rootScan.inventory else { return nil }
        return await Task.detached(priority: .userInitiated) {
            LoadedTree(inventory: inventory, candidates: scan.cleanupCandidates)
        }.value
    }

    /// Useful for fixture loading and immutable inventory installation.
    func installLoadedScan(_ scan: AppScan) async {
        loadedTree = await Self.indexTree(scan)
        self.scan = scan
        path = scan.rootScan.root
        directoryNavigation.reset(rootPath: path)
        selectedItem = loadedTree?.children(of: path)?.first ?? scan.largestItems.first
    }

    var currentFolder: AppScanItem? {
        if let item = loadedTree?.folder(path) { return item }
        guard let root = displayScan?.rootScan.tree else { return nil }
        return AppScanItem(name: root.name, path: root.path, sizeBytes: root.sizeBytes, percentOfRoot: 100,
                           isDir: root.isDir, partial: root.partial, safety: root.safety, cleanKind: root.cleanKind,
                           cleanupAction: root.cleanupAction, cleanupReason: root.cleanupReason,
                           logicalSizeBytes: root.logicalSizeBytes, hardLinkDuplicate: root.hardLinkDuplicate)
    }

    func mapItems(in scan: AppScan) -> [AppScanItem] {
        if isScanning, !streamingItems.isEmpty { return streamingItems }
        return loadedTree?.children(of: path) ?? scan.largestItems
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
        guard !directoryNavigation.isAtRoot else { return }
        transitionDirectory { $0.goUp() }
    }

    func goBack() {
        transitionDirectory { $0.goBack() }
    }

    func goForward() {
        transitionDirectory { $0.goForward() }
    }

    func goToScanRoot() {
        transitionDirectory { $0.goToRoot() }
    }

    func navigate(to newPath: String) {
        if case .scanDirectory(let target) = NavigationRules.navigating(to: newPath) {
            beginNavigation(to: target)
        }
    }

    private func beginNavigation(to target: String) {
        transitionDirectory { $0.open(target) }
    }

    private func transitionDirectory(
        _ transition: (inout DirectoryNavigation) -> String?
    ) {
        guard !busy else { return }
        cleanupReviewTokens.removeAll()
        let previous = directoryNavigation
        var next = directoryNavigation
        guard let target = transition(&next) else { return }
        directoryNavigation = next
        path = target
        if let loadedTree, loadedTree.containsDirectory(target) {
            selectedItem = loadedTree.children(of: target)?.first
            errorMessage = nil
            scanStage = "Loaded tree"
            return
        }
        // A different scope requires an explicit new full scan; descendants already
        // present in the inventory never arrive here.
        if loadedTree != nil {
            loadedTree = nil
            scan = nil
            directoryNavigation.reset(rootPath: target)
            refresh(fast: false, rollbackNavigation: previous)
            return
        }
        if let cached = scansByPath[target] {
            if let activeJobID = scanOwnership.activeJobID {
                service.cancelScan(jobID: activeJobID)
            }
            scanOwnership.cancel()
            scanTask?.cancel()
            progressTask?.cancel()
            scanTask = nil
            progressTask = nil
            isScanning = false
            streamingItems = []
            scan = cached
            selectedItem = cached.largestItems.first
            errorMessage = nil
            scanStage = "Cached"
        } else {
            refresh(rollbackNavigation: previous)
        }
    }

    func selectedCleanupTotal(in scan: AppScan) -> UInt64 {
        selectedCleanupTotal(scanCandidates: scan.cleanupCandidates)
    }

    func selectedCleanupTotalForRecipeOnly() -> UInt64 {
        selectedCleanupTotal(scanCandidates: [])
    }

    func selectedCleanupTotal(from items: [AppScanItem]) -> UInt64 {
        items.filter { selectedCleanupPaths.contains($0.path) }
            .reduce(UInt64(0)) { $0 &+ $1.sizeBytes }
    }

    func safeOpportunities() -> [AppScanItem] {
        var byPath: [String: AppScanItem] = [:]
        for recipe in recipes {
            for item in recipe.items where item.appEligible && item.safety == .safe && item.path != "/dev/null" {
                let candidate = AppScanItem(recipeItem: item, recipe: recipe)
                if candidate.sizeBytes > (byPath[item.path]?.sizeBytes ?? 0) {
                    byPath[item.path] = candidate
                }
            }
        }
        if let scan {
            for item in scan.cleanupCandidates where item.safety == .safe && item.canMoveToTrash {
                byPath[item.path] = item
            }
        }
        return byPath.values.sorted {
            $0.sizeBytes == $1.sizeBytes ? $0.path < $1.path : $0.sizeBytes > $1.sizeBytes
        }
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
        guard !busy else { return }
        cleanupReviewTokens.removeAll()
        collectedItems[item.path] = item
        selectedCleanupPaths = CleanupSelectionRules.toggling(
            path: item.path,
            isSelectable: item.canMoveToTrash,
            in: selectedCleanupPaths
        )
    }

    func addSafeCaches(from items: [AppScanItem]) {
        guard !busy else { return }
        let eligible = CleanupSelectionRules.safeSelectablePaths(in: items)
        for item in items where eligible.contains(item.path) && !selectedCleanupPaths.contains(item.path) {
            toggleCleanup(item)
        }
    }

    func selectSafeCandidates(in scan: AppScan) {
        cleanupReviewTokens.removeAll()
        selectedCleanupPaths = CleanupSelectionRules.safeSelectablePaths(in: scan.cleanupCandidates)
    }

    func selectSafeCandidates(from items: [AppScanItem]) {
        cleanupReviewTokens.removeAll()
        selectedRecipe = nil
        for item in items { collectedItems[item.path] = item }
        selectedCleanupPaths = CleanupSelectionRules.safeSelectablePaths(in: items)
    }

    func retainSafeCleanupSelection() {
        let visiblePaths = Set(safeOpportunities().map(\.path))
        let retained = selectedCleanupPaths.intersection(visiblePaths)
        if retained != selectedCleanupPaths {
            cleanupReviewTokens.removeAll()
            selectedCleanupPaths = retained
        }
    }

    func clearCleanupSelection() {
        cleanupReviewTokens.removeAll()
        selectedCleanupPaths.removeAll()
        collectedItems.removeAll()
        selectedRecipe = nil
    }

    func selectRecipePaths(_ recipe: CleanupRecipe) {
        cleanupReviewTokens.removeAll()
        selectedRecipe = recipe
        for item in recipe.items { collectedItems[item.path] = AppScanItem(recipeItem: item, recipe: recipe) }
        selectedCleanupPaths = CleanupSelectionRules.selectableRecipePaths(in: recipe)
        selectedItem = recipeCandidateItems().first
        selectedTab = .map
    }

    func cleanSelected() {
        guard !selectedCleanupPaths.isEmpty, !busy else {
            return
        }
        isCleaning = true
        cleanupMessage = nil
        cleanupOutcomes = []
        let paths = Array(selectedCleanupPaths)
        let tokens = paths.compactMap { cleanupReviewTokens[$0] }
        let projects = cleanupProjectRoots
        guard tokens.count == paths.count else {
            isCleaning = false
            cleanupMessage = "Cleanup review expired. Review the selected paths again."
            return
        }
        Task {
            do {
                let response = try await service.trash(paths: paths, reviewTokens: tokens, projects: projects)
                let failureDetail = response.outcomes
                    .compactMap(\.error)
                    .first
                    .map { " \($0)" } ?? ""
                let sessionDetail = response.sessionId.map { " Session: \($0)." } ?? ""
                let receiptDetail = response.receiptError.map { " \($0)" } ?? ""
                cleanupMessage = "Moved \(response.movedCount) item(s) (\(formatBytes(response.movedBytes))) to Trash; \(formatBytes(response.reclaimedBytes)) reclaimed until Trash is emptied. \(response.failedCount) failed.\(sessionDetail)\(failureDetail)\(receiptDetail)"
                cleanupOutcomes = response.outcomes
                selectedCleanupPaths.removeAll()
                collectedItems.removeAll()
                cleanupReviewTokens.removeAll()
                selectedRecipe = nil
                loadHistory()
                recipes = []
                cacheItems = []
                cacheWarnings = []
                forgetScans()
                // The cached scan describes the files before the Trash move.
                // Invalidate the old view; refresh only when the user asks.
                loadedTree = nil
                scan = nil
                selectedItem = nil
                scanStage = "Refresh to update storage"
            } catch {
                cleanupMessage = error.localizedDescription
            }
            isCleaning = false
        }
    }

    func preflightCleanup(_ completion: @escaping (Bool) -> Void) {
        guard !selectedCleanupPaths.isEmpty, !busy else {
            completion(false)
            return
        }
        isCleaning = true
        cleanupMessage = nil
        cleanupOutcomes = []
        let paths = Array(selectedCleanupPaths)
        cleanupProjectRoots = []
        let projects = cleanupProjectRoots
        Task {
            defer { isCleaning = false }
            do {
                let response = try await service.trash(paths: paths, dryRun: true, projects: projects)
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
        guard !busy else {
            return
        }
        isRestoring = true
        restoreMessage = nil
        Task {
            do {
                let response = try await service.restore(sessionId: session.sessionId)
                restoreMessage = "Restored \(response.restoredCount) item(s), \(response.failedCount) failed." + response.outcomes.compactMap { $0.error.map { " \($0)" } }.joined()
                loadHistory()
                forgetScans()
                loadedTree = nil
                scan = nil
                recipes = []
                cacheItems = []
                cacheWarnings = []
                clearCleanupSelection()
                scanStage = "Refresh to update storage"
            } catch {
                restoreMessage = error.localizedDescription
            }
            isRestoring = false
        }
    }
}
