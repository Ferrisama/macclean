import SwiftUI

struct DashboardView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                RecipeStrip(model: model)
                if let scan = model.displayScan {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 320), spacing: 14)], spacing: 14) {
                    MetricCard(
                        title: "Scanned",
                        value: formatBytes(scan.rootScan.tree.sizeBytes),
                        subtitle: "\(scan.rootScan.elapsedMs) ms . \(scan.rootScan.partial ? "partial" : "complete")"
                    )
                    if let systemData = scan.systemData {
                        MetricCard(
                            title: "System Data",
                            value: formatBytes(systemData.totalBytes),
                            subtitle: "\(systemData.elapsedMs) ms . \(systemData.partial ? "partial" : "complete")"
                        )
                    }
                    if let health = scan.health {
                        MetricCard(
                            title: "Disk Free",
                            value: formatBytes(health.diskFree),
                            subtitle: "\(diskPercent(health))% used"
                        )
                        MetricCard(
                            title: "Memory",
                            value: formatBytes(health.memUsed),
                            subtitle: "\(memoryPercent(health))% used"
                        )
                    }
                }

                HStack(alignment: .top, spacing: 14) {
                    SafetySummaryCard(totals: scan.safetyTotals)
                    LargestListCard(
                        title: "Largest Items",
                        items: scan.largestItems,
                        selectedItem: $model.selectedItem,
                        onOpen: model.openInMap
                    )
                }
                .padding(.top, 14)

                if let systemData = scan.systemData {
                    SystemDataCard(systemData: systemData)
                        .padding(.top, 14)
                }
                } else {
                    EmptyPanelText("No cached map scan yet. Recipes are available now; run Fast Scan when you want the map.")
                        .frame(minHeight: 180)
                }
            }
        }
        .padding(16)
        .overlay(alignment: .topTrailing) {
            if model.isScanning {
                ScanOverlay(stage: model.scanStage, elapsed: model.scanElapsedSeconds)
                    .padding(16)
            }
        }
    }
}

struct MapView: View {
    @ObservedObject var model: AppModel
    @State private var sortMode: MapSort = .size
    @State private var safetyFilter: StorageSafety?
    @State private var kindFilter: String?

    var body: some View {
        ScreenScaffold(model: model) { scan in
            HSplitView {
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Button {
                            model.goUp()
                        } label: {
                            Image(systemName: "chevron.up")
                        }
                        .help("Go to parent folder")

                        BreadcrumbBar(path: scan.rootScan.root, onNavigate: model.navigate)
                        Spacer()
                        Text(formatBytes(scan.rootScan.tree.sizeBytes))
                            .font(.headline)
                            .foregroundStyle(.secondary)
                    }
                    HStack {
                        Picker("Sort", selection: $sortMode) {
                            ForEach(MapSort.allCases) { mode in Text(mode.title).tag(mode) }
                        }
                        .pickerStyle(.segmented)
                        Picker("Safety", selection: $safetyFilter) {
                            Text("All").tag(StorageSafety?.none)
                            ForEach(StorageSafety.allCases) { safety in Text(safety.label).tag(Optional(safety)) }
                        }
                        .frame(width: 260)
                        Picker("Type", selection: $kindFilter) {
                            Text("All types").tag(String?.none)
                            ForEach(mapKinds(scan), id: \.self) { kind in
                                Text(kind.capitalized).tag(Optional(kind))
                            }
                        }
                        .frame(width: 180)
                        Spacer()
                    }
                    StorageTreemap(items: visibleItems(scan), selectedItem: $model.selectedItem, onOpen: model.open)
                        .frame(minWidth: 460, minHeight: 420)
                    SafetyLegend()
                }
                .padding(16)

                VStack(spacing: 12) {
                    LargestListCard(
                        title: "Largest Items",
                        items: visibleItems(scan),
                        selectedItem: $model.selectedItem,
                        onOpen: model.open
                    )
                    InspectorCard(item: inspectorItem(scan), onOpen: model.open)
                }
                .padding(16)
                .frame(minWidth: 380)
            }
        }
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

enum MapSort: String, CaseIterable, Identifiable {
    case size, name, safety
    var id: String { rawValue }
    var title: String { rawValue.capitalized }
}

