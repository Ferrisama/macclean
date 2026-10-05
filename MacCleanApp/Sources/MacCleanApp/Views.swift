import SwiftUI

struct DashboardView: View {
    @ObservedObject var model: AppModel
    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Your Mac, at a glance", subtitle: "Understand your storage and review what you can reclaim.")
            if let scan = model.displayScan {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 240), spacing: 16)], spacing: 16) {
                    MetricCard(title: "Scanned", value: formatBytes(scan.rootScan.tree.sizeBytes), subtitle: scan.rootScan.partial ? "Partial coverage" : "Scan complete")
                    if let data = scan.systemData { MetricCard(title: "System Data", value: formatBytes(data.totalBytes), subtitle: "Estimated storage") }
                    if let health = scan.health {
                        MetricCard(title: "Disk Free", value: formatBytes(health.diskFree), subtitle: "\(diskPercent(health))% used")
                        MetricCard(title: "Memory", value: formatBytes(health.memUsed), subtitle: "\(memoryPercent(health))% used")
                    }
                }
            }
            RecipeStrip(model: model)
            if let scan = model.displayScan {
                LargestListCard(title: "Largest Items", items: Array(scan.largestItems.prefix(8)), selectedItem: $model.selectedItem, onOpen: model.openInMap)
                SafetySummaryCard(totals: scan.safetyTotals)
                if let data = scan.systemData { SystemDataCard(systemData: data) }
            } else {
                EmptyPanelText("Choose a folder and start a scan. Known cleanup opportunities are already available above.")
            }
        }
    }
}

struct SafeCleanupView: View {
    @ObservedObject var model: AppModel
    @State private var search = ""
    private var items: [AppScanItem] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return model.safeOpportunities().filter {
            query.isEmpty || $0.name.localizedCaseInsensitiveContains(query)
                || $0.path.localizedCaseInsensitiveContains(query) || $0.cleanKind.localizedCaseInsensitiveContains(query)
        }
    }
    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Safe Cleanup", subtitle: "Regenerable caches and artifacts, largest first. Click any row to inspect it.")
            HStack(spacing: 10) {
                TextField("Search caches, names, or paths", text: $search).textFieldStyle(.roundedBorder).accessibilityIdentifier("safe.search")
                Button { model.loadRecipes() } label: { Label("Refresh", systemImage: "arrow.clockwise") }.disabled(model.isLoadingRecipes)
                Button { model.scanHomeForSafeCleanup() } label: { Label("Find Hidden Caches", systemImage: "magnifyingglass") }
                    .disabled(model.isScanning).help("Deep scan your home folder, including hidden folders.")
            }
            Text("\(items.count) opportunities • largest first").font(.callout).foregroundStyle(.secondary)
            if let scan = model.scan, scan.rootScan.partial { PartialScanNotice(reason: scan.rootScan.incompleteReason) }
            CandidateList(items: items, selectedItem: $model.selectedItem, selectedPaths: $model.selectedCleanupPaths,
                          selectedTotal: model.selectedCleanupTotal(from: model.safeOpportunities()), isCleaning: model.isCleaning || model.isScanning,
                          message: model.cleanupMessage, outcomes: model.cleanupOutcomes,
                          onSelectSafe: { model.selectSafeCandidates(from: items) }, onClear: model.clearCleanupSelection,
                          onToggle: model.toggleCleanup, onPreflight: model.preflightCleanup, onClean: model.cleanSelected, onOpen: model.openInMap)
        }
        .onAppear { model.retainSafeCleanupSelection() }
    }
}

struct MapView: View {
    @ObservedObject var model: AppModel
    @State private var sortMode: MapSort = .size
    @State private var safetyFilter: StorageSafety?
    @State private var kindFilter: String?

    var body: some View {
        ScreenScaffold(model: model) { scan in
            GeometryReader { proxy in
                let items = visibleItems(scan)
                WorkspacePage {
                    WorkspaceHeading(title: "Storage Map", subtitle: "Click a tile to inspect it. Double-click a folder or use Open Folder to explore it.")
                    navigationBar(scan: scan)
                    filterBar(scan: scan)
                    mapCanvas(items: items, minimumHeight: 240).frame(height: min(420, max(240, proxy.size.width * 0.42)))
                    MapCompactInspector(item: inspectorItem(scan), onOpen: model.open)
                    HStack {
                        Text("Folders and files").font(.headline)
                        Spacer()
                        Text("\(items.count) items").foregroundStyle(.secondary)
                    }
                    LazyVStack(spacing: 8) {
                        ForEach(items) { item in
                            MapItemRow(item: item, isSelected: model.selectedItem?.id == item.id,
                                       onSelect: { model.selectedItem = item }, onOpen: { model.open(item) })
                        }
                    }
                }
            }
        }
    }

    private func navigationBar(scan: AppScan) -> some View {
        HStack(spacing: 8) {
            Button(action: model.goBack) {
                Image(systemName: "chevron.left")
            }
            .buttonStyle(AeroIconButtonStyle())
            .disabled(!model.directoryNavigation.canGoBack || model.isScanning)
            .help("Back")
            .accessibilityIdentifier("map.back")

            Button(action: model.goForward) {
                Image(systemName: "chevron.right")
            }
            .buttonStyle(AeroIconButtonStyle())
            .disabled(!model.directoryNavigation.canGoForward || model.isScanning)
            .help("Forward")
            .accessibilityIdentifier("map.forward")

            Button(action: model.goUp) {
                Image(systemName: "arrow.up")
            }
            .buttonStyle(AeroIconButtonStyle())
            .disabled(!model.directoryNavigation.canGoUp || model.isScanning)
            .help("Parent folder")
            .accessibilityIdentifier("map.parent")

            Button(action: model.goToScanRoot) {
                Image(systemName: "scope")
            }
            .buttonStyle(AeroIconButtonStyle())
            .disabled(model.directoryNavigation.isAtRoot || model.isScanning)
            .help("Return to scan root: \(model.directoryNavigation.rootPath)")
            .accessibilityIdentifier("map.root")

            BreadcrumbBar(path: model.path, onNavigate: model.navigate)
                .frame(maxWidth: .infinity, alignment: .leading)

            if let error = model.errorMessage {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
                    .help(error)
                    .accessibilityLabel(error)
            }

            Text(formatBytes(scan.rootScan.tree.sizeBytes))
                .font(.headline.monospacedDigit())
                .foregroundStyle(.secondary)
        }
        .aeroControlGroup()
    }

    private func filterBar(scan: AppScan) -> some View {
        HStack(spacing: 12) {
            Picker("Sort", selection: $sortMode) {
                ForEach(MapSort.allCases) { mode in Text(mode.title).tag(mode) }
            }
            Picker("Safety", selection: $safetyFilter) {
                Text("All safety levels").tag(StorageSafety?.none)
                ForEach(StorageSafety.allCases) { safety in Text(safety.label).tag(Optional(safety)) }
            }
            Picker("Type", selection: $kindFilter) {
                Text("All types").tag(String?.none)
                ForEach(mapKinds(scan), id: \.self) { kind in Text(kind.capitalized).tag(Optional(kind)) }
            }
        }
        .pickerStyle(.menu)
        .aeroControlGroup()
    }

    private func mapCanvas(
        items: [AppScanItem],
        minimumHeight: CGFloat
    ) -> some View {
        StorageTreemap(
            items: items,
            selectedItem: $model.selectedItem,
            onOpen: model.open
        )
        .frame(maxWidth: .infinity)
        .frame(minHeight: minimumHeight)
        .aeroPanel(cornerRadius: 12, padding: 8)
    }

