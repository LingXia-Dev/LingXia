#if os(macOS)
import AppKit

// The configuration in effect for terminal surfaces.
//
// The shared Rust layer owns schema, storage and merging, and applies the
// theme itself — cell colors resolve when a frame is built, so a theme change
// is a repaint of every live session. What is left here is the half only the
// platform can do: work out which of the configured font candidates is
// actually installed, and turn the rest into view state.
@MainActor
struct LingXiaTerminalSettings: Decodable {
    struct Font: Decodable {
        var family: [String] = []
        var size: CGFloat = LingXiaTerminalFont.defaultSize
        var lineHeight: CGFloat = 1
        var ligatures: Bool = true
    }

    var font = Font()

    /// Publish the installed families so settings and product-control commands
    /// report what is really available. Enumerating them is platform work the
    /// shared configuration layer cannot do.
    static func registerInstalledFonts() {
        terminalRegisterFonts(LingXiaTerminalFontCatalog.installedJSON())
    }

    /// Load from the engine, which merges product defaults with the user's
    /// `terminal.json` and applies the theme on the way through.
    static func load() -> LingXiaTerminalSettings {
        registerInstalledFonts()
        let dark = NSApp?.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let json = terminalLoadConfig(dark).toString()
        // The load applied the theme, so the chrome derived from it is now
        // current — read it here rather than leaving the first paint on the
        // built-in fallback.
        LingXiaTerminalChrome.reload()
        guard let data = json.data(using: .utf8),
              let settings = try? JSONDecoder().decode(LingXiaTerminalSettings.self, from: data)
        else {
            LXLog.error("terminal configuration unreadable; using built-in defaults", category: "MacTerminal")
            return LingXiaTerminalSettings()
        }
        return settings
    }

    /// The first installed candidate, with the outcome logged.
    ///
    /// Nothing is bundled, so a configured family may simply be absent; a
    /// silent downgrade is the failure a user cannot diagnose, hence the log
    /// naming both what was asked for and what was used.
    func resolvedFontFamily() -> String? {
        guard !font.family.isEmpty else { return nil }
        let installed = LingXiaTerminalFontCatalog.installed()
        var missing: [String] = []
        for candidate in font.family {
            if let match = installed.first(where: {
                $0.family.compare(candidate, options: .caseInsensitive) == .orderedSame
            }) {
                if !missing.isEmpty {
                    LXLog.info(
                        "terminal font: using \(match.family); not installed: \(missing.joined(separator: ", "))",
                        category: "MacTerminal"
                    )
                }
                return match.family
            }
            missing.append(candidate)
        }
        LXLog.error(
            "terminal font: none of \(font.family.joined(separator: ", ")) is an installed monospace family; falling back",
            category: "MacTerminal"
        )
        return nil
    }

    /// The configured face, or the built-in fallback chain when no candidate
    /// resolves.
    func makeFont() -> NSFont {
        guard let family = resolvedFontFamily(),
              let resolved = NSFont(name: family, size: font.size)
        else {
            return LingXiaTerminalFont.regular(size: font.size)
        }
        return LingXiaTerminalFont.withCascade(resolved)
    }
}
#endif
