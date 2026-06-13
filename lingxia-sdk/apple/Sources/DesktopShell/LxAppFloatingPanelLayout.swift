/// Layout configuration for floating panel windows.
public struct LxAppFloatingPanelLayout: Codable, Sendable, Hashable {
    /// Default width of a panel window.
    public var width: Double

    /// Default height of a panel window.
    public var height: Double

    /// Whether the panel floats above the main window.
    public var floatsAboveMain: Bool

    public init(
        width: Double = 360,
        height: Double = 600,
        floatsAboveMain: Bool = true
    ) {
        self.width = width
        self.height = height
        self.floatsAboveMain = floatsAboveMain
    }

    public static let `default` = LxAppFloatingPanelLayout()
}
