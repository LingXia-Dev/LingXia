#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Persistence adapters for desktop shell state.
///
/// Window/sidebar state shares Rust's `shell-window-v1.json` with Windows.
/// Per-lxapp group collapse and aside sizes remain platform-local preferences.
@MainActor
enum LxAppShellPersistence {
    enum SidebarMode {
        case expanded, rail
    }

    private static var defaults: UserDefaults { .standard }

    private struct PersistedSidebar: Decodable {
        let expanded: Bool
        let expandedWidth: Double
    }

    private struct PersistedWindowFrame: Decodable {
        let x: Double
        let y: Double
        let width: Double
        let height: Double
        let maximized: Bool?
    }

    private struct InitialWindowSize: Decodable {
        let width: Double
        let height: Double
    }

    static var sidebarState: (mode: SidebarMode, expandedWidth: CGFloat)? {
        let raw = shellSidebarChrome().toString()
        guard let data = raw.data(using: .utf8),
              let state = try? JSONDecoder().decode(PersistedSidebar.self, from: data),
              state.expandedWidth.isFinite,
              state.expandedWidth > 0 else { return nil }
        return (state.expanded ? .expanded : .rail, CGFloat(state.expandedWidth))
    }

    static func setSidebarState(mode: SidebarMode, expandedWidth: CGFloat) {
        guard expandedWidth.isFinite, expandedWidth > 0 else { return }
        _ = shellSetSidebarChrome(mode == .expanded, Double(expandedWidth))
    }

    /// First-launch content size fitted to the main screen's work area.
    static func initialWindowSize(default fallback: CGSize) -> CGSize {
        guard let visible = NSScreen.main?.visibleFrame else { return fallback }
        let raw = shellInitialMainWindowSize(
            Double(visible.width), Double(visible.height)
        ).toString()
        guard let data = raw.data(using: .utf8),
              let size = try? JSONDecoder().decode(InitialWindowSize.self, from: data),
              size.width.isFinite, size.height.isFinite,
              size.width > 0, size.height > 0 else { return fallback }
        return CGSize(width: size.width, height: size.height)
    }

    static func restoredWindowFrame(minSize: CGSize) -> (frame: NSRect, maximized: Bool)? {
        let raw = shellWindowFrame().toString()
        guard let data = raw.data(using: .utf8),
              let saved = try? JSONDecoder().decode(PersistedWindowFrame.self, from: data),
              saved.x.isFinite, saved.y.isFinite,
              saved.width.isFinite, saved.height.isFinite,
              saved.width > 0, saved.height > 0 else { return nil }

        let proposed = NSRect(
            x: CGFloat(saved.x),
            y: CGFloat(saved.y),
            width: CGFloat(saved.width),
            height: CGFloat(saved.height)
        )
        let intersectionArea: (NSScreen) -> CGFloat = { screen in
            let intersection = screen.visibleFrame.intersection(proposed)
            return intersection.width * intersection.height
        }
        let closest = NSScreen.screens.max {
            intersectionArea($0) < intersectionArea($1)
        }
        let screen: NSScreen?
        if let closest, intersectionArea(closest) > 0 {
            screen = closest
        } else {
            screen = NSScreen.main
        }
        let maximized = saved.maximized ?? false
        guard let visible = screen?.visibleFrame else { return (proposed, maximized) }

        let width = min(max(proposed.width, minSize.width), visible.width)
        let height = min(max(proposed.height, minSize.height), visible.height)
        let x = min(max(proposed.minX, visible.minX), visible.maxX - width)
        let y = min(max(proposed.minY, visible.minY), visible.maxY - height)
        return (NSRect(x: x, y: y, width: width, height: height), maximized)
    }

    static func setWindowFrame(_ frame: NSRect, maximized: Bool) {
        guard frame.origin.x.isFinite, frame.origin.y.isFinite,
              frame.width.isFinite, frame.height.isFinite,
              frame.width > 0, frame.height > 0 else { return }
        _ = shellSetWindowFrame(
            Double(frame.origin.x),
            Double(frame.origin.y),
            Double(frame.width),
            Double(frame.height),
            maximized
        )
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