    private func visibleItems(_ scan: AppScan) -> [AppScanItem] {
        let filtered = model.mapItems(in: scan).filter {
            (safetyFilter == nil || $0.safety == safetyFilter)
                && (kindFilter == nil || $0.cleanKind == kindFilter)
        }
        switch sortMode {
        case .size: return filtered.sorted { $0.sizeBytes > $1.sizeBytes }
        case .name: return filtered.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
        case .safety: return filtered.sorted { $0.safety.sortOrder < $1.safety.sortOrder }
        }
    }

    private func mapKinds(_ scan: AppScan) -> [String] {
        Array(Set(model.mapItems(in: scan).map(\.cleanKind).filter { !$0.isEmpty && $0 != "Unknown" })).sorted()
    }

    private func inspectorItem(_ scan: AppScan) -> AppScanItem? {
        let items = visibleItems(scan)
        if let selected = model.selectedItem, items.contains(where: { $0.path == selected.path }) {
            return selected
        }
        return items.first
    }
}

struct MapItemRow: View {
    let item: AppScanItem
    let isSelected: Bool
    let onSelect: () -> Void
    let onOpen: () -> Void

    var body: some View {
        HStack(spacing: 9) {
            Button(action: onSelect) {
                HStack(spacing: 9) {
                    Image(systemName: item.isDir ? "folder.fill" : "doc.fill")
                        .foregroundStyle(item.safety.color)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(item.name)
                            .lineLimit(1)
                        Text(formatBytes(item.sizeBytes))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if item.isDir {
                Button(action: onOpen) {
                    Image(systemName: "arrow.right")
                }
                .buttonStyle(.plain)
                .help("Open in map")
            }
            Button {
                revealInFinder(item.path)
            } label: {
                Image(systemName: "finder")
            }
            .buttonStyle(.plain)
            .help("Reveal in Finder")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            isSelected ? item.safety.color.opacity(0.16) : Color.clear,
            in: RoundedRectangle(cornerRadius: 11, style: .continuous)
        )
    }
}

struct MapCompactInspector: View {
    let item: AppScanItem?
    let onOpen: (AppScanItem) -> Void

    var body: some View {
        MapInspectorContent(item: item, onOpen: onOpen)
            .aeroPanel(cornerRadius: 18, padding: 12)
    }
}

struct MapInspectorContent: View {
    let item: AppScanItem?
    let onOpen: (AppScanItem) -> Void

    var body: some View {
        if let item {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 9) {
                    Image(systemName: item.isDir ? "folder.fill" : "doc.fill")
                        .foregroundStyle(item.safety.color)
                    Text(item.name)
                        .font(.headline)
                        .lineLimit(1)
                    Spacer()
                    Text(formatBytes(item.sizeBytes))
                        .font(.headline.monospacedDigit())
                }
                Text(item.path)
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
                Text(item.cleanupReason)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                HStack {
                    SafetyBadge(safety: item.safety)
                    Spacer()
                    if item.isDir {
                        Button("Open Folder") { onOpen(item) }
                    }
                    Button("Reveal") { revealInFinder(item.path) }
                }
            }
        } else {
            Text("Select a map tile to inspect it.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

enum MapSort: String, CaseIterable, Identifiable {
    case size, name, safety
    var id: String { rawValue }
    var title: String { rawValue.capitalized }
}

struct DuplicatesView: View {
    @ObservedObject var model: AppModel
    @State private var confirmingCleanup = false

    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Duplicate Files", subtitle: "Choose a keeper, inspect the matching copies, then review your selection.")
            HStack {
                Stepper(
                    "Minimum \(model.duplicateMinMB) MB",
                    value: $model.duplicateMinMB,
                    in: 1...1024,
                    step: 1
                )
                .frame(width: 190)
                if model.isScanningDuplicates {
                    Button("Cancel", action: model.cancelDuplicateScan)
                } else {
                    Button {
                        model.scanDuplicates()
                    } label: {
                        Label("Find Duplicates", systemImage: "doc.on.doc")
                    }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("duplicates.scan")
                }
            }

            if model.isScanningDuplicates {
                VStack(alignment: .leading, spacing: 8) {
                    if let fraction = model.duplicateProgress?.fraction {
                        ProgressView(value: fraction)
                    } else {
                        ProgressView()
                    }
                    Text(model.duplicateProgress?.stage == "hashing"
                        ? "Hashing \(model.duplicateProgress?.processedCandidateFiles ?? 0) of \(model.duplicateProgress?.candidateFiles ?? 0) same-size candidates"
                        : "Discovering files… \(model.duplicateProgress?.scannedFiles ?? 0) examined")
                        .foregroundStyle(.secondary)
                }
            } else if let error = model.duplicateError {
                EmptyState(message: error)
            } else if let report = model.duplicateReport {
                HStack(spacing: 12) {
                    MetricCard(
                        title: "Recoverable copies",
                        value: formatBytes(report.totalWastedBytes),
                        subtitle: "\(report.groups.count) content-identical group(s)"
                    )
                    MetricCard(
                        title: "Files examined",
                        value: "\(report.scannedFiles)",
                        subtitle: "\(report.hashedFiles) candidate file(s) hashed"
                    )
                }

                if report.partial {
                    PartialScanNotice(
                        reason: "\(report.errorCount) path(s) could not be read; results may be incomplete."
                    )
                }

                if report.groups.isEmpty {
                    EmptyPanelText("No content-identical files at or above \(model.duplicateMinMB) MB.")
                        .frame(maxWidth: .infinity, minHeight: 140)
                } else {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(model.selectedDuplicateCount == 1
                                ? "1 copy selected"
                                : "\(model.selectedDuplicateCount) copies selected")
                                .font(.headline)
                            Text("\(formatBytes(model.selectedDuplicateBytes)) will move to Trash; one keeper per group is protected.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        if model.isCleaningDuplicates {
                            ProgressView()
                                .controlSize(.small)
                        }
                        Button {
                            model.preflightDuplicateCleanup { passed in
                                confirmingCleanup = passed
                            }
                        } label: {
                            Label("Move Selected to Trash", systemImage: "trash")
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(.red)
                        .disabled(model.selectedDuplicateCount == 0 || model.isCleaningDuplicates)
                        .accessibilityIdentifier("duplicates.cleanup")
                    }

                    if let message = model.duplicateCleanupMessage {
                        Text(message)
                            .font(.callout)
                            .foregroundStyle(model.duplicateCleanupOutcomes.contains { $0.error != nil }
                                ? Color.orange
                                : Color.secondary)
                            .textSelection(.enabled)
                    }

                    let failedOutcomes = model.duplicateCleanupOutcomes.filter { $0.error != nil }
                    if !failedOutcomes.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(failedOutcomes) { outcome in
                                Text("\(outcome.path): \(outcome.error ?? "Unknown error")")
                                    .font(.caption.monospaced())
                                    .foregroundStyle(.orange)
                                    .textSelection(.enabled)
                            }
                        }
                    }

                    LazyVStack(spacing: 16) {
                    ForEach(report.groups) { group in
                        GroupBox {
                            HStack {
                                Picker(
                                    "Keep",
                                    selection: Binding(
                                        get: {
                                            model.duplicateSelections[group.id]?.strategy ?? .newest
                                        },
                                        set: { model.setDuplicateStrategy($0, for: group) }
                                    )
                                ) {
                                    Text("Newest").tag(DuplicateKeepStrategy.newest)
                                    Text("Oldest").tag(DuplicateKeepStrategy.oldest)
                                    Text("Shortest path").tag(DuplicateKeepStrategy.shortestPath)
                                    Text("Manual").tag(DuplicateKeepStrategy.manual)
                                }
                                .frame(width: 240)
                                Spacer()
                                Button("Select Copies") {
                                    model.selectDuplicateCopies(in: group)
                                }
                                Button("Clear") {
                                    model.clearDuplicateCopies(in: group)
                                }
                            }
                            ForEach(group.files) { file in
                                HStack(spacing: 10) {
                                    let selection = model.duplicateSelections[group.id]
                                    let isKeeper = selection?.keeperPath == file.path
                                    let isSelected = selection?.selectedDeletionPaths.contains(file.path) == true
                                    Button {
                                        model.toggleDuplicateDeletion(path: file.path, in: group)
                                    } label: {
                                        Image(systemName: isKeeper
                                            ? "shield.checkered"
                                            : isSelected ? "checkmark.square.fill" : "square")
                                            .foregroundStyle(isKeeper ? .green : .secondary)
                                    }
                                    .buttonStyle(.plain)
                                    .disabled(isKeeper)
                                    .accessibilityIdentifier("duplicates.select.\(file.path)")
                                    .help(isKeeper ? "This copy is protected as the keeper" : "Move this copy to Trash")
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(URL(fileURLWithPath: file.path).lastPathComponent)
                                        Text(file.path)
                                            .font(.caption.monospaced())
                                            .foregroundStyle(.secondary)
                                            .lineLimit(1)
                                    }
                                    Spacer()
                                    Text(formatBytes(file.sizeBytes))
                                        .monospacedDigit()
                                    if !isKeeper {
                                        Button("Keep") {
                                            model.keepDuplicate(path: file.path, in: group)
                                        }
                                        .buttonStyle(.link)
                                    } else {
                                        Text("Keeper")
                                            .font(.caption.weight(.semibold))
                                            .foregroundStyle(.green)
                                    }
                                    Button {
                                        revealInFinder(file.path)
                                    } label: {
                                        Image(systemName: "finder")
                                    }
                                    .buttonStyle(.plain)
                                    .help("Reveal in Finder")
                                    .accessibilityIdentifier("duplicates.reveal.\(file.path)")
                                }
                            }
                        } label: {
                            HStack {
                                Text("\(group.files.count) copies")
                                Spacer()
                                Text("\(formatBytes(group.wastedBytes)) recoverable")
                            }
                        }
                    }
                    }
                }
            } else {
                EmptyPanelText("Choose a folder in the toolbar, then find duplicates. No files will be selected or removed.")
                    .frame(maxWidth: .infinity, minHeight: 140)
            }
        }
        .confirmationDialog(
            "Move selected duplicate copies to Trash?",
            isPresented: $confirmingCleanup,
            titleVisibility: .visible
        ) {
            Button(model.selectedDuplicateCount == 1
                ? "Move 1 Copy to Trash"
                : "Move \(model.selectedDuplicateCount) Copies to Trash", role: .destructive) {
                model.cleanSelectedDuplicates()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("The chosen keeper in every group remains in place. Moved copies are recoverable from History or Finder’s Trash until Trash is emptied.")
        }
    }
}

