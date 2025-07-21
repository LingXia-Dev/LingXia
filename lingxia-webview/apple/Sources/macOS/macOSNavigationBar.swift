#if os(macOS)
import Cocoa
import Foundation

/// macOS-specific NavigationBar support utilities
@MainActor
public class macOSNavigationBarSupport {

    /// Creates a NavigationBar for a specific window
    public static func createNavigationBarForWindow(_ window: NSWindow) -> macOSNavigationBar? {
        return macOSNavigationBar.createForWindow(window)
    }

    /// Gets the navigation bar height for macOS
    public static func getNavigationBarHeight() -> CGFloat {
        return 32
    }

    /// Updates window title bar appearance for custom navigation bar
    public static func configureWindowForCustomNavigationBar(_ window: NSWindow) {
        window.standardWindowButton(.closeButton)?.isHidden = true
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true
    }
}

#endif
