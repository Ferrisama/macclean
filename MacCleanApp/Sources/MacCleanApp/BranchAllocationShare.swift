import Foundation

/// A branch's contribution to its current folder's observed allocation.
/// An absent/zero/inconsistent denominator cannot establish a percentage.
struct BranchAllocationShare {
    let fraction: Double?

    init(bytes: UInt64, folderBytes: UInt64?) {
        guard let folderBytes, folderBytes > 0, bytes <= folderBytes else {
            fraction = nil
            return
        }
        fraction = Double(bytes) / Double(folderBytes)
    }

    var label: String {
        guard let fraction else { return "—" }
        if fraction > 0 && fraction < 0.001 { return "<0.1%" }
        return fraction.formatted(.percent.precision(.fractionLength(0...1)))
    }
}