struct CleanReviewView: View {
    @ObservedObject var model: AppModel
    private var items: [AppScanItem] {
        if let scan = model.scan { return model.cleanupReviewItems(in: scan) }
        return model.recipeCandidateItems()
    }
    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Cleanup Review", subtitle: "Inspect paths and choose what to move to Trash. Every removal is reviewed before confirmation.")
            DisclosureGroup("Cleanup recipes") { RecipeStrip(model: model).padding(.top, 12) }
            if let recipe = model.selectedRecipe { Label(recipe.title, systemImage: "checklist").font(.headline) }
            CandidateList(items: items, selectedItem: $model.selectedItem, selectedPaths: $model.selectedCleanupPaths,
                          selectedTotal: model.selectedCleanupTotal(from: items), isCleaning: model.isCleaning || model.isScanning,
                          message: model.cleanupMessage, outcomes: model.cleanupOutcomes,
                          onSelectSafe: { model.selectSafeCandidates(from: items) }, onClear: model.clearCleanupSelection,
                          onToggle: model.toggleCleanup, onPreflight: model.preflightCleanup, onClean: model.cleanSelected, onOpen: model.openInMap)
        }
    }
}

struct DeveloperView: View {
    @ObservedObject var model: AppModel
    private var items: [AppScanItem] {
        (model.scan?.cleanupCandidates ?? []).filter {
            $0.cleanKind.localizedCaseInsensitiveContains("dev") || $0.path.contains("/.cargo/")
                || $0.path.contains("/.gradle/") || $0.path.contains("/node_modules")
        }
    }
    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Developer Cleanup", subtitle: "Review generated build output and package caches. Rust targets require manual selection.")
            HStack {
                Button("Find Home Build Output") {
                    model.scanCleanupScope(NSHomeDirectory(), tab: .developer)
                }.disabled(model.isScanning || model.isCleaning)
                Button("Review Temporary Builds") { model.selectedTab = .temporary }
            }
            Text("Stop builds and tests before cleaning. Sources are preserved; the next build may take longer.")
                .font(.callout).foregroundStyle(.secondary)
            if let scan = model.scan, scan.rootScan.partial { PartialScanNotice(reason: scan.rootScan.incompleteReason) }
            if model.scan == nil { EmptyPanelText("Scan your home folder to find project build output, including hidden folders.") }
            CandidateList(items: items, selectedItem: $model.selectedItem, selectedPaths: $model.selectedCleanupPaths,
                          selectedTotal: model.selectedCleanupTotal(from: items), isCleaning: model.isCleaning || model.isScanning,
                          message: model.cleanupMessage, outcomes: model.cleanupOutcomes,
                          onSelectSafe: { model.selectSafeCandidates(from: items) }, onClear: model.clearCleanupSelection,
                          onToggle: model.toggleCleanup, onPreflight: model.preflightCleanup, onClean: model.cleanSelected,
                          onOpen: model.openInMap)
        }
        .onAppear { model.clearCleanupSelection() }
    }
}

struct TemporaryBuildsView: View {
    @ObservedObject var model: AppModel
    private var hasTemporaryScan: Bool {
        model.scan?.rootScan.root == "/private/tmp" || model.scan?.rootScan.root == "/tmp"
    }
    private var items: [AppScanItem] {
        guard hasTemporaryScan else { return [] }
        return (model.scan?.cleanupCandidates ?? []).filter {
            let url = URL(fileURLWithPath: $0.path)
            return ["/private/tmp", "/tmp"].contains(url.deletingLastPathComponent().path)
                && (url.lastPathComponent == "target" || url.lastPathComponent.hasSuffix("-target"))
                && $0.safety == .review && $0.cleanKind.localizedCaseInsensitiveContains("dev") && $0.canMoveToTrash
        }.sorted { $0.sizeBytes > $1.sizeBytes }
    }
    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Temporary Builds", subtitle: "Review Cargo build folders in /private/tmp, largest first.")
            Button { model.scanCleanupScope("/private/tmp", tab: .temporary) } label: {
                Label(hasTemporaryScan ? "Refresh Temporary Builds" : "Find Temporary Builds", systemImage: "magnifyingglass")
            }.disabled(model.isScanning || model.isCleaning)
            Text("Only verified Cargo targets owned by your account are offered. Each folder needs manual selection. Stop builds first; open files are checked before removal.")
                .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            if !hasTemporaryScan {
                EmptyPanelText("Run a temporary-folder scan to see eligible build output. Other temporary and system data are excluded.")
            } else if let scan = model.scan, scan.rootScan.partial {
                PartialScanNotice(reason: scan.rootScan.incompleteReason)
            }
            CandidateList(items: items, selectedItem: $model.selectedItem, selectedPaths: $model.selectedCleanupPaths,
                          selectedTotal: model.selectedCleanupTotal(from: items), isCleaning: model.isCleaning || model.isScanning,
                          message: model.cleanupMessage, outcomes: model.cleanupOutcomes,
                          onSelectSafe: { model.selectSafeCandidates(from: items) }, onClear: model.clearCleanupSelection,
                          onToggle: model.toggleCleanup, onPreflight: model.preflightCleanup, onClean: model.cleanSelected,
                          onOpen: model.openInMap)
        }
        .onAppear { model.clearCleanupSelection() }
    }
}

