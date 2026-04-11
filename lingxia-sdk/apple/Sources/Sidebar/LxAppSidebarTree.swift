/// A complete declarative sidebar structure.
///
/// ```swift
/// let tree = LxAppSidebarTree(sections: [
///     LxAppSidebarSection(id: "main", tabs: [
///         LxAppSidebarTab(id: "home", label: "Home", icon: "house", appId: "com.example.home"),
///         LxAppSidebarTab(id: "settings", label: "Settings", icon: "gear", appId: "com.example.settings"),
///     ])
/// ])
/// ```
public struct LxAppSidebarTree: Codable, Sendable, Hashable {
    public var sections: [LxAppSidebarSection]

    /// Width of the sidebar in points. `nil` uses the default.
    public var width: Double?

    /// Minimum width during resize.
    public var minWidth: Double

    /// Maximum width during resize.
    public var maxWidth: Double

    public init(
        sections: [LxAppSidebarSection] = [],
        width: Double? = nil,
        minWidth: Double = 180,
        maxWidth: Double = 400
    ) {
        self.sections = sections
        self.width = width
        self.minWidth = minWidth
        self.maxWidth = maxWidth
    }
}
