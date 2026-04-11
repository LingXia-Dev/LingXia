/// Declarative specification for the navigation toolbar.
///
/// Controls visibility and configuration of toolbar elements (back button,
/// home button, title, capsule).
public struct LxAppToolbarSpec: Codable, Sendable, Hashable {
    /// Height of the toolbar in points.
    public var height: Double

    /// Show the back navigation button.
    public var showBackButton: Bool

    /// Show the home navigation button.
    public var showHomeButton: Bool

    /// Show the page title.
    public var showTitle: Bool

    /// Show the capsule (action menu button).
    public var showCapsule: Bool

    public init(
        height: Double = 38,
        showBackButton: Bool = true,
        showHomeButton: Bool = true,
        showTitle: Bool = true,
        showCapsule: Bool = true
    ) {
        self.height = height
        self.showBackButton = showBackButton
        self.showHomeButton = showHomeButton
        self.showTitle = showTitle
        self.showCapsule = showCapsule
    }

    /// Default toolbar layout.
    public static let `default` = LxAppToolbarSpec()

    /// Minimal toolbar: only back button and title.
    public static let minimal = LxAppToolbarSpec(
        showHomeButton: false,
        showCapsule: false
    )
}