struct MonitorView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScreenScaffold(model: model) { scan in
            WorkspacePage {
            if let health = scan.health {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 240), spacing: 14)], spacing: 14) {
                    GaugeCard(title: "Disk", percent: diskPercent(health), detail: "\(formatBytes(health.diskFree)) free")
                    GaugeCard(title: "Memory", percent: memoryPercent(health), detail: "\(formatBytes(health.memUsed)) / \(formatBytes(health.memTotal))")
                    MetricCard(title: "CPU", value: health.ncpu, subtitle: health.loadAvg)
                    MetricCard(title: "Battery", value: compactBattery(health.battery), subtitle: "Current power state")
                    MetricCard(title: "FileVault", value: health.filevault ? "On" : "Off", subtitle: "Storage encryption")
                    MetricCard(title: "Firewall", value: health.firewall ? "On" : "Off", subtitle: "Network protection")
                    MetricCard(title: "SIP", value: health.sip ? "On" : "Off", subtitle: "System integrity")
                }
            } else {
                EmptyState(message: "Enable Health and run a scan to show monitor data.")
            }
            }
        }
    }
}

struct FullDiskAccessView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        WorkspacePage {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top, spacing: 16) {
                    Image(systemName: accessIcon(model.fullDiskAccessStatus))
                        .font(.system(size: 44))
                        .foregroundStyle(accessColor(model.fullDiskAccessStatus))
                    VStack(alignment: .leading, spacing: 6) {
                        Text(model.fullDiskAccessStatus.title)
                            .font(.title2.bold())
                        Text(model.fullDiskAccessStatus.detail)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                }

                GroupBox("Why MacClean needs access") {
                    VStack(alignment: .leading, spacing: 10) {
                        AccessReason(icon: "message", text: "Measure protected Messages and attachment storage.")
                        AccessReason(icon: "envelope", text: "Inspect Mail downloads and caches.")
                        AccessReason(icon: "safari", text: "Account for Safari website data and caches.")
                        AccessReason(icon: "shippingbox", text: "Discover protected app containers and developer data.")
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 6)
                }

                HStack {
                    Button {
                        model.openFullDiskAccessSettings()
                    } label: {
                        Label("Open System Settings", systemImage: "gear")
                    }
                    .buttonStyle(.borderedProminent)

                    Button {
                        model.checkFullDiskAccess()
                    } label: {
                        Label("Check Again", systemImage: "arrow.clockwise")
                    }
                    .disabled(model.fullDiskAccessStatus == .checking)
                }

                if !model.accessDeniedLocations.isEmpty {
                    GroupBox("Protected locations currently unavailable") {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(model.accessDeniedLocations, id: \.self) { path in
                                Text(path)
                                    .font(.caption.monospaced())
                                    .textSelection(.enabled)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }

                Text("After enabling MacClean in System Settings, return here and choose Check Again.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: 760, alignment: .leading)
        }
    }
}

struct UninstallView: View {
    @ObservedObject var model: AppModel
    @State private var search = ""
    @State private var deepReview = false
    private var filteredApps: [InstalledApplication] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return model.installedApplications.filter {
            query.isEmpty || $0.name.localizedCaseInsensitiveContains(query)
                || $0.path.localizedCaseInsensitiveContains(query)
                || ($0.bundleId?.localizedCaseInsensitiveContains(query) ?? false)
        }
    }
    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Installed Applications", subtitle: "Search your apps and open a scrollable removal-plan preview.")
            HStack {
                TextField("Search apps, paths, or bundle IDs", text: $search).textFieldStyle(.roundedBorder)
                Button { model.loadInstalledApplications() } label: { Label("Refresh", systemImage: "arrow.clockwise") }
                    .disabled(model.isLoadingInstalledApplications)
            }
            Text("\(filteredApps.count) applications").foregroundStyle(.secondary)
            if let error = model.uninstallListError { Text(error).foregroundStyle(.orange).textSelection(.enabled) }
            if model.isLoadingInstalledApplications && model.installedApplications.isEmpty { ProgressView("Loading applications…") }
            if filteredApps.isEmpty { EmptyPanelText(search.isEmpty ? "No applications found." : "No applications match your search.") }
            LazyVStack(spacing: 8) {
                ForEach(filteredApps) { app in
                    Button { model.selectInstalledApplication(app) } label: {
                        HStack(spacing: 12) {
                            ApplicationIcon(path: app.path)
                            VStack(alignment: .leading, spacing: 4) {
                                Text(app.name).font(.headline)
                                Text(app.bundleId ?? app.path).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            }
                            Spacer()
                            if app.protected { Text("Protected").font(.caption).foregroundStyle(.purple) }
                            Image(systemName: "chevron.right").foregroundStyle(.secondary)
                        }.workspaceRow()
                    }.buttonStyle(.plain).accessibilityIdentifier("uninstall.app.\(app.path)")
                }
            }
        }
        .task { if model.installedApplications.isEmpty { model.loadInstalledApplications() } }
        .sheet(isPresented: Binding(
            get: { model.selectedInstalledApplication != nil },
            set: { if !$0 { model.selectInstalledApplication(nil) } }
        )) {
            VStack(spacing: 0) {
                HStack {
                    Text("Application Review").font(.headline)
                    Spacer()
                    Button("Done") { model.selectInstalledApplication(nil) }.keyboardShortcut(.cancelAction)
                }.padding(16)
                Divider()
                WorkspacePage { UninstallInspector(model: model, deepReview: $deepReview) }
            }
            .frame(width: 680, height: 540)
        }
    }
}

struct ApplicationIcon: View {
    let path: String

    var body: some View {
        Image(nsImage: NSWorkspace.shared.icon(forFile: path))
            .resizable()
            .scaledToFit()
            .frame(width: 34, height: 34)
    }
}

struct UninstallInspector: View {
    @ObservedObject var model: AppModel
    @Binding var deepReview: Bool

    private var app: InstalledApplication? { model.selectedInstalledApplication }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("App Details")
                .font(.headline)
            if let app {
                HStack(spacing: 12) {
                    ApplicationIcon(path: app.path)
                        .frame(width: 54, height: 54)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(app.name)
                            .font(.title2.bold())
                        Text(app.protected ? "Protected system application" : "Available for removal-plan review")
                            .foregroundStyle(app.protected ? .purple : .secondary)
                    }
                }
                if let plan = model.uninstallPlan {
                    UninstallPlanPreview(plan: plan, onClose: model.clearUninstallPlan)
                } else {
                    Divider()
                    DetailLine(label: "Bundle ID", value: app.bundleId ?? "Unavailable")
                    DetailLine(label: "Location", value: app.path.hasPrefix("/Applications/") ? "System Applications" : "User Applications")
                    Text("Path")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(app.path)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)

                    Toggle("Deep review", isOn: $deepReview)
                        .disabled(app.protected || model.isLoadingUninstallPlan)
                    Text(deepReview
                         ? "Includes helpers, receipts, group containers, scripts, WebKit data, and system-level traces."
                         : "Checks the app bundle and common user Library locations.")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    if let error = model.uninstallPlanError {
                        Text(error)
                            .foregroundStyle(.red)
                            .textSelection(.enabled)
                    }

