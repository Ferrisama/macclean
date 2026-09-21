import SwiftUI

enum AeroTheme {
    static let panelRadius: CGFloat = 20
    static let controlRadius: CGFloat = 12
    static let tileRadius: CGFloat = 14
    static let spacing: CGFloat = 14
}

struct AeroBackdrop: View {
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        ZStack {
            Color(nsColor: .windowBackgroundColor)
            LinearGradient(
                colors: colorScheme == .dark
                    ? [Color(red: 0.055, green: 0.075, blue: 0.105),
                       Color(red: 0.08, green: 0.105, blue: 0.14)]
                    : [Color(red: 0.91, green: 0.96, blue: 1.0),
                       Color(red: 0.97, green: 0.985, blue: 1.0)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            RadialGradient(
                colors: [Color.cyan.opacity(colorScheme == .dark ? 0.16 : 0.11), .clear],
                center: .topLeading,
                startRadius: 20,
                endRadius: 620
            )
            RadialGradient(
                colors: [Color.indigo.opacity(colorScheme == .dark ? 0.13 : 0.08), .clear],
                center: .bottomTrailing,
                startRadius: 40,
                endRadius: 720
            )
        }
        .ignoresSafeArea()
    }
}

private struct AeroPanelModifier: ViewModifier {
    let cornerRadius: CGFloat
    let padding: CGFloat

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(.ultraThinMaterial, in: RoundedRectangle(
                cornerRadius: cornerRadius,
                style: .continuous
            ))
            .overlay {
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .strokeBorder(
                        LinearGradient(
                            colors: [.white.opacity(0.3), .white.opacity(0.06)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        lineWidth: 1
                    )
                    .allowsHitTesting(false)
            }
            .shadow(color: .black.opacity(0.12), radius: 18, y: 8)
    }
}

extension View {
    func aeroPanel(
        cornerRadius: CGFloat = AeroTheme.panelRadius,
        padding: CGFloat = AeroTheme.spacing
    ) -> some View {
        modifier(AeroPanelModifier(cornerRadius: cornerRadius, padding: padding))
    }

    func aeroControlGroup() -> some View {
        padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(.thinMaterial, in: RoundedRectangle(
                cornerRadius: AeroTheme.controlRadius,
                style: .continuous
            ))
            .overlay {
                RoundedRectangle(cornerRadius: AeroTheme.controlRadius, style: .continuous)
                    .strokeBorder(.white.opacity(0.16), lineWidth: 1)
                    .allowsHitTesting(false)
            }
    }

    func aeroSurface(cornerRadius: CGFloat = AeroTheme.panelRadius) -> some View {
        background(.ultraThinMaterial, in: RoundedRectangle(
            cornerRadius: cornerRadius,
            style: .continuous
        ))
        .overlay {
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .strokeBorder(.white.opacity(0.14), lineWidth: 1)
                .allowsHitTesting(false)
        }
    }
}

struct AeroIconButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .frame(width: 30, height: 28)
            .background(.thinMaterial, in: RoundedRectangle(
                cornerRadius: 9,
                style: .continuous
            ))
            .overlay {
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .strokeBorder(.white.opacity(configuration.isPressed ? 0.08 : 0.2))
            }
            .scaleEffect(configuration.isPressed ? 0.94 : 1)
            .opacity(configuration.isPressed ? 0.76 : 1)
            .animation(.easeOut(duration: 0.14), value: configuration.isPressed)
    }
}
