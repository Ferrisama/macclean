import AppKit
import SwiftUI

enum AppTab: String, CaseIterable, Identifiable {
    case map, history
    var id: String { rawValue }
    var title: String { self == .map ? "Explore" : "History" }
}

enum ExplorerFilter: String, CaseIterable, Identifiable {
    case all = "All storage", opportunities = "Opportunities", caches = "Caches", apps = "App activity", folders = "Folder activity"
    var id: String { rawValue }
}

struct RootView: View {
    @StateObject private var model: AppModel
    private let loadsStartupData: Bool
    @State private var settings = false
    @State private var review = false
    @State private var filter: ExplorerFilter = .all
    @MainActor init(model: AppModel? = nil, loadsStartupData: Bool = true) {
        _model = StateObject(wrappedValue: model ?? AppModel())
        self.loadsStartupData = loadsStartupData
    }
    var body: some View {
        VStack(spacing: 0) {
            chrome
            Divider()
            if model.selectedTab == .history {
                HistoryView(model: model)
            } else {
                ExploreWorkspace(model: model, filter: $filter)
            }
            if !model.selectedCleanupPaths.isEmpty {
                Divider()
                HStack(spacing: 14) {
                    Label("\(model.selectedCleanupPaths.count) selected", systemImage: "tray")
                    Text(formatBytes(model.collectionBytes)).monospacedDigit().foregroundStyle(.secondary)
                    Spacer()
                    Button("Clear", action: model.clearCleanupSelection).disabled(model.busy)
                    Button("Review selection") { review = true }
                        .buttonStyle(.borderedProminent).disabled(model.busy)
                        .accessibilityIdentifier("cleanup.review")
                }.padding(.horizontal, 20).padding(.vertical, 12).background(AeroTheme.surface)
            }
        }
        .background(AeroTheme.background).tint(AeroTheme.accent).preferredColorScheme(.dark)
        .sheet(isPresented: $settings) { settingsPanel }
        .sheet(isPresented: $review) { CleanupReviewSheet(model: model) }
        .task { if loadsStartupData { model.loadStartupData() } }
    }
    private var chrome: some View {
        VStack(spacing: 12) {
            HStack(spacing: 18) {
                HStack(spacing: 0) { Text("mac"); Text("clean").foregroundStyle(AeroTheme.accent) }
                    .font(.title3.weight(.semibold)).accessibilityLabel("MacClean")
                Picker("Workspace", selection: $model.selectedTab) {
                    ForEach(AppTab.allCases) { Text($0.title).tag($0) }
                }.pickerStyle(.segmented).labelsHidden().frame(width: 190).disabled(model.isCleaning || model.isRestoring)
                Spacer()
                Menu {
                    if model.recentFolders.isEmpty { Text("No recent scans") }
                    ForEach(model.recentFolders) { folder in
                        Button(folder.path) { model.openRecentFolder(folder); filter = .all; model.selectedTab = .map }
                    }
                    Divider()
                    Button("Clear recent folders", action: model.clearRecentFolders)
                } label: { Label("Recent", systemImage: "clock") }.disabled(model.busy)
                Button { settings = true } label: { Image(systemName: "gearshape") }
                    .accessibilityLabel("Settings and access")
            }
            if model.selectedTab == .map {
                HStack(spacing: 8) {
                    Image(systemName: "folder").foregroundStyle(.secondary)
                    TextField("Folder to scan", text: $model.path).textFieldStyle(.roundedBorder)
                        .onSubmit { model.startRootScan() }.disabled(model.busy)
                        .accessibilityIdentifier("toolbar.path")
                    Button { model.chooseScanFolder() } label: { Image(systemName: "folder.badge.plus") }
                        .disabled(model.busy).accessibilityLabel("Choose folder").accessibilityIdentifier("toolbar.chooseFolder")
                    Button { filter = .all; model.startRootScan(fast: false) } label: { Label("Scan folder", systemImage: "magnifyingglass") }
                        .buttonStyle(.borderedProminent).disabled(model.busy).accessibilityIdentifier("toolbar.deepScan")
                }
            }
        }.padding(.horizontal, 20).padding(.vertical, 14).background(AeroTheme.surface)
    }
    private var settingsPanel: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack { Text("Access & privacy").font(.title2); Spacer(); Button("Done") { settings = false } }
            Text("Scans stay on this Mac. Nothing is scanned automatically at startup. Recent folders record scans in MacClean, not your system-wide activity.")
                .foregroundStyle(.secondary)
            Text(model.fullDiskAccessStatus.title).font(.headline)
            Text(model.fullDiskAccessStatus.detail).foregroundStyle(.secondary)
            HStack {
                Button("Open System Settings", action: model.openFullDiskAccessSettings)
                Button("Check access", action: model.checkFullDiskAccess)
            }
            Text("Cleanup moves reviewed items to Trash. Emptying Trash is a separate action in Finder.").foregroundStyle(.secondary)
        }.padding(24).frame(width: 530).background(AeroTheme.background).preferredColorScheme(.dark)
    }
}