struct CleanReviewView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        if let scan = model.scan {
            let reviewItems = model.cleanupReviewItems(in: scan)
            HSplitView {
                VStack(spacing: 12) {
                    RecipeStrip(model: model)
                CandidateList(
                    items: reviewItems,
                    selectedItem: $model.selectedItem,
                    selectedPaths: $model.selectedCleanupPaths,
                    selectedTotal: model.selectedCleanupTotal(in: scan),
                    isCleaning: model.isCleaning,
                    message: model.cleanupMessage,
                    outcomes: model.cleanupOutcomes,
                    onSelectSafe: { model.selectSafeCandidates(from: reviewItems) },
                    onClear: model.clearCleanupSelection,
                    onToggle: model.toggleCleanup,
                    onPreflight: model.preflightCleanup,
                    onClean: model.cleanSelected,
                    onOpen: model.open
                )
                }
                    .padding(16)
                InspectorCard(item: model.selectedItem ?? reviewItems.first)
                    .padding(16)
                    .frame(minWidth: 380)
            }
            .overlay(alignment: .topTrailing) {
                if model.isScanning {
                    ScanOverlay(stage: model.scanStage, elapsed: model.scanElapsedSeconds)
                        .padding(16)
                }
            }
        } else {
            let recipeItems = model.recipeCandidateItems()
            VStack(spacing: 14) {
                RecipeStrip(model: model)
                CandidateList(
                    items: recipeItems,
                    selectedItem: $model.selectedItem,
                    selectedPaths: $model.selectedCleanupPaths,
                    selectedTotal: model.selectedCleanupTotalForRecipeOnly(),
                    isCleaning: model.isCleaning,
                    message: model.cleanupMessage,
                    outcomes: model.cleanupOutcomes,
                    onSelectSafe: { model.selectSafeCandidates(from: recipeItems) },
                    onClear: model.clearCleanupSelection,
                    onToggle: model.toggleCleanup,
                    onPreflight: model.preflightCleanup,
                    onClean: model.cleanSelected,
                    onOpen: model.open
                )
            }
            .padding(16)
        }
    }
}

struct DeveloperView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScreenScaffold(model: model) { scan in
            let devItems = scan.cleanupCandidates.filter {
                $0.cleanKind.localizedCaseInsensitiveContains("dev")
                    || $0.path.localizedCaseInsensitiveContains("docker")
                    || $0.path.localizedCaseInsensitiveContains("xcode")
                    || $0.path.localizedCaseInsensitiveContains("android")
                    || $0.path.localizedCaseInsensitiveContains("node_modules")
            }

            VStack(alignment: .leading, spacing: 12) {
                Text("Developer Cleanup")
                    .font(.title2.bold())
                Text("Generated artifacts, package caches, Docker data, simulators, and build output.")
                    .foregroundStyle(.secondary)
                CandidateList(
                    items: devItems.isEmpty ? scan.cleanupCandidates : devItems,
                    selectedItem: $model.selectedItem,
                    selectedPaths: $model.selectedCleanupPaths,
                    selectedTotal: model.selectedCleanupTotal(in: scan),
                    isCleaning: model.isCleaning,
                    message: model.cleanupMessage,
                    outcomes: model.cleanupOutcomes,
                    onSelectSafe: { model.selectSafeCandidates(in: scan) },
                    onClear: model.clearCleanupSelection,
                    onToggle: model.toggleCleanup,
                    onPreflight: model.preflightCleanup,
                    onClean: model.cleanSelected,
                    onOpen: model.open
                )
            }
            .padding(16)
        }
    }
}

struct MonitorView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScreenScaffold(model: model) { scan in
            if let health = scan.health {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 260), spacing: 14)], spacing: 14) {
                    GaugeCard(title: "Disk", percent: diskPercent(health), detail: "\(formatBytes(health.diskFree)) free")
                    GaugeCard(title: "Memory", percent: memoryPercent(health), detail: "\(formatBytes(health.memUsed)) / \(formatBytes(health.memTotal))")
                    MetricCard(title: "CPU", value: health.ncpu, subtitle: health.loadAvg)
                    MetricCard(title: "Battery", value: compactBattery(health.battery), subtitle: "Current power state")
                    MetricCard(title: "FileVault", value: health.filevault ? "On" : "Off", subtitle: "Storage encryption")
                    MetricCard(title: "Firewall", value: health.firewall ? "On" : "Off", subtitle: "Network protection")
                    MetricCard(title: "SIP", value: health.sip ? "On" : "Off", subtitle: "System integrity")
                }
                .padding(16)
            } else {
                EmptyState(message: "Enable Health and run a scan to show monitor data.")
            }
        }
    }
}

