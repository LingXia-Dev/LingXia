#if os(macOS)
import AppKit
import CoreText

// What the machine actually has installed.
//
// The framework bundles no font, so a configured family may simply be absent
// and the picker must show what is really available — with the two properties
// people choose on: does it ligate, and does it carry Nerd Font icons.
/// Programming ligatures are contextual alternates (`calt`), which CoreText
/// applies by default. `NSAttributedString.Key.ligature` does not control
/// them — it only reaches `liga`, the `fi`/`fl` kind — so turning them off
/// has to happen on the font itself.
enum LingXiaTerminalFontVariant {
    static func withoutLigatures(_ font: NSFont) -> NSFont {
        let settings = ["calt", "liga", "clig", "dlig"].map {
            [
                kCTFontOpenTypeFeatureTag as String: $0,
                kCTFontOpenTypeFeatureValue as String: 0,
            ]
        }
        let descriptor = font.fontDescriptor.addingAttributes([.featureSettings: settings])
        return NSFont(descriptor: descriptor, size: font.pointSize) ?? font
    }
}

enum LingXiaTerminalFontCatalog {
    struct Entry {
        let family: String
        let monospace: Bool
        let ligatures: Bool
        let nerdIcons: Bool
    }

    /// Installed families, monospace first and alphabetical within each group.
    static func installed(includeProportional: Bool = false) -> [Entry] {
        let families = NSFontManager.shared.availableFontFamilies
        var entries: [Entry] = []
        entries.reserveCapacity(families.count)
        for family in families {
            guard let font = NSFont(name: family, size: 13) else { continue }
            let monospace = isMonospace(font)
            if !monospace, !includeProportional { continue }
            entries.append(Entry(
                family: family,
                monospace: monospace,
                ligatures: hasLigatures(font),
                nerdIcons: hasNerdIcons(font)
            ))
        }
        return entries.sorted {
            $0.monospace == $1.monospace
                ? $0.family.localizedCaseInsensitiveCompare($1.family) == .orderedAscending
                : $0.monospace
        }
    }

    /// The font's own trait bit is not enough: plenty of patched coding fonts
    /// leave it unset, so equal advances decide it.
    static func isMonospace(_ font: NSFont) -> Bool {
        if font.fontDescriptor.symbolicTraits.contains(.monoSpace) { return true }
        let samples: [Character] = ["i", "M", "W", "0"]
        var advances: [CGFloat] = []
        for sample in samples {
            guard let advance = advance(of: sample, in: font) else { return false }
            advances.append(advance)
        }
        guard let first = advances.first, first > 0 else { return false }
        return advances.allSatisfy { abs($0 - first) < 0.01 }
    }

    private static func advance(of character: Character, in font: NSFont) -> CGFloat? {
        var utf16 = Array(String(character).utf16)
        var glyphs = [CGGlyph](repeating: 0, count: utf16.count)
        guard CTFontGetGlyphsForCharacters(font as CTFont, &utf16, &glyphs, utf16.count),
              let glyph = glyphs.first else { return nil }
        var subject = glyph
        var size = CGSize.zero
        CTFontGetAdvancesForGlyphs(font as CTFont, .horizontal, &subject, &size, 1)
        return size.width
    }

    /// Whether the font ligates programming sequences.
    ///
    /// Measured by shaping rather than read from a feature table: the AAT
    /// table lists Menlo's `fi`/`fl` ligatures and omits JetBrains Mono's
    /// `calt`, i.e. it answers exactly backwards for this question. Glyph
    /// *count* is no good either — a monospace ligature substitutes two
    /// glyphs for two, so the advance stays on the grid. Comparing the glyph
    /// ids against the same text with `calt` disabled is the real test.
    static func hasLigatures(_ font: NSFont) -> Bool {
        let plain = LingXiaTerminalFontVariant.withoutLigatures(font)
        for probe in ["!=", "=>", "->", "<=", "www"] {
            if shapedGlyphs(probe, font) != shapedGlyphs(probe, plain) { return true }
        }
        return false
    }

    private static func shapedGlyphs(_ text: String, _ font: NSFont) -> [CGGlyph] {
        let line = CTLineCreateWithAttributedString(
            NSAttributedString(string: text, attributes: [.font: font])
        )
        var glyphs: [CGGlyph] = []
        for run in (CTLineGetGlyphRuns(line) as? [CTRun]) ?? [] {
            let count = CTRunGetGlyphCount(run)
            var buffer = [CGGlyph](repeating: 0, count: count)
            CTRunGetGlyphs(run, CFRangeMake(0, count), &buffer)
            glyphs += buffer
        }
        return glyphs
    }

    /// Powerline separators live at U+E0B0; their presence is the practical
    /// test for a Nerd Font patched face.
    static func hasNerdIcons(_ font: NSFont) -> Bool {
        var utf16: [UniChar] = [0xE0B0]
        var glyphs = [CGGlyph](repeating: 0, count: 1)
        return CTFontGetGlyphsForCharacters(font as CTFont, &utf16, &glyphs, 1) && glyphs[0] != 0
    }

    /// JSON for the config layer's `InstalledFont`, so the CLI can list fonts
    /// without every platform inventing its own shape.
    static func installedJSON(includeProportional: Bool = false) -> String {
        let entries = installed(includeProportional: includeProportional).map {
            [
                "family": $0.family,
                "monospace": $0.monospace,
                "ligatures": $0.ligatures,
                "nerdIcons": $0.nerdIcons,
            ] as [String: Any]
        }
        guard let data = try? JSONSerialization.data(withJSONObject: entries),
              let text = String(data: data, encoding: .utf8) else { return "[]" }
        return text
    }
}
#endif