struct ExploreWorkspace: View {
    @ObservedObject var model: AppModel
    @Binding var filter: ExplorerFilter
    @State private var search = ""
    @State private var usage: ItemUsage?
    @State private var compactInspector: AppScanItem?
    @State private var compactLayout = true
    @State private var visibleOpportunityCount = 100
    private struct SearchViewRequest: Equatable {
        let filter: ExplorerFilter
        let query: InventorySearchRequest
    }
    private var searchRequest: SearchViewRequest {
        SearchViewRequest(filter: filter, query: model.inventorySearchRequest)
    }
    private var items: [AppScanItem] {
        let source: [AppScanItem]
        switch filter {
        case .all: return model.displayScan.map { model.mapItems(in: $0) } ?? []
        case .opportunities: source = model.opportunities
        case .caches: source = model.opportunities.filter { $0.cleanKind.lowercased() == "cache" }
        case .apps, .folders: source = []
        }
        return source.filter { search.isEmpty || $0.name.localizedCaseInsensitiveContains(search) || $0.path.localizedCaseInsensitiveContains(search) }
    }
    var body: some View {
        GeometryReader { geometry in
            let opportunityItems = (filter == .caches || filter == .opportunities) ? items : []
            HStack(spacing: 0) {
                WorkspacePage {
                    heading
                    if model.isScanning || model.isLoadingRecipes {
                        HStack { ProgressView().controlSize(.small); Text(model.isLoadingRecipes ? "Measuring known cache folders…" : model.scanStage); Spacer(); if model.isScanning { Button("Stop", action: model.cancelScan) } else { Button("Stop", action: model.cancelCacheLookup) } }
                            .font(.callout).foregroundStyle(.secondary)
                    }
                    if let error = model.errorMessage {
                        Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.orange).textSelection(.enabled)
                    }
                    if let message = model.cleanupMessage {
                        Text(message).font(.callout).foregroundStyle(.secondary).textSelection(.enabled)
                    }
                    if !model.cleanupOutcomes.isEmpty {
                        DisclosureGroup("Cleanup results (\(model.cleanupOutcomes.count))") {
                            ForEach(model.cleanupOutcomes) { outcome in
                                VStack(alignment: .leading, spacing: 4) {
                                    Label(outcome.path, systemImage: outcome.moved ? "checkmark.circle" : "exclamationmark.circle")
                                        .font(.caption).textSelection(.enabled)
                                    if let error = outcome.error { Text(error).font(.caption).foregroundStyle(.orange) }
                                }.padding(.vertical, 4)
                            }
                        }
                    }
                    if let scan = model.displayScan, scan.rootScan.partial {
                        Label(scan.rootScan.incompleteReason ?? "Incomplete coverage; totals may be understated.", systemImage: "exclamationmark.triangle")
                            .font(.caption).foregroundStyle(.orange)
                    }
                    if filter == .apps || filter == .folders { ApplicationActivityView(model: model, folders: filter == .folders).id(filter) }
                    ForEach(model.cacheWarnings, id: \.self) { warning in
                        Text(warning).font(.caption).foregroundStyle(.orange)
                    }
                    if filter == .caches, !opportunityItems.isEmpty {
                        HStack {
                            Text("Add safe caches applies to all matching rows, including those not shown. Each path is checked again before cleanup.").font(.caption).foregroundStyle(.secondary)
                            Spacer()
                            Button("Add safe caches") { model.addSafeCaches(from: opportunityItems) }.disabled(model.busy)
                        }
                    }
                    if filter == .all, model.isInventorySearchActive {
                        inventoryResults
                    } else if filter == .all, model.displayScan != nil {
                        CircuitNavigator(folder: model.currentFolder, items: items, selectedItem: $model.selectedItem,
                                         selectedPaths: model.selectedCleanupPaths, busy: model.busy,
                                         onOpen: model.open, onToggle: model.toggleCleanup,
                                         onInspect: { item in
                                             usage = nil
                                             if geometry.size.width < 1050 { compactInspector = item }
                                         })
                    }
                    if filter == .caches || filter == .opportunities {
                    HStack {
                        Text(filter == .all ? "Folders & files" : "Reviewable paths").font(.headline)
                        Spacer(); Text("\(opportunityItems.count.formatted()) items · showing \(min(visibleOpportunityCount, opportunityItems.count).formatted())").foregroundStyle(.secondary)
                    }
                    }
                    if filter == .apps || filter == .folders { EmptyView() } else if filter == .all {
                        if model.displayScan == nil { emptyState }
                    } else if opportunityItems.isEmpty {
                        emptyState
                    } else {
                        LazyVStack(spacing: 2) {
                            ForEach(opportunityItems.prefix(visibleOpportunityCount)) { item in itemRow(item) }
                        }
                        if opportunityItems.count > visibleOpportunityCount {
                            Button("Show next \(min(100, opportunityItems.count - visibleOpportunityCount)) items") { visibleOpportunityCount += 100 }
                                .accessibilityIdentifier("explore.moreOpportunities")
                        }
                    }
                }
                if geometry.size.width >= 1050, filter != .apps, filter != .folders {
                    Divider()
                    ScrollView {
                        if let item = model.selectedItem { inspector(item).padding(20) }
                        else { Text("Select a folder or file to inspect its size, cleanup evidence, and activity.").foregroundStyle(.secondary).padding(20) }
                    }.frame(width: 270).background(AeroTheme.surface)
                }
            }
            .onAppear { compactLayout = geometry.size.width < 1050 }
            .onChange(of: geometry.size.width) { compactLayout = $0 < 1050 }
        }
        .sheet(item: $compactInspector) { item in
            VStack(spacing: 0) {
                HStack { Text("Folder & file details").font(.headline); Spacer(); Button("Done") { compactInspector = nil } }.padding(20)
                Divider()
                ScrollView { inspector(item).padding(20) }
            }.frame(width: 510, height: 520).background(AeroTheme.background).preferredColorScheme(.dark)
        }
        .onChange(of: filter) { _ in visibleOpportunityCount = 100 }
        .onChange(of: search) { _ in visibleOpportunityCount = 100 }
        .onChange(of: model.loadedTree.map(ObjectIdentifier.init)) { _ in visibleOpportunityCount = 100 }
        .task(id: searchRequest) {
            if filter == .all { await model.searchInventory() }
        }
        .task(id: model.volumeCapacityRequest) { await model.refreshVolumeCapacity() }
        .onChange(of: model.inventorySearchQuery) { _ in model.inventorySearchLimit = 100 }
        .onChange(of: model.inventorySearchKind) { _ in model.inventorySearchLimit = 100 }
        .onChange(of: model.inventorySearchSize) { _ in model.inventorySearchLimit = 100 }
        .task(id: model.selectedItem?.path) {
            usage = nil
            if let item = model.selectedItem {
                let result = await model.inspectUsage(item)
                guard !Task.isCancelled, model.selectedItem?.path == item.path else { return }
                usage = result
            }
        }
    }
    private var heading: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 8) {
                Button(action: model.goBack) { Image(systemName: "chevron.left") }.disabled(!model.directoryNavigation.canGoBack || model.busy).accessibilityLabel("Back")
                Button(action: model.goForward) { Image(systemName: "chevron.right") }.disabled(!model.directoryNavigation.canGoForward || model.busy).accessibilityLabel("Forward")
                Button(action: model.goUp) { Image(systemName: "arrow.up") }.disabled(model.directoryNavigation.isAtRoot || model.busy).accessibilityLabel("Parent folder")
                BreadcrumbBar(path: model.path, onNavigate: model.navigate).disabled(model.busy)
            }.buttonStyle(.borderless)
            HStack(alignment: .firstTextBaseline) {
                Text(filter == .caches ? "Caches" : filter == .all ? "Follow the space." : filter.rawValue)
                    .font(.system(size: 26, weight: .medium))
                Spacer()

            }
            VolumeCapacityStrip(model: model)
            if let scan = model.scan, filter == .all {
                HStack(spacing: 8) {
                    if let tree = model.loadedTree {
                        Label("Tree loaded", systemImage: "point.3.connected.trianglepath.dotted")
                        Text("\(tree.folderCount.formatted()) folders · \(tree.fileCount.formatted()) files")
                    } else { Text("Overview only · scan to load the complete tree") }
                    Spacer()
                    Text(Date(timeIntervalSince1970: TimeInterval(scan.rootScan.scannedAt)), style: .time)
                }.font(.caption).foregroundStyle(.secondary)
            }
            HStack(spacing: 10) {
                Picker("Show", selection: $filter) { ForEach(ExplorerFilter.allCases) { Text($0.rawValue).tag($0) } }
                    .pickerStyle(.menu).labelsHidden().frame(width: 190)
                Spacer(minLength: 0)
                Button { filter = .caches; model.loadRecipes() } label: { Label("Find caches", systemImage: "arrow.clockwise") }
                    .disabled(model.busy).accessibilityIdentifier("explore.findCaches")
            }
            if filter == .all {
                HStack {
                    TextField("Search names or paths across the scanned tree", text: $model.inventorySearchQuery)
                        .textFieldStyle(.roundedBorder).accessibilityIdentifier("explore.search")
                    if model.isInventorySearchActive {
                        Button("Clear", action: model.clearInventorySearch)
                    }
                    Menu {
                        Picker("Kind", selection: $model.inventorySearchKind) {
                            ForEach(InventorySearchKind.allCases) { Text($0.rawValue).tag($0) }
                        }
                        Picker("Allocated size", selection: $model.inventorySearchSize) {
                            ForEach(InventorySearchSize.allCases) { Text($0.title).tag($0) }
                        }
                    } label: {
                        Label(model.inventorySearchKind == .all && model.inventorySearchSize == .any ? "Filters" : "Filters active",
                              systemImage: "line.3.horizontal.decrease")
                    }.fixedSize().disabled(model.loadedTree == nil || model.isScanning)
                }.disabled(model.loadedTree == nil || model.isScanning)
            } else {
                TextField("Filter this list by name or path", text: $search).textFieldStyle(.roundedBorder)
            }
            if filter == .caches || filter == .opportunities {
                Text("Find caches measures known cache locations in your home folder. A full folder scan also discovers eligible project output. Nothing is preselected.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
    }
    private var inventoryResults: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let tree = model.loadedTree {
                Text("Search scope: \(tree.inventory.root)")
                    .font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                Text("Results use the loaded scan, largest allocated size first. Files and folders can overlap in size; results are not a reclaimable total.")
                    .font(.caption).foregroundStyle(.secondary)
                if model.isSearchingInventory {
                    HStack { ProgressView().controlSize(.small); Text("Searching loaded tree…") }
                } else if let page = model.inventorySearchPage {
                    Text("\(page.totalCount.formatted()) matches · showing \(page.items.count.formatted())")
                        .font(.callout).accessibilityIdentifier("explore.searchCount")
                    if page.totalCount == 0 {
                        Text("No matches in this scanned scope. Try a different name, path, kind, or size.").foregroundStyle(.secondary)
                    }
                    LazyVStack(spacing: 2) {
                        ForEach(page.items) { item in itemRow(item, searchResult: true) }
                    }
                    if page.totalCount > page.items.count {
                        Button("Show next 100 results") { model.inventorySearchLimit += 100 }
                    }
                }
            } else {
                Text("Complete a folder scan to search its tree.").foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder private var emptyState: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(model.busy ? "Results appear as they are measured." : filter == .all ? "Choose a folder to explore, or find known caches." : "No matching measured caches. Choose Find caches to check known locations.")
                .foregroundStyle(.secondary)
            if filter == .all, model.scan == nil, !model.recentFolders.isEmpty {
                Text("Recently scanned").font(.headline)
                ForEach(model.recentFolders) { folder in
                    Button { model.openRecentFolder(folder) } label: {
                        HStack { Text(folder.path).lineLimit(1).truncationMode(.middle); Spacer(); Text(folder.scannedAt, style: .relative).foregroundStyle(.secondary) }
                    }.buttonStyle(.plain).disabled(model.busy)
                }
            }
        }.padding(.vertical, 16)
    }
    private func itemRow(_ item: AppScanItem, searchResult: Bool = false) -> some View {
        HStack(spacing: 12) {
            Button {
                usage = nil
                model.selectedItem = item
                if compactLayout { compactInspector = item }
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: item.isDir ? "folder" : "doc").foregroundStyle(.secondary)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(item.name).lineLimit(1)
                        Text(item.path).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                    }
                    Spacer()
                    Text(formatBytes(item.sizeBytes)).monospacedDigit()
                    if item.partial { Image(systemName: "exclamationmark.triangle").foregroundStyle(.orange).accessibilityLabel("Partial size") }
                }.contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier("explore.inspect.\(item.path)")
            if item.canMoveToTrash {
                Button { model.toggleCleanup(item) } label: { Image(systemName: model.selectedCleanupPaths.contains(item.path) ? "checkmark.circle.fill" : "plus.circle") }
                    .buttonStyle(.borderless).disabled(model.busy)
                    .accessibilityLabel(model.selectedCleanupPaths.contains(item.path) ? "Remove \(item.name) from review" : "Add \(item.name) to review")
                    .accessibilityIdentifier("cleanup.select.\(item.path)")
            }
            if searchResult {
                Button { model.openSearchResult(item, containingFolder: true) } label: {
                    Image(systemName: "folder")
                }.buttonStyle(.borderless).disabled(model.busy)
                    .accessibilityLabel("Show \(item.name) in its containing folder")
                    .help("Show in containing folder")
                    .accessibilityIdentifier("explore.showInFolder.\(item.path)")
            }
            if item.isDir {
                Button {
                    if searchResult { model.openSearchResult(item) } else { model.open(item) }
                    filter = .all
                } label: { Image(systemName: "arrow.right") }
                    .buttonStyle(.borderless).disabled(model.busy).accessibilityLabel("Open \(item.name)")
            }
        }.padding(12).background(model.selectedItem?.path == item.path ? AeroTheme.selection : Color.clear, in: RoundedRectangle(cornerRadius: 6))
    }
    private func inspector(_ item: AppScanItem) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("SELECTED ITEM").font(.caption2).foregroundStyle(.secondary)
            Text(item.name).font(.title3).textSelection(.enabled)
            Text(item.path).font(.caption.monospaced()).foregroundStyle(.secondary).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
            Text(formatBytes(item.sizeBytes)).font(.system(size: 30, weight: .medium, design: .rounded)).monospacedDigit()
            Text(item.logicalSizeBytes == nil ? "Size estimate" : item.partial ? "Allocated bytes · incomplete" : "Allocated bytes attributed")
                .font(.caption).foregroundStyle(.secondary)
            if let logical = item.logicalSizeBytes {
                LabeledContent("Logical bytes attributed", value: formatBytes(logical)).font(.caption)
                Text(item.hardLinkDuplicate == true
                     ? "This is another hard link to a file already counted in this scan. Its bytes are attributed to the first observed link."
                     : "Hard links count once across this scan. Allocation includes directory metadata and does not predict space recovered after cleanup.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Divider()
            Label(item.canMoveToTrash ? "Eligible for review" : "Inspect in Finder", systemImage: item.canMoveToTrash ? "checkmark.shield" : "lock")
            Text(item.cleanupReason).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            if let usage {
                Divider()
                if let modified = usage.modified { LabeledContent("Modified", value: modified.formatted(date: .abbreviated, time: .shortened)) }
                LabeledContent("Last opened", value: usage.lastOpened ?? "Unknown")
                if let app = usage.appName {
                    LabeledContent("Owner", value: app)
                    LabeledContent("App last opened", value: usage.appLastOpened ?? "Unknown")
                    LabeledContent("Running now", value: usage.appRunning == true ? "Yes — quit before cleanup" : "No matching app")
                }
                Text("Last opened is Spotlight metadata when available. Modified time is not last use. Neither proves an item is unused.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Divider()
            if item.canMoveToTrash {
                Text("This data may need to be downloaded or rebuilt. Open files are checked again before cleanup.").font(.caption).foregroundStyle(.secondary)
                Button(model.selectedCleanupPaths.contains(item.path) ? "Remove from review" : "Add to review") { model.toggleCleanup(item) }
                    .buttonStyle(.borderedProminent).disabled(model.busy)
            }
            Button("Reveal in Finder") { revealInFinder(item.path) }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct CleanupReviewSheet: View {
    @ObservedObject var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var validated = false
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Make room. Keep a way back.").font(.title2)
            Text("Review \(model.collection.count) paths · \(formatBytes(model.collectionBytes)). Items move to Trash; space is freed when Trash is emptied.")
                .foregroundStyle(.secondary)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 16) {
                    ForEach(model.collection) { item in
                        VStack(alignment: .leading, spacing: 5) {
                            HStack { Text(item.name).font(.headline); Spacer(); Text(formatBytes(item.sizeBytes)).monospacedDigit() }
                            Text(item.path).font(.caption.monospaced()).textSelection(.enabled)
                            Text(item.cleanupReason).font(.callout).foregroundStyle(.secondary)
                        }
                        Divider()
                    }
                    ForEach(model.cleanupOutcomes.filter { $0.error != nil }) { outcome in
                        Text("\(outcome.path): \(outcome.error ?? "")").foregroundStyle(.orange).textSelection(.enabled)
                    }
                }
            }
            if let message = model.cleanupMessage { Text(message).font(.callout).foregroundStyle(.secondary).textSelection(.enabled) }
            HStack {
                Button("Cancel") { dismiss() }.disabled(model.isCleaning)
                Spacer()
                if validated {
                    Button("Move to Trash", role: .destructive) { model.cleanSelected(); dismiss() }
                        .buttonStyle(.borderedProminent).disabled(model.busy || model.selectedCleanupPaths.isEmpty)
                } else {
                    Button("Check selected paths") { model.preflightCleanup { validated = $0 } }
                        .buttonStyle(.borderedProminent).disabled(model.busy || model.selectedCleanupPaths.isEmpty)
                }
            }
        }.padding(24).frame(width: 620, height: 530).background(AeroTheme.background).preferredColorScheme(.dark)
        .interactiveDismissDisabled(model.isCleaning)
        .onChange(of: model.selectedCleanupPaths) { _ in validated = false }
    }
}