struct FullDiskAccessView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
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
            .padding(24)
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
        guard !query.isEmpty else { return model.installedApplications }
        return model.installedApplications.filter {
            $0.name.localizedCaseInsensitiveContains(query)
                || $0.path.localizedCaseInsensitiveContains(query)
                || ($0.bundleId?.localizedCaseInsensitiveContains(query) ?? false)
        }
    }

    var body: some View {
        HSplitView {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text("Installed Applications")
                        .font(.title2.bold())
                    Spacer()
                    Text("\(filteredApps.count)")
                        .foregroundStyle(.secondary)
                    Button {
                        model.loadInstalledApplications()
                    } label: {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                    .disabled(model.isLoadingInstalledApplications)
                }

                TextField("Search apps, paths, or bundle IDs", text: $search)
                    .textFieldStyle(.roundedBorder)

                if let error = model.uninstallListError {
                    Text(error)
                        .foregroundStyle(.red)
                        .textSelection(.enabled)
                }

                if model.isLoadingInstalledApplications && model.installedApplications.isEmpty {
                    VStack(spacing: 10) {
                        ProgressView()
                        Text("Loading installed applications…")
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if filteredApps.isEmpty {
                    EmptyPanelText(search.isEmpty ? "No applications found." : "No applications match your search.")
                } else {
                    List(filteredApps, selection: Binding(
                        get: { model.selectedInstalledApplication?.id },
                        set: { id in
                            model.selectInstalledApplication(filteredApps.first { $0.id == id })
                        }
                    )) { app in
                        HStack(spacing: 10) {
                            ApplicationIcon(path: app.path)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(app.name)
                                    .font(.headline)
                                Text(app.bundleId ?? app.path)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            Spacer()
                            if app.protected {
                                Text("Protected")
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(.purple)
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 4)
                                    .background(.purple.opacity(0.14), in: Capsule())
                            }
                        }
                        .padding(.vertical, 3)
                        .tag(app.id)
                    }
                    .listStyle(.inset)
                    .scrollContentBackground(.hidden)
                }
            }
            .padding(16)
            .frame(minWidth: 520)

            UninstallInspector(model: model, deepReview: $deepReview)
                .padding(16)
                .frame(minWidth: 480)
        }
        .task {
            if model.installedApplications.isEmpty {
                model.loadInstalledApplications()
            }
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

                    Spacer()
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

            if !plan.canExecute {
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

            List(plan.items) { item in
                VStack(alignment: .leading, spacing: 5) {
                    HStack {
                        Image(systemName: "doc")
                        Text(item.path)
                            .font(.caption.monospaced())
                            .lineLimit(2)
                        Spacer()
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
            .listStyle(.inset)
            .scrollContentBackground(.hidden)

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
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("History")
                    .font(.title2.bold())
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
                List(model.history) { session in
                    HStack {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(session.cleaner)
                                .font(.headline)
                            Text(session.sessionId)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text("\(session.itemCount) item(s)")
                        Text(formatBytes(session.totalBytes))
                            .monospacedDigit()
                            .frame(width: 100, alignment: .trailing)
                        Text("\(session.restorableCount) restorable")
                            .foregroundStyle(.secondary)
                            .frame(width: 110, alignment: .trailing)
                        Button {
                            restoreTarget = session
                        } label: {
                            Label("Restore", systemImage: "arrow.uturn.backward")
                        }
                        .disabled(session.restorableCount == 0 || model.isRestoring)
                    }
                    .padding(.vertical, 4)
                }
                .scrollContentBackground(.hidden)
            }
            Spacer()
        }
        .padding(16)
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
        ZStack(alignment: .topTrailing) {
            Group {
                if let scan = model.displayScan {
                    content(scan)
                } else if let message = model.errorMessage {
                    EmptyState(message: message)
                } else {
                    EmptyState(message: model.isScanning ? "Scanning..." : "Run a scan to begin.")
                }
            }
            if model.isScanning {
                ScanOverlay(stage: model.scanStage, elapsed: model.scanElapsedSeconds)
                    .padding(16)
            }
        }
        .overlay(alignment: .bottomLeading) {
            if let scan = model.displayScan, scan.rootScan.partial {
                PartialScanNotice(reason: scan.rootScan.incompleteReason)
                    .padding(16)
            }
        }
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
            .frame(maxWidth: 420, alignment: .leading)
    }
}

struct ScanOverlay: View {
    let stage: String
    let elapsed: Int

    var body: some View {
        HStack(spacing: 8) {
            ProgressView()
                .controlSize(.small)
            Text(stage)
            Text("\(elapsed)s")
                .monospacedDigit()
        }
        .font(.caption)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(.regularMaterial, in: Capsule())
        .shadow(radius: 6, y: 2)
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
            Text(subtitle)
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
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
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 12) {
                        ForEach(model.recipes) { recipe in
                            RecipeCard(recipe: recipe) {
                                model.selectRecipePaths(recipe)
                            }
                        }
                    }
                }
            }
        }
        .padding(14)
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
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
                .lineLimit(3)
            Spacer()
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
            }
        }
        .frame(width: 260, height: 150)
        .padding(12)
        .background(.background.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
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
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
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
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
    }
}

