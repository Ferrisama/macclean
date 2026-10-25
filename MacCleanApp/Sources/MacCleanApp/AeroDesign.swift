import SwiftUI

enum AeroTheme {
    static let panelRadius: CGFloat = 10
    static let controlRadius: CGFloat = 6
    static let tileRadius: CGFloat = 5
    static let spacing: CGFloat = 16
    static let background = Color(red: 0.031, green: 0.035, blue: 0.043)
    static let surface = Color(red: 0.067, green: 0.071, blue: 0.086)
    static let raised = Color(red: 0.098, green: 0.106, blue: 0.129)
    static let tile = Color(red: 0.125, green: 0.133, blue: 0.161)
    static let border = Color(red: 0.173, green: 0.18, blue: 0.216)
    static let accent = Color(red: 1, green: 0.322, blue: 0.408)
    static let selection = Color(red: 0.208, green: 0.094, blue: 0.125)
}

struct AeroBackdrop: View {
    var body: some View { AeroTheme.background.ignoresSafeArea().allowsHitTesting(false) }
}

extension View {
    func aeroPanel(cornerRadius: CGFloat = AeroTheme.panelRadius, padding: CGFloat = AeroTheme.spacing) -> some View {
        self.padding(padding).aeroSurface(cornerRadius: cornerRadius)
    }
    func aeroControlGroup() -> some View { padding(10).aeroSurface(cornerRadius: AeroTheme.controlRadius) }
    func aeroSurface(cornerRadius: CGFloat = AeroTheme.panelRadius) -> some View {
        background(AeroTheme.surface, in: RoundedRectangle(cornerRadius: cornerRadius))
            .overlay { RoundedRectangle(cornerRadius: cornerRadius).strokeBorder(AeroTheme.border, lineWidth: 1).allowsHitTesting(false) }
    }
}

struct AeroIconButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.frame(width: 30, height: 28)
            .background(configuration.isPressed ? AeroTheme.selection : AeroTheme.raised, in: RoundedRectangle(cornerRadius: 6))
    }
}
