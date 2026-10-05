import SwiftUI

enum AeroTheme {
    static let panelRadius: CGFloat = 12
    static let controlRadius: CGFloat = 8
    static let tileRadius: CGFloat = 8
    static let spacing: CGFloat = 16
}

struct AeroBackdrop: View {
    var body: some View {
        Color(nsColor: .windowBackgroundColor)
            .ignoresSafeArea()
            .allowsHitTesting(false)
    }
}

extension View {
    func aeroPanel(cornerRadius: CGFloat = AeroTheme.panelRadius, padding: CGFloat = AeroTheme.spacing) -> some View {
        self.padding(padding).aeroSurface(cornerRadius: cornerRadius)
    }

    func aeroControlGroup() -> some View {
        padding(10).aeroSurface(cornerRadius: AeroTheme.controlRadius)
    }

    func aeroSurface(cornerRadius: CGFloat = AeroTheme.panelRadius) -> some View {
        background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: cornerRadius))
            .overlay {
                RoundedRectangle(cornerRadius: cornerRadius)
                    .strokeBorder(Color.primary.opacity(0.08), lineWidth: 1)
                    .allowsHitTesting(false)
            }
    }
}

struct AeroIconButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .frame(width: 30, height: 28)
            .background(configuration.isPressed ? Color.accentColor.opacity(0.15) : Color.primary.opacity(0.05),
                        in: RoundedRectangle(cornerRadius: 6))
    }
}