struct LargestListCard: View {
    let title: String
    let items: [AppScanItem]
    @Binding var selectedItem: AppScanItem?
    var onOpen: ((AppScanItem) -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.headline)
            if items.isEmpty {
                EmptyPanelText("No items returned. Try Deep Scan or disable System Data for a faster map refresh.")
            } else {
                List(items, selection: Binding(
                    get: { selectedItem?.id },
                    set: { id in selectedItem = items.first { $0.id == id } }
                )) { item in
                    HStack {
                        ItemRow(item: item)
                        if item.isDir, let onOpen {
                            Button {
                                onOpen(item)
                            } label: {
                                Image(systemName: "arrow.right.circle")
                            }
                            .buttonStyle(.plain)
                            .help("Open folder")
                        }
                    }
                    .tag(item.id)
                    .onTapGesture(count: 2) {
                        if item.isDir {
                            onOpen?(item)
                        }
                    }
                }
                .scrollContentBackground(.hidden)
                .listStyle(.inset)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, minHeight: 300)
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
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
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Cleanup Review")
                    .font(.title2.bold())
                Spacer()
                Text(formatBytes(selectedTotal))
                    .font(.headline)
                    .foregroundStyle(.secondary)
                Button {
                    onSelectSafe()
                } label: {
                    Label("Select Safe", systemImage: "checkmark.circle")
                }
                Button {
                    onClear()
                } label: {
                    Label("Clear", systemImage: "xmark.circle")
                }
                Button {
                    onPreflight { passed in
                        if passed {
                            confirmingClean = true
                        }
                    }
                } label: {
                    Label("Move to Trash", systemImage: "trash")
                }
                .buttonStyle(.borderedProminent)
                .disabled(selectedPaths.isEmpty || isCleaning)
                .confirmationDialog(
                    "Move selected items to Trash?",
                    isPresented: $confirmingClean,
                    titleVisibility: .visible
                ) {
                    Button("Move to Trash", role: .destructive) {
                        onClean()
                    }
                    Button("Cancel", role: .cancel) {}
                } message: {
                    Text("Selected items are recoverable from Trash and macclean history when the backend records the move.")
                }
            }
            if let message {
                Text(message)
                    .foregroundStyle(.secondary)
            }
            if !outcomes.isEmpty {
                VStack(alignment: .leading, spacing: 5) {
                    ForEach(Array(outcomes.prefix(6))) { outcome in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Image(systemName: outcome.moved ? "checkmark.circle.fill" : "xmark.octagon.fill")
                                .foregroundStyle(outcome.moved ? .green : .red)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(outcome.path)
                                    .font(.caption.monospaced())
                                    .lineLimit(1)
                                if let trashPath = outcome.trashPath {
                                    Text("Trash: \(trashPath)")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                        .lineLimit(1)
                                }
                                if let error = outcome.error {
                                    Text(error)
                                        .font(.caption2)
                                        .foregroundStyle(.red)
                                }
                            }
                        }
                    }
                    if outcomes.count > 6 {
                        Text("And \(outcomes.count - 6) more result(s) in History.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(8)
                .background(.quaternary.opacity(0.45), in: RoundedRectangle(cornerRadius: 6))
            }
            if items.isEmpty {
                EmptyPanelText("No cleanable candidates in this scan. Use Deep Scan or scan a developer/project folder.")
            } else {
                List(items, selection: Binding(
                    get: { selectedItem?.id },
                    set: { id in selectedItem = items.first { $0.id == id } }
                )) { item in
                    HStack {
                        Button {
                            onToggle(item)
                        } label: {
                            Image(systemName: selectedPaths.contains(item.path) ? "checkmark.square.fill" : "square")
                        }
                        .buttonStyle(.plain)
                        .disabled(!item.canMoveToTrash)
                        .help(item.canMoveToTrash ? "Include in cleanup" : "This item requires its dedicated CLI cleaner")
                        ItemRow(item: item)
                        if item.isDir {
                            Button {
                                onOpen?(item)
                            } label: {
                                Image(systemName: "arrow.right.circle")
                            }
                            .buttonStyle(.plain)
                            .help("Open folder in MacClean")
                        }
                        Button {
                            revealInFinder(item.path)
                        } label: {
                            Image(systemName: "finder")
                        }
                        .buttonStyle(.plain)
                        .help("Reveal in Finder")
                    }
                    .tag(item.id)
                }
                .scrollContentBackground(.hidden)
                .listStyle(.inset)
            }
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

struct InspectorCard: View {
    let item: AppScanItem?
    var onOpen: ((AppScanItem) -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Inspector")
                .font(.headline)
            if let item {
                HStack {
                    Image(systemName: item.isDir ? "folder" : "doc")
                    Text(item.name)
                        .font(.title3.bold())
                        .lineLimit(1)
                }
                SafetyBadge(safety: item.safety)
                DetailLine(label: "Size", value: formatBytes(item.sizeBytes))
                DetailLine(label: "Kind", value: item.cleanKind)
                DetailLine(label: "Action", value: item.cleanupAction)
                VStack(alignment: .leading, spacing: 6) {
                    Text("Reason")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(item.cleanupReason)
                }
                VStack(alignment: .leading, spacing: 6) {
                    Text("Path")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(item.path)
                        .font(.caption)
                        .textSelection(.enabled)
                }
                Spacer()
                HStack {
                    if item.isDir, let onOpen {
                        Button {
                            onOpen(item)
                        } label: {
                            Label("Open", systemImage: "arrow.right.circle")
                        }
                    }
                    Button {
                        revealInFinder(item.path)
                    } label: {
                        Label("Reveal", systemImage: "finder")
                    }
                }
            } else {
                Text("Select an item to inspect.")
                    .foregroundStyle(.secondary)
                Spacer()
            }
        }
        .padding(14)
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
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
        .background(.quaternary.opacity(0.65), in: RoundedRectangle(cornerRadius: 8))
    }
}

