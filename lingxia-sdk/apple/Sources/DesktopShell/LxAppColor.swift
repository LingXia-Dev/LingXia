/// A platform-agnostic, Codable color representation.
public struct LxAppColor: Codable, Sendable, Hashable {
    public var r: Double
    public var g: Double
    public var b: Double
    public var a: Double

    public init(r: Double, g: Double, b: Double, a: Double = 1.0) {
        self.r = r
        self.g = g
        self.b = b
        self.a = a
    }

    // MARK: - Presets

    public static let white = LxAppColor(r: 1, g: 1, b: 1)
    public static let black = LxAppColor(r: 0, g: 0, b: 0)
    public static let clear = LxAppColor(r: 0, g: 0, b: 0, a: 0)

    /// The default sidebar background.
    public static let sidebarBackground = LxAppColor(r: 0.95, g: 0.95, b: 0.95)

    /// The default toolbar background.
    public static let toolbarBackground = LxAppColor(r: 1, g: 1, b: 1)
}

#if os(macOS)
import AppKit
extension LxAppColor {
    /// Convert to `NSColor`. For internal use only.
    internal var nsColor: NSColor {
        NSColor(red: r, green: g, blue: b, alpha: a)
    }

    /// Create from `NSColor`.
    public init(_ nsColor: NSColor) {
        let c = nsColor.usingColorSpace(.sRGB) ?? nsColor
        self.r = Double(c.redComponent)
        self.g = Double(c.greenComponent)
        self.b = Double(c.blueComponent)
        self.a = Double(c.alphaComponent)
    }
}
#elseif os(iOS)
import UIKit
extension LxAppColor {
    /// Convert to `UIColor`. For internal use only.
    internal var uiColor: UIColor {
        UIColor(red: r, green: g, blue: b, alpha: a)
    }

    /// Create from `UIColor`.
    public init(_ uiColor: UIColor) {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        uiColor.getRed(&r, green: &g, blue: &b, alpha: &a)
        self.r = Double(r)
        self.g = Double(g)
        self.b = Double(b)
        self.a = Double(a)
    }
}
#endif
