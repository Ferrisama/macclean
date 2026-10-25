import SwiftUI

/// A folder is a junction, and its children are branches. Thin bars compare each
/// branch with the folder's measured allocation; rows remain readable at any size.
struct CircuitNavigator: View {
    let folder: AppScanItem?
    let items: [AppScanItem]
    @Binding var selectedItem: AppScanItem?
    let selectedPaths: Set<String>
    let busy: Bool
    let onOpen: (AppScanItem) -> Void
    let onToggle: (AppScanItem) -> Void
    var onInspect: ((AppScanItem) -> Void)? = nil
    @State private var visibleCount = 100

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let folder {
                HStack(spacing: 14) {
                    Image(systemName: "cpu").font(.title2).foregroundStyle(AeroTheme.accent)
                        .frame(width: 28, height: 36)
                    VStack(alignment: .leading, spacing: 5) {
                        Text("CURRENT FOLDER").font(.caption2).tracking(1.4).foregroundStyle(.secondary)
                        Text(folder.name == "/" ? "Startup disk" : folder.name).font(.title3.weight(.medium))
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 5) {
                        Text(formatBytes(folder.sizeBytes)).font(.title3.monospacedDigit())
                        Text("\(items.count) direct \(items.count == 1 ? "item" : "items")").font(.caption).foregroundStyle(.secondary)
                    }
                }.padding(.bottom, 18)
            }
            HStack {
                Text("BRANCH").tracking(1.1)
                Spacer()
                Text("SIZE · % OF FOLDER").tracking(1.1)
            }.font(.caption2).foregroundStyle(.secondary).padding(.leading, 44).padding(.trailing, 78).padding(.bottom, 8)
            LazyVStack(spacing: 0) {
                ForEach(Array(items.prefix(visibleCount).enumerated()), id: \.element.id) { position, item in
                    branch(item, last: position == min(items.count, visibleCount) - 1)
                }
            }
            if items.count > visibleCount {
                Button("Show next \(min(100, items.count - visibleCount)) items · \(items.count - visibleCount) more loaded") { visibleCount += 100 }
                    .buttonStyle(.borderless).padding(.leading, 44).padding(.vertical, 16)
            }
            if items.isEmpty {
                Text(folder?.partial == true ? "No readable children were measured here. Coverage is incomplete." : "No matching children in this folder.")
                    .foregroundStyle(.secondary).padding(.vertical, 20).padding(.leading, 44)
            }
        }
        .onChange(of: folder?.path) { _ in visibleCount = 100 }
    }

    private func branch(_ item: AppScanItem, last: Bool) -> some View {
        let selected = selectedItem?.path == item.path
        let share = BranchAllocationShare(bytes: item.sizeBytes, folderBytes: folder?.sizeBytes)
        let shareHelp = "Bar length is this branch’s share of the current folder’s measured allocated bytes. Directory metadata and undisplayed items can account for the remainder. Partial scans are incomplete; these are not predicted cleanup savings."
        return HStack(spacing: 0) {
            CircuitTrace(last: last).stroke(selected ? AeroTheme.accent : AeroTheme.border, lineWidth: selected ? 1.5 : 1)
                .overlay(alignment: .center) {
                    Circle().fill(selected ? AeroTheme.accent : AeroTheme.border).frame(width: 5, height: 5).offset(x: -3)
                }
                .frame(width: 36)
                .accessibilityHidden(true)
            HStack(spacing: 12) {
                Button { selectedItem = item; onInspect?(item) } label: {
                    HStack(spacing: 12) {
                        Image(systemName: item.isDir ? "folder" : "doc")
                            .foregroundStyle(selected ? AeroTheme.accent : .secondary).frame(width: 20)
                        VStack(alignment: .leading, spacing: 5) {
                            Text(item.name).font(.body.weight(selected ? .medium : .regular)).lineLimit(1)
                            Text(item.partial ? "Incomplete measurement" : item.isDir ? "Folder · open to follow this branch" : "File · inspect in Finder")
                                .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            GeometryReader { geometry in
                                Capsule().fill(AeroTheme.border.opacity(0.6))
                                if let fraction = share.fraction {
                                    Capsule().fill(selected ? AeroTheme.accent : Color.secondary.opacity(0.7))
                                        .frame(width: geometry.size.width * fraction)
                                }
                            }.frame(height: 4).accessibilityHidden(true)
                        }
                        Spacer(minLength: 12)
                        if item.partial { Image(systemName: "exclamationmark.triangle").foregroundStyle(.orange) }
                        VStack(alignment: .trailing, spacing: 5) {
                            Text(formatBytes(item.sizeBytes)).font(.body.monospacedDigit())
                            Text(share.label).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                                .accessibilityLabel(share.fraction == nil ? "Folder share unavailable" : "\(share.label) of measured folder allocation")
                        }.frame(width: 110, alignment: .trailing).help(shareHelp)
                    }.contentShape(Rectangle())
                }.buttonStyle(.plain).help(item.path).accessibilityIdentifier("explore.inspect.\(item.path)")
                if item.canMoveToTrash {
                    Button { onToggle(item) } label: {
                        Image(systemName: selectedPaths.contains(item.path) ? "checkmark.circle.fill" : "plus.circle")
                    }.buttonStyle(.borderless).disabled(busy).frame(width: 24)
                        .accessibilityLabel("Toggle \(item.name) in review")
                } else { Color.clear.frame(width: 24, height: 1).accessibilityHidden(true) }
                if item.isDir {
                    Button { onOpen(item) } label: { Image(systemName: "arrow.right") }
                        .buttonStyle(.borderless).disabled(busy).frame(width: 24)
                        .accessibilityLabel("Open \(item.name)").accessibilityIdentifier("explore.open.\(item.path)")
                } else { Color.clear.frame(width: 24, height: 1).accessibilityHidden(true) }
            }
            .padding(.leading, 8).padding(.trailing, 4)
            .padding(.vertical, 14)
            .overlay(alignment: .bottom) { Rectangle().fill(AeroTheme.border.opacity(0.5)).frame(height: 0.5) }
        }
        .frame(minHeight: 70)
    }
}

private struct CircuitTrace: Shape {
    let last: Bool
    func path(in rect: CGRect) -> Path {
        var path = Path()
        let trunk: CGFloat = 15
        path.move(to: CGPoint(x: trunk, y: rect.minY))
        path.addLine(to: CGPoint(x: trunk, y: last ? rect.midY : rect.maxY))
        path.move(to: CGPoint(x: trunk, y: rect.midY))
        path.addLine(to: CGPoint(x: rect.maxX, y: rect.midY))
        return path
    }
}
