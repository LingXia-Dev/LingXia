/// A named group of tabs in the sidebar.
///
/// Sections are collapsible groups within the sidebar tree.
/// A section with no label renders as a flat list of tabs.
public struct LxAppSidebarSection: Codable, Sendable, Hashable, Identifiable {
    public var id: String
    public var label: String?
    public var tabs: [LxAppSidebarTab]
    public var isCollapsed: Bool

    public init(
        id: String,
        label: String? = nil,
        tabs: [LxAppSidebarTab] = [],
        isCollapsed: Bool = false
    ) {
        self.id = id
        self.label = label
        self.tabs = tabs
        self.isCollapsed = isCollapsed
    }
}
