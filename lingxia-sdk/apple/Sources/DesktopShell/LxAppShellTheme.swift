#if os(macOS)
import AppKit

/// Home-product semantic colors for host-owned desktop chrome. Page and
/// child-lxapp CSS never reads this palette; they inherit appearance only.
@MainActor
enum LxAppShellTheme {
    private static let values: [String: [String: String]] = {
        guard let data = shellThemeJSON().toString().data(using: .utf8),
              let value = try? JSONSerialization.jsonObject(with: data) as? [String: [String: String]]
        else { return [:] }
        return value
    }()

    static let windowBackground = adaptive(
        "windowBackgroundColor",
        light: 0xE6E6EB,
        dark: 0x29292E
    )
    static let surfaceBackground = adaptive(
        "surfaceBackgroundColor",
        light: 0xFFFFFF,
        dark: 0x2B2B2B
    )
    static let foreground = adaptive("foregroundColor", light: 0x111827, dark: 0xF3F3F3)
    static let mutedForeground = adaptive(
        "mutedForegroundColor",
        light: 0x667085,
        dark: 0x9AA0A6
    )
    static let accent = adaptive("accentColor", light: 0x1677FF, dark: 0x5B8CFF)
    static let separator = adaptive("separatorColor", light: 0xC7C2D2, dark: 0x383838)
    static let selectionBackground = adaptive(
        "selectionBackgroundColor",
        light: 0xF7F5FB,
        dark: 0x34333A
    )
    static let sidebarBackground = adaptive(
        "sidebarBackgroundColor",
        light: 0xE6E6EB,
        dark: 0x29292E
    )
    static let sidebarForeground = adaptive(
        "sidebarForegroundColor",
        fallback: foreground
    )
    static let sidebarSelectedBackground = adaptive(
        "sidebarSelectedBackgroundColor",
        fallback: selectionBackground
    )
    static let sidebarSelectedForeground = adaptive(
        "sidebarSelectedForegroundColor",
        fallback: foreground
    )

    private static func adaptive(_ key: String, light: UInt32, dark: UInt32) -> NSColor {
        adaptive(key, fallback: NSColor(name: nil) { appearance in
            color(isDark(appearance) ? dark : light)
        })
    }

    private static func adaptive(_ key: String, fallback: NSColor) -> NSColor {
        NSColor(name: nil) { appearance in
            let mode = isDark(appearance) ? "dark" : "light"
            guard let raw = values[mode]?[key], let configured = parse(raw) else {
                return resolved(fallback, for: appearance)
            }
            return configured
        }
    }

    static func cgColor(_ color: NSColor, for appearance: NSAppearance) -> CGColor {
        var result = color.cgColor
        appearance.performAsCurrentDrawingAppearance {
            result = color.cgColor
        }
        return result
    }

    private static func parse(_ raw: String) -> NSColor? {
        let hex = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard hex.hasPrefix("#") else { return nil }
        let value = String(hex.dropFirst())
        guard value.count == 6 || value.count == 8,
              let number = UInt32(value, radix: 16) else { return nil }
        let rgb = value.count == 8 ? number & 0x00FF_FFFF : number
        let alpha = value.count == 8 ? CGFloat((number >> 24) & 0xFF) / 255 : 1
        return NSColor(
            srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: alpha
        )
    }

    private static func isDark(_ appearance: NSAppearance) -> Bool {
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    }

    private static func color(_ rgb: UInt32) -> NSColor {
        NSColor(
            srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }

    private static func resolved(_ color: NSColor, for appearance: NSAppearance) -> NSColor {
        var result = color
        appearance.performAsCurrentDrawingAppearance {
            result = color.usingColorSpace(.deviceRGB) ?? color
        }
        return result
    }
}

@MainActor
final class LxAppShellThemedView: NSView {
    private let color: NSColor

    init(color: NSColor) {
        self.color = color
        super.init(frame: .zero)
        wantsLayer = true
        updateColor()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateColor()
    }

    private func updateColor() {
        layer?.backgroundColor = LxAppShellTheme.cgColor(color, for: effectiveAppearance)
    }
}
#endif
