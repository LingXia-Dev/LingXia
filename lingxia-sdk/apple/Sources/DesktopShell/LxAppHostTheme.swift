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
    case selectedTabBackground
}

/// Product chrome colours: the host theme's roles, or AppKit's own colours when the product
/// ships no theme.
enum LxAppHostTheme {
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

    /// The neutral selected fill (an active lxapp, a browser tab).
    static var selectionBackground: NSColor {
        adaptive(.selectionBackground)
    }

    /// The selected tab row: the one accented level, so the lxapp's header stays neutral.
    static var selectedTabBackground: NSColor {
        adaptive(.selectedTabBackground)
    }

    private static func adaptive(_ role: LxAppHostThemeRole) -> NSColor {
        NSColor(name: nil) { appearance in resolved(role, for: appearance) }
    }

    static func resolved(
        _ role: LxAppHostThemeRole,
        for appearance: NSAppearance
    ) -> NSColor {
        let dark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        return parseColor(themeRole("", roleName(role), dark).toString()) ?? platformDefault(role)
    }

    private static func roleName(_ role: LxAppHostThemeRole) -> String {
        switch role {
        case .windowBackground: "canvas"
        case .surfaceBackground: "surface"
        case .foreground: "text"
        case .mutedForeground: "text-muted"
        case .accent: "accent"
        case .separator: "line"
        case .selectionBackground: "surface-raised"
        case .selectedTabBackground: "accent-soft"
        }
    }

    private static func platformDefault(_ role: LxAppHostThemeRole) -> NSColor {
        switch role {
        case .windowBackground: .windowBackgroundColor
        case .surfaceBackground: .controlBackgroundColor
        case .foreground: .labelColor
        case .mutedForeground: .secondaryLabelColor
        case .accent: .controlAccentColor
        case .separator: .separatorColor
        case .selectionBackground, .selectedTabBackground: .unemphasizedSelectedContentBackgroundColor
        }
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

extension NSView {
    /// `NSColor.cgColor` resolves against the *current drawing* appearance,
    /// which outside a draw pass is the system's — not this view's. A layer
    /// colour assigned straight from a dynamic colour therefore ignores the
    /// product's own light/dark setting and never re-resolves when it changes.
    func themeCGColor(_ color: NSColor) -> CGColor {
        var resolved = color.cgColor
        effectiveAppearance.performAsCurrentDrawingAppearance {
            resolved = color.cgColor
        }
        return resolved
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