                    Text("Review is read-only. Nothing is removed by building this plan.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    HStack {
                        Button {
                            model.loadUninstallPlan(deep: deepReview)
                        } label: {
                            if model.isLoadingUninstallPlan {
                                Label("Building Plan…", systemImage: "hourglass")
                            } else {
                                Label("Review Removal Plan", systemImage: "doc.text.magnifyingglass")
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(app.protected || model.isLoadingUninstallPlan)
                        Button {
                            revealInFinder(app.path)
                        } label: {
                            Label("Reveal", systemImage: "folder")
                        }
                    }
                }
            } else {
                EmptyPanelText("Select an application to inspect it.")
            }
        }
        .padding(14)
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
    }
}

struct UninstallPlanPreview: View {
    let plan: UninstallPlan
    let onClose: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Divider()
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(plan.deep ? "Deep removal plan" : "Standard removal plan")
                        .font(.headline)
                    Text("\(plan.items.count) path(s)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text(formatBytes(plan.totalSize))
                    .font(.title3.bold())
                    .monospacedDigit()
                Button("Change Review") {
                    onClose()
                }
            }

            if plan.bundleIdGuessed {
                Label("Bundle ID was inferred. Review every match carefully.", systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.orange)
            }

            if let preflightError = plan.preflightError {
                VStack(alignment: .leading, spacing: 6) {
                    Label("Unable to verify that uninstall is safe", systemImage: "questionmark.diamond.fill")
                        .font(.headline)
                        .foregroundStyle(.red)
                    Text(preflightError)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(10)
                .background(.red.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
            } else if !plan.canExecute {
                VStack(alignment: .leading, spacing: 6) {
                    Label("Quit \(plan.appName) before uninstalling", systemImage: "exclamationmark.octagon.fill")
                        .font(.headline)
                        .foregroundStyle(.red)
                    Text("Execution is blocked while these processes are running:")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(Array(plan.runningProcesses.prefix(5))) { process in
                        VStack(alignment: .leading, spacing: 2) {
                            Text("PID \(process.pid)")
                                .font(.caption.weight(.semibold))
                            Text(process.command)
                                .font(.caption2.monospaced())
                                .lineLimit(2)
                                .textSelection(.enabled)
                        }
                    }
                    if plan.runningProcesses.count > 5 {
                        Text("And \(plan.runningProcesses.count - 5) more matching process(es).")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(10)
                .background(.red.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
            } else {
                Label("Preflight passed: the app is not running", systemImage: "checkmark.shield.fill")
                    .font(.caption)
                    .foregroundStyle(.green)
            }

            LazyVStack(alignment: .leading, spacing: 12) {
            ForEach(plan.items) { item in
                VStack(alignment: .leading, spacing: 5) {
                    HStack {
                        Image(systemName: "doc")
                        Text(item.path)
                            .font(.caption.monospaced())
                            .lineLimit(2)
                        Spacer(minLength: 8)
                        RiskBadge(risk: item.risk)
                        Text(formatBytes(item.sizeBytes))
                            .monospacedDigit()
                    }
                    Text(item.reason)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 3)
            }
            }

            Label("Read-only preview — no files have been changed.", systemImage: "lock")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}

struct RiskBadge: View {
    let risk: String

    private var color: Color {
        switch risk.lowercased() {
        case "low": .green
        case "medium": .orange
        default: .red
        }
    }

    var body: some View {
        Text(risk.capitalized)
            .font(.caption2.weight(.semibold))
            .foregroundStyle(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(color.opacity(0.14), in: Capsule())
    }
}

struct AccessReason: View {
    let icon: String
    let text: String

    var body: some View {
        Label(text, systemImage: icon)
            .foregroundStyle(.secondary)
    }
}

private func accessIcon(_ status: FullDiskAccessStatus) -> String {
    switch status {
    case .checking: "hourglass"
    case .granted: "checkmark.shield.fill"
    case .denied: "exclamationmark.shield.fill"
    case .unavailable: "questionmark.diamond.fill"
    }
}

private func accessColor(_ status: FullDiskAccessStatus) -> Color {
    switch status {
    case .checking: .secondary
    case .granted: .green
    case .denied: .orange
    case .unavailable: .yellow
    }
}

struct HistoryView: View {
    @ObservedObject var model: AppModel
    @State private var restoreTarget: HistorySession?

    var body: some View {
        WorkspacePage {
            HStack {
                WorkspaceHeading(title: "History", subtitle: "Review cleanup sessions and restore items from Trash.")
                Spacer()
                Button {
                    model.loadHistory()
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(model.isLoadingHistory || model.isRestoring)
            }

            if let message = model.restoreMessage {
                Text(message)
                    .foregroundStyle(.secondary)
            }

            if model.history.isEmpty {
                EmptyPanelText(model.isLoadingHistory ? "Loading history..." : "No cleanup history yet.")
            } else {
                LazyVStack(spacing: 10) {
                ForEach(model.history) { session in
                    HStack {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(session.cleaner)
                                .font(.headline)
                            Text(session.sessionId)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer(minLength: 12)
                        Text("\(session.itemCount) item(s)")
                        Text(formatBytes(session.totalBytes))
                            .monospacedDigit()
                            .frame(width: 100, alignment: .trailing)

                        Button {
                            restoreTarget = session
                        } label: {
                            Label("Restore", systemImage: "arrow.uturn.backward")
                        }
                        .disabled(session.restorableCount == 0 || model.isRestoring)
                    }
                    .workspaceRow()
                }
                }
            }
        }
        .task {
            model.loadHistory()
        }
        .confirmationDialog(
            "Restore this cleanup session?",
            item: $restoreTarget,
            titleVisibility: .visible
        ) { session in
            Button("Restore", role: .destructive) {
                model.restore(session)
            }
            Button("Cancel", role: .cancel) {}
        } message: { session in
            Text("macclean will move restorable Trash items back to their original paths for \(session.sessionId).")
        }
    }
}

struct ScreenScaffold<Content: View>: View {
    @ObservedObject var model: AppModel
    let content: (AppScan) -> Content
    init(model: AppModel, @ViewBuilder content: @escaping (AppScan) -> Content) {
        self.model = model
        self.content = content
    }
    var body: some View {
        VStack(spacing: 0) {
            if let scan = model.displayScan {
                if scan.rootScan.partial {
                    PartialScanNotice(reason: scan.rootScan.incompleteReason).padding(.horizontal, 20).padding(.top, 8)
                }
                content(scan)
            } else {
                WorkspacePage {
                    EmptyState(message: model.errorMessage ?? (model.isScanning ? "Scanning…" : "Choose a folder and run a scan to begin."))
                }
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

struct PartialScanNotice: View {
    let reason: String?

    var body: some View {
        Label(reason ?? "This scan is incomplete; totals may be understated.", systemImage: "exclamationmark.triangle.fill")
            .font(.caption)
            .foregroundStyle(.orange)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct MetricCard: View {
    let title: String
    let value: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(size: 30, weight: .bold, design: .rounded))
                .lineLimit(1).minimumScaleFactor(0.7)
            Text(subtitle)
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .aeroSurface()
    }
}

struct RecipeStrip: View {
    @ObservedObject var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Cleanup Recipes")
                    .font(.headline)
                if model.isLoadingRecipes {
                    ProgressView()
                        .controlSize(.small)
                }
                Spacer()
                Button {
                    model.loadRecipes()
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
            }
            if model.recipes.isEmpty {
                EmptyPanelText(model.isLoadingRecipes ? "Finding cleanup recipes..." : "No recipe data yet.")
                    .frame(height: 120)
            } else {
                LazyVGrid(
                    columns: [GridItem(.adaptive(minimum: 260, maximum: 360), spacing: 12)],
                    alignment: .leading,
                    spacing: 12
                ) {
                    ForEach(model.recipes) { recipe in
                        RecipeCard(recipe: recipe) {
                            model.selectRecipePaths(recipe)
                        }
                    }
                }
            }
        }
        .padding(14)
        .aeroSurface()
    }
}

struct RecipeCard: View {
    let recipe: CleanupRecipe
    let onSelect: () -> Void

    private var appEligibleCount: Int {
        recipe.items.filter { $0.appEligible && $0.path != "/dev/null" }.count
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                SafetyBadge(safety: recipe.safety)
                Spacer()
                Text(formatBytes(recipe.totalBytes))
                    .font(.headline)
                    .monospacedDigit()
            }
            Text(recipe.title)
                .font(.headline)
                .lineLimit(1)
            Text(recipe.subtitle)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                Text("\(appEligibleCount) ready item(s)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    onSelect()
                } label: {
                    Label("Select", systemImage: "checkmark.circle")
                }
                .disabled(appEligibleCount == 0)
                .help(appEligibleCount == 0 ? "No items in this recipe are eligible for direct app cleanup." : "Select eligible recipe paths in Clean Review")
                .accessibilityIdentifier("recipe.select.\(recipe.id)")
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .aeroSurface(cornerRadius: 16)
    }
}

struct GaugeCard: View {
    let title: String
    let percent: Int
    let detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(title)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                Text("\(percent)%")
                    .font(.headline)
            }
            ProgressView(value: Double(percent), total: 100)
                .tint(percent >= 90 ? .red : percent >= 75 ? .orange : .green)
            Text(detail)
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .padding(14)
        .aeroSurface()
    }
}

struct SafetySummaryCard: View {
    let totals: [SafetyTotal]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Safety Tiers")
                .font(.headline)
            ForEach(totals.sorted { $0.safety.sortOrder < $1.safety.sortOrder }) { total in
                HStack {
                    SafetyBadge(safety: total.safety)
                    Spacer()
                    Text(formatBytes(total.sizeBytes))
                        .monospacedDigit()
                    Text("\(total.itemCount)")
                        .foregroundStyle(.secondary)
                        .frame(width: 36, alignment: .trailing)
                }
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .aeroSurface()
    }
}

struct LargestListCard: View {
    let title: String
    let items: [AppScanItem]
    @Binding var selectedItem: AppScanItem?
    var onOpen: ((AppScanItem) -> Void)? = nil
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline)
            if items.isEmpty { EmptyPanelText("No items returned. Run a scan to explore storage.") }
            ForEach(items) { item in
                MapItemRow(item: item, isSelected: selectedItem?.id == item.id,
                           onSelect: { selectedItem = item }, onOpen: { onOpen?(item) })
            }
        }
        .padding(16).aeroSurface()
    }
}

struct CandidateList: View {
    let items: [AppScanItem]
    @Binding var selectedItem: AppScanItem?
    @Binding var selectedPaths: Set<String>
    let selectedTotal: UInt64
    let isCleaning: Bool
    let message: String?
    let outcomes: [AppTrashOutcome]
    let onSelectSafe: () -> Void
    let onClear: () -> Void
    let onToggle: (AppScanItem) -> Void
    let onPreflight: (@escaping (Bool) -> Void) -> Void
    let onClean: () -> Void
    var onOpen: ((AppScanItem) -> Void)? = nil
    @State private var confirmingClean = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("\(selectedPaths.count) selected").font(.headline)
                    Text(formatBytes(selectedTotal)).foregroundStyle(.secondary).monospacedDigit()
                }
                Spacer(minLength: 8)
                Button("Select Safe", action: onSelectSafe)
                    .disabled(isCleaning || !items.contains { $0.safety == .safe && $0.canMoveToTrash })
                Button("Clear", action: onClear).disabled(isCleaning || selectedPaths.isEmpty)
                Button { onPreflight { confirmingClean = $0 } } label: { Label("Move to Trash", systemImage: "trash") }
                    .buttonStyle(.borderedProminent).disabled(selectedPaths.isEmpty || isCleaning)
                    .accessibilityIdentifier("cleanup.review")
            }.padding(12).aeroSurface()
            if let message {
                Text(message).font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
            }
            if !outcomes.isEmpty {
                DisclosureGroup("Cleanup results (\(outcomes.count))") {
                    ForEach(outcomes) { outcome in
                        VStack(alignment: .leading, spacing: 4) {
                            Label(outcome.path, systemImage: outcome.moved ? "checkmark.circle" : "exclamationmark.circle")
                                .font(.caption).textSelection(.enabled)
                            if let error = outcome.error { Text(error).font(.caption).foregroundStyle(.orange) }
                        }.padding(.vertical, 4)
                    }
                }
            }
            if items.isEmpty { EmptyPanelText("No matching cleanup items. Refresh known caches or run a deeper scan.") }
            LazyVStack(spacing: 8) {
                ForEach(items) { item in
                    let inspected = selectedItem?.id == item.id
                    VStack(alignment: .leading, spacing: 10) {
                        HStack(spacing: 10) {
                            Button { onToggle(item) } label: {
                                Image(systemName: selectedPaths.contains(item.path) ? "checkmark.square.fill" : "square")
                                    .font(.title3).foregroundStyle(selectedPaths.contains(item.path) ? Color.accentColor : .secondary)
                                    .frame(width: 28, height: 28).contentShape(Rectangle())
                            }
                            .buttonStyle(.plain).disabled(!item.canMoveToTrash || isCleaning)
                            .accessibilityLabel("Include \(item.name) in cleanup")
                            .accessibilityIdentifier("cleanup.select.\(item.path)")
                            Button { selectedItem = inspected ? nil : item } label: {
                                ItemRow(item: item).contentShape(Rectangle())
                            }
                                .buttonStyle(.plain).accessibilityIdentifier("cleanup.inspect.\(item.path)")
                        }
                        if inspected {
                            Divider()
                            Text(item.cleanupReason).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                            Text(item.path).font(.caption.monospaced()).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                            HStack {
                                if item.isDir, let onOpen {
                                    Button { onOpen(item) } label: { Label("Open Folder", systemImage: "folder") }
                                }
                                Button { revealInFinder(item.path) } label: { Label("Reveal in Finder", systemImage: "finder") }
                                if item.partial { Text("Size may be incomplete").font(.caption).foregroundStyle(.orange) }
                            }
                        }
                    }.workspaceRow(selected: inspected)
                }
            }
        }
        .confirmationDialog("Move selected items to Trash?", isPresented: $confirmingClean, titleVisibility: .visible) {
            Button("Move to Trash", role: .destructive, action: onClean)
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Selected items are recoverable through History until Trash is emptied. Space is reclaimed after Trash is emptied.")
        }
    }
}

