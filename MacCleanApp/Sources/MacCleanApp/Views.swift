import AppKit
import SwiftUI

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
                            Text(Date(timeIntervalSince1970: TimeInterval(session.timestamp)), style: .date)
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
                        .disabled(session.restorableCount == 0 || model.busy)
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
            Button("Restore") {
                model.restore(session)
            }
            Button("Cancel", role: .cancel) {}
        } message: { session in
            Text("macclean will move restorable Trash items back to their original paths for \(session.sessionId).")
        }
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
                        for entry in rects {
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
                            let fill = isSelected ? AeroTheme.selection : (isHovered ? AeroTheme.raised : AeroTheme.tile)
                            context.fill(path, with: .color(fill))
                            context.stroke(path, with: .color(isSelected ? AeroTheme.accent : AeroTheme.border), lineWidth: isSelected ? 2 : 1)

                            if entry.rect.width > 66 && entry.rect.height > 42 {
                                let characters = max(5, Int((entry.rect.width - 16) / 7))
                                let name = entry.item.name.count > characters
                                    ? String(entry.item.name.prefix(characters - 1)) + "…" : entry.item.name
                                let text = Text("\(name)\n\(formatBytes(entry.item.sizeBytes))")
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
        let rowWidth = area / rect.height
        var y = rect.minY
        for (offset, entry) in row.enumerated() {
            let height = offset == row.count - 1 ? rect.maxY - y : entry.1 / rowWidth
            result.append(TreemapEntry(item: entry.0, rect: CGRect(x: rect.minX, y: y, width: rowWidth, height: height)))
            y += height
        }
        return CGRect(x: rect.minX + rowWidth, y: rect.minY, width: max(0, rect.width - rowWidth), height: rect.height)
    }
    let rowHeight = area / rect.width
    var x = rect.minX
    for (offset, entry) in row.enumerated() {
        let width = offset == row.count - 1 ? rect.maxX - x : entry.1 / rowHeight
        result.append(TreemapEntry(item: entry.0, rect: CGRect(x: x, y: rect.minY, width: width, height: rowHeight)))
        x += width
    }
    return CGRect(x: rect.minX, y: rect.minY + rowHeight, width: rect.width, height: max(0, rect.height - rowHeight))
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


func revealInFinder(_ path: String) { NSWorkspace.shared.selectFile(path, inFileViewerRootedAtPath: "") }
