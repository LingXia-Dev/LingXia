#if os(macOS)
import AppKit

enum LxAppHostThemeRole {
    case windowBackground
    case surfaceBackground
    case foreground
    case mutedForeground
    case accent
    case separator
    case selectionBackground
}

enum LxAppHostTheme {
    // Installed once during runtime startup, before any shell views exist.
    private nonisolated(unsafe) static var config: LxAppGeneratedThemeConfig?

    static func install(_ config: LxAppGeneratedThemeConfig?) {
        self.config = config
    }

    static var windowBackground: NSColor {
        adaptive(.windowBackground)
    }

    static var surfaceBackground: NSColor {
        adaptive(.surfaceBackground)
    }

    static var foreground: NSColor {
        adaptive(.foreground)
    }

    static var mutedForeground: NSColor {
        adaptive(.mutedForeground)
    }

    static var accent: NSColor {
        adaptive(.accent)
    }

    static var separator: NSColor {
        adaptive(.separator)
    }

    static var selectionBackground: NSColor {
        adaptive(.selectionBackground)
    }

    private static func adaptive(_ role: LxAppHostThemeRole) -> NSColor {
        let config = config
        return NSColor(name: nil) { appearance in
            resolve(role, config: config, appearance: appearance) ?? platformDefault(role)
        }
    }

    static func resolved(
        _ role: LxAppHostThemeRole,
        for appearance: NSAppearance
    ) -> NSColor {
        resolve(role, config: config, appearance: appearance) ?? platformDefault(role)
    }

    private static func platformDefault(_ role: LxAppHostThemeRole) -> NSColor {
        switch role {
        case .windowBackground: .windowBackgroundColor
        case .surfaceBackground: .controlBackgroundColor
        case .foreground: .labelColor
        case .mutedForeground: .secondaryLabelColor
        case .accent: .controlAccentColor
        case .separator: .separatorColor
        case .selectionBackground: .unemphasizedSelectedContentBackgroundColor
        }
    }

    private static func resolve(
        _ role: LxAppHostThemeRole,
        config: LxAppGeneratedThemeConfig?,
        appearance: NSAppearance
    ) -> NSColor? {
        let dark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        guard let style = dark ? config?.dark : config?.light else { return nil }

        let value = switch role {
        case .windowBackground: style.windowBackgroundColor
        case .surfaceBackground: style.surfaceBackgroundColor
        case .foreground: style.foregroundColor
        case .mutedForeground: style.mutedForegroundColor
        case .accent: style.accentColor
        case .separator: style.separatorColor
        case .selectionBackground: style.selectionBackgroundColor
        }
        return value.flatMap(parseColor)
    }

    private static func parseColor(_ value: String) -> NSColor? {
        guard value.count == 7,
              value.first == "#",
              let rgb = UInt32(value.dropFirst(), radix: 16) else {
            return nil
        }
        return NSColor(
            srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}

@MainActor
final class LxAppHostThemeLayerView: NSView {
    private let role: LxAppHostThemeRole
    private let alpha: CGFloat?

    /// `alpha: nil` keeps the role color's own alpha, so a platform hairline
    /// (`separatorColor` is a low-alpha tint) stays a hairline instead of being
    /// forced up to a solid line.
    init(role: LxAppHostThemeRole, alpha: CGFloat? = 1) {
        self.role = role
        self.alpha = alpha
        super.init(frame: .zero)
        wantsLayer = true
        updateThemeColor()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        updateThemeColor()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateThemeColor()
    }

    private func updateThemeColor() {
        effectiveAppearance.performAsCurrentDrawingAppearance {
            let color = LxAppHostTheme.resolved(role, for: effectiveAppearance)
            layer?.backgroundColor = alpha.map(color.withAlphaComponent)?.cgColor ?? color.cgColor
        }
    }
}
#endif