struct ItemRow: View {
    let item: AppScanItem

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: item.isDir ? "folder" : "doc")
                .foregroundStyle(item.safety.color)
                .frame(width: 20)
            VStack(alignment: .leading, spacing: 2) {
                Text(item.name)
                    .lineLimit(1)
                Text(item.path)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer()
            SafetyBadge(safety: item.safety)
            Text(formatBytes(item.sizeBytes))
                .monospacedDigit()
                .frame(width: 92, alignment: .trailing)
        }
        .padding(.vertical, 3)
    }
}

struct SystemDataCard: View {
    let systemData: SystemDataScan

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("System Data")
                    .font(.headline)
                Spacer()
                Text(formatBytes(systemData.totalBytes))
                    .foregroundStyle(.secondary)
            }
            ForEach(systemData.categories.filter { $0.sizeBytes > 0 }) { category in
                HStack {
                    SafetyBadge(safety: category.safety)
                    Text(category.name)
                    Spacer()
                    Text(category.cleanWith)
                        .foregroundStyle(.secondary)
                    Text(formatBytes(category.sizeBytes))
                        .monospacedDigit()
                        .frame(width: 92, alignment: .trailing)
                }
            }
        }
        .padding(14)
        .aeroSurface()
    }
}

struct StorageTreemap: View {
    let items: [AppScanItem]
    @Binding var selectedItem: AppScanItem?
    var onOpen: ((AppScanItem) -> Void)? = nil
    @State private var hoveredItemID: String?

