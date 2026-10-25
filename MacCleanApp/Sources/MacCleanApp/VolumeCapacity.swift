import Foundation
import SwiftUI

struct VolumeCapacity: Sendable {
    let name: String
    let volumePath: String
    let totalBytes: UInt64
    let availableBytes: UInt64
    let measuredAt: Date

    init(name: String, volumePath: String, totalBytes: Int64, availableBytes: Int64,
         measuredAt: Date = Date()) throws {
        guard totalBytes > 0, availableBytes >= 0, availableBytes <= totalBytes else {
            throw CocoaError(.fileReadCorruptFile)
        }
        self.name = name
        self.volumePath = volumePath
        self.totalBytes = UInt64(totalBytes)
        self.availableBytes = UInt64(availableBytes)
        self.measuredAt = measuredAt
    }

    var occupiedBytes: UInt64 { totalBytes - availableBytes }
    var occupiedFraction: Double { Double(occupiedBytes) / Double(totalBytes) }

    static func read(at path: String) throws -> VolumeCapacity {
        let values = try URL(fileURLWithPath: path).resourceValues(forKeys: [
            .volumeNameKey, .volumeURLKey, .volumeTotalCapacityKey, .volumeAvailableCapacityKey
        ])
        guard let total = values.volumeTotalCapacity, let available = values.volumeAvailableCapacity,
              let volumeURL = values.volume else { throw CocoaError(.fileReadUnknown) }
        return try VolumeCapacity(name: values.volumeName ?? volumeURL.path, volumePath: volumeURL.path,
                                  totalBytes: Int64(total), availableBytes: Int64(available))
    }
}

struct VolumeCapacityRequest: Equatable {
    let path: String
    let scanning: Bool
    let cleaning: Bool
    let restoring: Bool
}

extension AppModel {
    var volumeCapacityRequest: VolumeCapacityRequest {
        VolumeCapacityRequest(path: directoryNavigation.currentPath, scanning: isScanning,
                              cleaning: isCleaning, restoring: isRestoring)
    }

    func refreshVolumeCapacity() async {
        let ticket = UUID()
        volumeCapacityTicket = ticket
        let request = volumeCapacityRequest
        guard !request.cleaning, !request.restoring else {
            isLoadingVolumeCapacity = false
            return
        }
        if volumeCapacitySourcePath != request.path { volumeCapacity = nil }
        volumeCapacitySourcePath = request.path
        volumeCapacityError = nil
        isLoadingVolumeCapacity = true
        defer { if volumeCapacityTicket == ticket { isLoadingVolumeCapacity = false } }
        let reader = capacityReader
        let result = await Task.detached(priority: .utility) {
            Result { try reader(request.path) }
        }.value
        guard !Task.isCancelled, volumeCapacityTicket == ticket, volumeCapacityRequest == request else { return }
        switch result {
        case .success(let value): volumeCapacity = value
        case .failure:
            volumeCapacity = nil
            volumeCapacityError = "Volume capacity is unavailable for this location."
        }
    }
}

struct VolumeCapacityStrip: View {
    @ObservedObject var model: AppModel
    @State private var showsDetails = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let volume = model.volumeCapacity {
                HStack {
                    Label(volume.name, systemImage: "internaldrive").font(.callout.weight(.medium))
                    Button { showsDetails = true } label: { Image(systemName: "info.circle") }
                        .buttonStyle(.borderless).accessibilityLabel("How volume capacity is measured")
                        .popover(isPresented: $showsDetails) {
                            Text("macOS reports capacity for this volume, including space outside the scanned folder. Used / unavailable includes reserved space. APFS volumes can share capacity; purgeable space and snapshots can make Finder show different available space. A folder scan is not a whole-disk accounting report.")
                                .font(.callout).padding(16).frame(width: 330)
                        }
                    Spacer()
                    Text("\(formatBytes(volume.availableBytes)) available").monospacedDigit()
                }
                GeometryReader { geometry in
                    Capsule().fill(AeroTheme.border)
                    Capsule().fill(AeroTheme.accent.opacity(0.75))
                        .frame(width: geometry.size.width * volume.occupiedFraction)
                }.frame(height: 5)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("Volume capacity")
                    .accessibilityValue("\(formatBytes(volume.occupiedBytes)) used or unavailable, \(formatBytes(volume.availableBytes)) available, \(formatBytes(volume.totalBytes)) total")
                HStack {
                    Text("\(formatBytes(volume.occupiedBytes)) used / unavailable · \(formatBytes(volume.totalBytes)) total")
                    Spacer()
                    Button { Task { await model.refreshVolumeCapacity() } } label: { Image(systemName: "arrow.clockwise") }
                        .buttonStyle(.borderless).accessibilityLabel("Refresh volume capacity")
                        .disabled(model.isLoadingVolumeCapacity || model.isCleaning || model.isRestoring)
                }.font(.caption).foregroundStyle(.secondary)
            } else {
                HStack {
                    Label(model.volumeCapacityError ?? "Reading volume capacity…",
                          systemImage: model.volumeCapacityError == nil ? "internaldrive" : "exclamationmark.triangle")
                        .font(.caption).foregroundStyle(.secondary)
                    if model.volumeCapacityError != nil {
                        Button("Retry") { Task { await model.refreshVolumeCapacity() } }
                    }
                }
            }
            if let scan = model.scan {
                Text("Scanned scope: \(scan.rootScan.root) · \(formatBytes(scan.rootScan.tree.sizeBytes))\(scan.rootScan.partial ? " measured · incomplete" : " measured")")
                    .font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                    .lineLimit(1).truncationMode(.middle).help(scan.rootScan.root)
            }
        }.accessibilityIdentifier("explore.volumeCapacity")
    }
}