struct StorageTreemap: View {
    let items: [AppScanItem]
    @Binding var selectedItem: AppScanItem?
    var onOpen: ((AppScanItem) -> Void)? = nil

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                RoundedRectangle(cornerRadius: 8)
                    .fill(.quaternary.opacity(0.32))
                if items.isEmpty {
                    EmptyPanelText("No map data")
                } else {
                    Canvas { context, size in
                        let rects = treemapRects(items: items, in: CGRect(origin: .zero, size: size))
                        for entry in rects {
                            let isSelected = entry.item.id == selectedItem?.id
                            let visibleRect = entry.rect.width > 4 && entry.rect.height > 4
                                ? entry.rect.insetBy(dx: 2, dy: 2)
                                : entry.rect
                            let path = Path(visibleRect)
                            context.fill(path, with: .color(entry.item.safety.color.opacity(isSelected ? 0.9 : 0.58)))
                            context.stroke(path, with: .color(isSelected ? .primary : .black.opacity(0.18)), lineWidth: isSelected ? 2 : 1)

                            if entry.rect.width > 90 && entry.rect.height > 42 {
                                let text = Text(entry.item.name)
                                    .font(.caption.bold())
                                    .foregroundColor(.primary)
                                context.draw(text, at: CGPoint(x: entry.rect.midX, y: entry.rect.midY))
                            }
                        }
                    }
                    .contentShape(Rectangle())
                    .gesture(
                        SpatialTapGesture()
                            .onEnded { value in
                                let rects = treemapRects(
                                    items: items,
                                    in: CGRect(origin: .zero, size: proxy.size)
                                )
                                guard let entry = rects.first(where: { $0.rect.contains(value.location) }) else {
                                    return
                                }
                                selectedItem = entry.item
                                if entry.item.isDir {
                                    onOpen?(entry.item)
                                }
                            }
                    )
                }
            }
        }
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
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(parts) { part in
                    Button {
                        onNavigate(part.path)
                    } label: {
                        Text(part.label)
                    }
                    .buttonStyle(.borderless)
                    if part.id != parts.last?.id {
                        Image(systemName: "chevron.right")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
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