    var body: some View {
        GeometryReader { proxy in
            let rects = treemapRects(
                items: items,
                in: CGRect(origin: .zero, size: proxy.size)
            )
            ZStack {
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .fill(Color.black.opacity(0.08))
                if items.isEmpty {
                    EmptyPanelText("No map data")
                } else {
                    Canvas(rendersAsynchronously: true) { context, size in
                        for (index, entry) in rects.enumerated() {
                            let isSelected = entry.item.id == selectedItem?.id
                            let isHovered = entry.item.id == hoveredItemID
                            let visibleRect = entry.rect.width > 7 && entry.rect.height > 7
                                ? entry.rect.insetBy(dx: 3, dy: 3)
                                : entry.rect
                            let radius = min(
                                AeroTheme.tileRadius,
                                max(2, min(visibleRect.width, visibleRect.height) * 0.16)
                            )
                            let path = Path(roundedRect: visibleRect, cornerRadius: radius)
                            let color = treemapTileColor(for: entry.item, index: index)
                            var tileContext = context
                            if isSelected || isHovered {
                                tileContext.addFilter(.shadow(
                                    color: color.opacity(0.55),
                                    radius: isSelected ? 10 : 6
                                ))
                            }
                            tileContext.fill(
                                path,
                                with: .linearGradient(
                                    Gradient(colors: [
                                        color.opacity(isSelected ? 0.96 : isHovered ? 0.88 : 0.74),
                                        color.opacity(isSelected ? 0.72 : 0.48)
                                    ]),
                                    startPoint: CGPoint(x: visibleRect.minX, y: visibleRect.minY),
                                    endPoint: CGPoint(x: visibleRect.maxX, y: visibleRect.maxY)
                                )
                            )
                            tileContext.stroke(
                                path,
                                with: .color(.white.opacity(isSelected ? 0.82 : isHovered ? 0.5 : 0.2)),
                                lineWidth: isSelected ? 2.5 : 1
                            )

                            if entry.rect.width > 105 && entry.rect.height > 55 {
                                let text = Text("\(entry.item.name)\n\(formatBytes(entry.item.sizeBytes))")
                                    .font(.caption.bold())
                                    .foregroundColor(.white.opacity(0.94))
                                context.draw(text, at: CGPoint(x: entry.rect.midX, y: entry.rect.midY))
                            }
                        }
                    }

                    ForEach(rects, id: \.item.id) { entry in
                        Button {
                            selectedItem = entry.item
                        } label: {
                            Rectangle()
                                .fill(.clear)
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .simultaneousGesture(TapGesture(count: 2).onEnded {
                            if entry.item.isDir { onOpen?(entry.item) }
                        })
                        .frame(width: max(1, entry.rect.width), height: max(1, entry.rect.height))
                        .position(x: entry.rect.midX, y: entry.rect.midY)
                        .help(entry.item.isDir
                            ? "Open \(entry.item.name)"
                            : "Select \(entry.item.name)")
                        .accessibilityLabel(entry.item.name)
                        .accessibilityValue(formatBytes(entry.item.sizeBytes))
                        .accessibilityHint(entry.item.isDir
                            ? "Selects this folder; double-click to open"
                            : "Selects this file for inspection")
                        .accessibilityIdentifier("map.tile.\(entry.item.path)")
                        .onHover { hovering in
                            hoveredItemID = hovering ? entry.item.id : nil
                        }
                        .contextMenu {
                            if entry.item.isDir {
                                Button("Open in Map") {
                                    selectedItem = entry.item
                                    onOpen?(entry.item)
                                }
                            }
                            Button("Reveal in Finder") {
                                revealInFinder(entry.item.path)
                            }
                        }
                    }
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))

    }
}

private func treemapTileColor(for item: AppScanItem, index: Int) -> Color {
    switch item.safety {
    case .safe:
        return Color(red: 0.16, green: 0.68, blue: 0.48)
    case .review:
        return Color(red: 0.96, green: 0.55, blue: 0.18)
    case .userData:
        return Color(red: 0.91, green: 0.32, blue: 0.4)
    case .protected:
        return Color(red: 0.62, green: 0.36, blue: 0.92)
    case .unknown:
        let palette: [Color] = [
            Color(red: 0.22, green: 0.55, blue: 0.94),
            Color(red: 0.18, green: 0.67, blue: 0.78),
            Color(red: 0.35, green: 0.48, blue: 0.92),
            Color(red: 0.2, green: 0.61, blue: 0.66)
        ]
        return palette[index % palette.count]
    }
}

struct TreemapEntry {
    let item: AppScanItem
    let rect: CGRect
}

func treemapRects(items: [AppScanItem], in rect: CGRect) -> [TreemapEntry] {
    guard rect.width > 0, rect.height > 0 else { return [] }
    let sortedItems = items
        .filter { $0.sizeBytes > 0 }
        .sorted { $0.sizeBytes > $1.sizeBytes }
    let total = sortedItems.reduce(UInt64(0)) { $0 + $1.sizeBytes }
    guard total > 0 else { return [] }

    let scale = rect.width * rect.height / CGFloat(total)
    let weightedItems = sortedItems.map { ($0, CGFloat($0.sizeBytes) * scale) }
    var result: [TreemapEntry] = []
    var remaining = rect
    var row: [(AppScanItem, CGFloat)] = []
    var index = 0

    while index < weightedItems.count {
        let candidate = weightedItems[index]
        let side = min(remaining.width, remaining.height)
        if row.isEmpty || worstAspect(row + [candidate], side: side) <= worstAspect(row, side: side) {
            row.append(candidate)
            index += 1
        } else {
            remaining = layoutTreemapRow(row, in: remaining, result: &result)
            row.removeAll(keepingCapacity: true)
        }
    }
    if !row.isEmpty {
        _ = layoutTreemapRow(row, in: remaining, result: &result)
    }
    return result
}

private func worstAspect(_ row: [(AppScanItem, CGFloat)], side: CGFloat) -> CGFloat {
    guard !row.isEmpty, side > 0 else { return .infinity }
    let sum = row.reduce(CGFloat.zero) { $0 + $1.1 }
    guard sum > 0, let smallest = row.map(\.1).min(), let largest = row.map(\.1).max(), smallest > 0 else {
        return .infinity
    }
    let sideSquared = side * side
    return max(sideSquared * largest / (sum * sum), (sum * sum) / (sideSquared * smallest))
}

/// Places one squarified row, returning the unoccupied part of the parent.
/// No artificial minimum dimensions are introduced: each tile's area stays
/// proportional to its measured bytes.
@discardableResult
private func layoutTreemapRow(
    _ row: [(AppScanItem, CGFloat)],
    in rect: CGRect,
    result: inout [TreemapEntry]
) -> CGRect {
    let area = row.reduce(CGFloat.zero) { $0 + $1.1 }
    guard area > 0 else { return rect }
    if rect.width >= rect.height {
        let rowHeight = area / rect.width
        var x = rect.minX
        for (offset, entry) in row.enumerated() {
            let width = offset == row.count - 1 ? rect.maxX - x : entry.1 / rowHeight
            result.append(TreemapEntry(item: entry.0, rect: CGRect(x: x, y: rect.minY, width: width, height: rowHeight)))
            x += width
        }
        return CGRect(x: rect.minX, y: rect.minY + rowHeight, width: rect.width, height: max(0, rect.height - rowHeight))
    }

    let rowWidth = area / rect.height
    var y = rect.minY
    for (offset, entry) in row.enumerated() {
        let height = offset == row.count - 1 ? rect.maxY - y : entry.1 / rowWidth
        result.append(TreemapEntry(item: entry.0, rect: CGRect(x: rect.minX, y: y, width: rowWidth, height: height)))
        y += height
    }
    return CGRect(x: rect.minX + rowWidth, y: rect.minY, width: max(0, rect.width - rowWidth), height: rect.height)
}

struct SafetyLegend: View {
    var body: some View {
        HStack {
            ForEach(StorageSafety.allCases) { safety in
                SafetyBadge(safety: safety)
            }
            Spacer()
        }
    }
}

struct BreadcrumbBar: View {
    let path: String
    let onNavigate: (String) -> Void
    var body: some View {
        let parts = breadcrumbParts(path)
        HStack(spacing: 6) {
            if parts.count > 2 {
                Menu {
                    ForEach(parts.dropLast(2)) { part in
                        Button(part.label) { onNavigate(part.path) }
                    }
                } label: { Image(systemName: "ellipsis") }
                .menuStyle(.borderlessButton).frame(width: 20).help("Ancestor folders")
            }
            ForEach(Array(parts.suffix(2))) { part in
                Button(part.label) { onNavigate(part.path) }
                    .buttonStyle(.plain).lineLimit(1).truncationMode(.middle)
                    .help(part.path).accessibilityIdentifier("map.breadcrumb.\(part.path)")
                if part.id != parts.last?.id {
                    Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.secondary)
                }
            }
        }
    }
}

struct BreadcrumbPart: Identifiable {
    let id: String
    let label: String
    let path: String
}

func breadcrumbParts(_ path: String) -> [BreadcrumbPart] {
    let url = URL(fileURLWithPath: path).standardizedFileURL
    let components = url.pathComponents
    var parts: [BreadcrumbPart] = []
    var current = ""
    for component in components {
        if component == "/" {
            current = "/"
            parts.append(BreadcrumbPart(id: current, label: "Macintosh HD", path: current))
        } else {
            current = URL(fileURLWithPath: current).appendingPathComponent(component).path
            parts.append(BreadcrumbPart(id: current, label: component, path: current))
        }
    }
    return parts
}

struct SafetyBadge: View {
    let safety: StorageSafety

