#if os(macOS)
import AppKit

/// UserDefaults-backed persistence for desktop shell chrome state.
///
/// Only user-driven geometry and state is stored (window frame, sidebar
/// width/mode, group collapse, aside sizes). API-driven state — tabbar
/// hidden, activator items — is rebuilt by the app on launch and never
/// persisted here. Asides restore geometry only, never content.
@MainActor
enum LxAppShellPersistence {
    /// NSWindow frame autosave name; AppKit owns save/restore + screen clamping.
    static let windowFrameName = "lingxia.shell.window"

    enum SidebarMode: String {
        case expanded, rail, hidden
    }

    private static var defaults: UserDefaults { .standard }
    private static let sidebarWidthKey = "lingxia.shell.sidebar.width"
    private static let sidebarModeKey = "lingxia.shell.sidebar.mode"

    /// Last settled expanded sidebar width; nil when never saved.
    static var sidebarWidth: CGFloat? {
        get {
            let width = defaults.double(forKey: sidebarWidthKey)
            return width > 0 ? CGFloat(width) : nil
        }
        set {
            if let newValue { defaults.set(Double(newValue), forKey: sidebarWidthKey) }
        }
    }

    static var sidebarMode: SidebarMode? {
        get { defaults.string(forKey: sidebarModeKey).flatMap(SidebarMode.init) }
        set {
            if let newValue { defaults.set(newValue.rawValue, forKey: sidebarModeKey) }
        }
    }

    /// User collapse state of an lxapp's sidebar group; nil when never toggled.
    static func groupCollapsed(appId: String) -> Bool? {
        let key = "lingxia.shell.group.collapsed.\(appId)"
        guard defaults.object(forKey: key) != nil else { return nil }
        return defaults.bool(forKey: key)
    }

    static func setGroupCollapsed(_ collapsed: Bool, appId: String) {
        defaults.set(collapsed, forKey: "lingxia.shell.group.collapsed.\(appId)")
    }

    /// User-resized aside panel size (width or height by edge); nil when unset.
    static func asideSize(panelId: String) -> CGFloat? {
        let size = defaults.double(forKey: "lingxia.shell.aside.size.\(panelId)")
        return size > 0 ? CGFloat(size) : nil
    }

    static func setAsideSize(_ size: CGFloat, panelId: String) {
        defaults.set(Double(size), forKey: "lingxia.shell.aside.size.\(panelId)")
    }
}
#endif
