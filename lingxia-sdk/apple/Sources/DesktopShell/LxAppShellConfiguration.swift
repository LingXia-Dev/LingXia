/// Complete configuration for an `LxAppShell`.
///
/// All declarative paths are `Codable`. The `.swiftNative(...)` variants
/// are encoded as sentinels for serialization purposes.
///
/// ```swift
/// var config = LxAppShellConfiguration()
/// config.sidebar = .declarative(myTree)
/// config.chrome = .flat
/// config.toolbar = .hidden
/// let shell = LxAppShell(controller: controller, configuration: config)
/// ```
public struct LxAppShellConfiguration: Codable, Sendable {
    /// Sidebar mode. Default: `.hidden` (host must opt in).
    public var sidebar: LxAppSidebarMode

    /// Toolbar mode. Default: `.declarative(.default)`.
    public var toolbar: LxAppToolbarMode

    /// Chrome (window frame) style.
    public var chrome: LxAppChromeStyle

    /// Traffic light button placement (macOS only).
    public var trafficLightPlacement: LxAppTrafficLightPlacement

    /// Floating panel layout.
    public var panelLayout: LxAppFloatingPanelLayout

    /// Background color of the sidebar area.
    public var sidebarBackground: LxAppColor

    /// Background color of the toolbar area.
    public var toolbarBackground: LxAppColor

    public init(
        sidebar: LxAppSidebarMode = .hidden,
        toolbar: LxAppToolbarMode = .declarative(.default),
        chrome: LxAppChromeStyle = .default,
        trafficLightPlacement: LxAppTrafficLightPlacement = .sidebar,
        panelLayout: LxAppFloatingPanelLayout = .default,
        sidebarBackground: LxAppColor = .sidebarBackground,
        toolbarBackground: LxAppColor = .toolbarBackground
    ) {
        self.sidebar = sidebar
        self.toolbar = toolbar
        self.chrome = chrome
        self.trafficLightPlacement = trafficLightPlacement
        self.panelLayout = panelLayout
        self.sidebarBackground = sidebarBackground
        self.toolbarBackground = toolbarBackground
    }
}