    var body: some View {
        Text(safety.label)
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(safety.color.opacity(0.16), in: Capsule())
            .foregroundStyle(safety.color)
    }
}

struct DetailLine: View {
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .multilineTextAlignment(.trailing)
        }
    }
}

struct EmptyState: View {
    let message: String

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "externaldrive.badge.magnifyingglass")
                .font(.system(size: 44))
                .foregroundStyle(.secondary)
            Text(message)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(32)
    }
}

struct EmptyPanelText: View {
    let message: String

    init(_ message: String) {
        self.message = message
    }

    var body: some View {
        Text(message)
            .foregroundStyle(.secondary)
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(18)
    }
}

func diskPercent(_ health: HealthSnapshot) -> Int {
    guard health.diskTotal > 0 else { return 0 }
    return Int((Double(health.diskUsed) / Double(health.diskTotal) * 100).rounded())
}

func memoryPercent(_ health: HealthSnapshot) -> Int {
    guard health.memTotal > 0 else { return 0 }
    return Int((Double(health.memUsed) / Double(health.memTotal) * 100).rounded())
}

func compactBattery(_ battery: String) -> String {
    if let percentRange = battery.range(of: #"\d+%"#, options: .regularExpression) {
        let percent = String(battery[percentRange])
        if battery.localizedCaseInsensitiveContains("discharging"),
           let timeRange = battery.range(of: #"\d+:\d+"#, options: .regularExpression) {
            return "\(percent), \(battery[timeRange]) left"
        }
        if battery.localizedCaseInsensitiveContains("charging") {
            return "\(percent), charging"
        }
        return percent
    }
    return battery.isEmpty ? "N/A" : battery
}

func revealInFinder(_ path: String) {
    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
}

struct VersionsReviewView: View {
    @ObservedObject var model: AppModel
    @State private var search = ""
    @State private var onlyRemovable = false
    private var items: [AppScanItem] {
        (model.versionReport?.entries ?? []).filter {
            (!onlyRemovable || $0.removable) && (search.isEmpty || $0.name.localizedCaseInsensitiveContains(search)
                || $0.family.localizedCaseInsensitiveContains(search) || $0.path.localizedCaseInsensitiveContains(search))
        }.map(\.cleanupItem)
    }
    var body: some View {
        WorkspacePage {
            WorkspaceHeading(title: "Versions Review", subtitle: "Review Rust, Node, Python, VS Code, and Cursor versions. Defaults and project pins are protected.")
            HStack {
                TextField("Additional project folder outside your home (optional)", text: $model.additionalVersionProjectRoot)
                    .textFieldStyle(.roundedBorder).disabled(model.isLoadingVersions || model.isCleaning)
                Button(model.versionReport == nil ? "Check Versions" : "Refresh") { model.loadVersions() }
                    .disabled(model.isLoadingVersions || model.isCleaning)
            }
            Text("Checks standard rustup, nvm, and pyenv locations plus editor extension metadata. Home projects are always checked; add an external project folder if you use one. No versions are selected automatically.")
                .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            if model.isLoadingVersions { ProgressView("Checking project pins and installed versions…") }
            if let error = model.versionError { Text(error).foregroundStyle(.orange).textSelection(.enabled) }
            if let report = model.versionReport {
                DisclosureGroup("Checked \(report.checkedFiles) project/config files · \(report.complete ? "Coverage complete within scan scope" : "Incomplete coverage")") {
                    VStack(alignment: .leading, spacing: 8) {
                        ForEach(report.projectRoots, id: \.self) { Text($0).font(.caption.monospaced()).textSelection(.enabled) }
                        Text("Generated folders, manager data, Library, Git data, and symlinked project folders are excluded. Usage outside these roots is unknown.")
                            .font(.caption).foregroundStyle(.secondary)
                        ForEach(Array(report.warnings.enumerated()), id: \.offset) { _, warning in Text(warning).font(.caption).foregroundStyle(.orange) }
                    }.padding(.top, 8)
                }
                HStack {
                    TextField("Search versions or editors", text: $search).textFieldStyle(.roundedBorder)
                    Toggle("Eligible only", isOn: $onlyRemovable).toggleStyle(.checkbox)
                }
                Text("\(report.entries.filter(\.removable).count) eligible · \(report.entries.filter { !$0.removable }.count) protected. Click a row to see the evidence.")
                    .font(.callout).foregroundStyle(.secondary)
                CandidateList(items: items, selectedItem: $model.selectedItem, selectedPaths: $model.selectedCleanupPaths,
                              selectedTotal: model.selectedCleanupTotal(from: report.entries.map(\.cleanupItem)),
                              isCleaning: model.isCleaning || model.isLoadingVersions,
                              message: model.cleanupMessage, outcomes: model.cleanupOutcomes,
                              onSelectSafe: { }, onClear: model.clearCleanupSelection, onToggle: model.toggleCleanup,
                              onPreflight: model.preflightCleanup, onClean: model.cleanSelected, onOpen: model.openInMap)
            } else if !model.isLoadingVersions {
                EmptyPanelText("Check Versions to inventory installed versions and the projects that use them.")
            }
        }
        .onAppear { model.clearCleanupSelection() }
        .onChange(of: model.additionalVersionProjectRoot) { _ in
            model.versionReport = nil
            model.clearCleanupSelection()
        }
    }
}
