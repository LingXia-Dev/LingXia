/// A single tab item in the sidebar.
///
/// Tabs are the leaf nodes of the sidebar tree. Each tab represents
/// an LxApp or a specific page within an LxApp.
public struct LxAppSidebarTab: Codable, Sendable, Hashable, Identifiable {
    public var id: String
    public var label: String
    public var icon: String?
    public var appId: String
    public var path: String

    public init(
        id: String,
        label: String,
        icon: String? = nil,
        appId: String,
        path: String = "/"
    ) {
        self.id = id
        self.label = label
        self.icon = icon
        self.appId = appId
        self.path = path
    }
}