struct ApplicationActivityView: View {
    @ObservedObject var model: AppModel
    var folders = false
    @State private var olderThan = 90
    private var entries: [ApplicationActivity] {
        (folders ? model.folderActivity : model.appActivity).filter { app in
            if olderThan == -1 { return app.lastOpened == nil }
            guard let opened = app.lastOpened else { return olderThan == 0 }
            return olderThan == 0 || (!app.running && opened < Date().addingTimeInterval(-Double(olderThan) * 86_400))
        }.sorted { ($0.lastOpened ?? .distantFuture) < ($1.lastOpened ?? .distantFuture) }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Picker("Last opened", selection: $olderThan) {
                    Text(folders ? "All folders" : "All apps").tag(0)
                    Text("Over 90 days").tag(90)
                    Text("Over 180 days").tag(180)
                    Text("Unknown").tag(-1)
                }.frame(maxWidth: 260)
                Spacer()
                Button(folders ? "Check folder activity" : "Check app activity") { Task { if folders { await model.loadFolderActivity() } else { await model.loadAppActivity() } } }.disabled(model.busy)
            }
            Text(folders ? "Checks folders visible in the current scan, including project folders. Last-opened dates come from Spotlight and may be absent or stale. Age alone never authorizes cleanup." : "On-demand inventory of apps directly in /Applications and ~/Applications. Dates come from Spotlight and may be absent or stale. This is read-only; an old date does not prove an app is unused.")
                .font(.callout).foregroundStyle(.secondary)
            if model.isFindingApps { ProgressView("Reading activity metadata…") }
            if entries.isEmpty, !model.isFindingApps { Text("No matching activity. Run a check or change the age filter. Folder activity requires a folder scan first.").foregroundStyle(.secondary) }
            ForEach(entries) { app in
                HStack {
                    Image(systemName: folders ? "folder" : "app").foregroundStyle(.secondary)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(app.name)
                        Text(app.path).font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    if app.running { Text("Running").foregroundStyle(.orange) }
                    Text(app.lastOpened?.formatted(date: .abbreviated, time: .omitted) ?? "Unknown").monospacedDigit()
                    Button("Reveal") { revealInFinder(app.path) }
                }.padding(.vertical, 8)
                Divider()
            }
        }
    }
}
