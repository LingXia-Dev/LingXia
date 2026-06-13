/// Visual style for the shell chrome (window frame, padding, shadows).
public struct LxAppChromeStyle: Codable, Sendable, Hashable {
    /// Corner radius of the content area. 0 = square.
    public var cornerRadius: Double

    /// Whether the content area has a shadow.
    public var hasShadow: Bool

    /// Padding around the content area inside the window.
    public var contentPadding: Double

    public init(
        cornerRadius: Double = 10,
        hasShadow: Bool = true,
        contentPadding: Double = 0
    ) {
        self.cornerRadius = cornerRadius
        self.hasShadow = hasShadow
        self.contentPadding = contentPadding
    }

    /// Default chrome matching the standard shell appearance.
    public static let `default` = LxAppChromeStyle()

    /// Flat chrome: no rounding, no shadow, no padding.
    public static let flat = LxAppChromeStyle(cornerRadius: 0, hasShadow: false, contentPadding: 0)
}
