import SwiftUI

/// A page owns one vertical scroll view; its cards size to their content.
struct WorkspacePage<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        ScrollView(.vertical) {
            VStack(alignment: .leading, spacing: 20) {
                content
            }
            .frame(maxWidth: .infinity, alignment: .topLeading)
            .padding(20)
        }
        .scrollIndicators(.visible)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier("workspace.scroll")
    }
}

struct WorkspaceHeading: View {
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.system(size: 26, weight: .semibold))
            Text(subtitle).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct WorkspaceRowModifier: ViewModifier {
    let selected: Bool

    func body(content: Content) -> some View {
        content
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(selected ? Color.accentColor.opacity(0.08) : Color(nsColor: .controlBackgroundColor),
                        in: RoundedRectangle(cornerRadius: 10))
            .overlay {
                RoundedRectangle(cornerRadius: 10)
                    .strokeBorder(selected ? Color.accentColor.opacity(0.4) : Color.primary.opacity(0.06), lineWidth: 1)
                    .allowsHitTesting(false)
            }
    }
}

extension View {
    func workspaceRow(selected: Bool = false) -> some View {
        modifier(WorkspaceRowModifier(selected: selected))
    }
}
