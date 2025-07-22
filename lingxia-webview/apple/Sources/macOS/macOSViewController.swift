#if os(macOS)
import Cocoa
import Foundation

/// macOS-specific view controller support utilities
@MainActor
public class macOSViewControllerSupport {

    /// Configures the view controller for edge-to-edge display
    public static func configureEdgeToEdgeDisplay(_ viewController: NSViewController) {
        // macOS doesn't need edge-to-edge configuration like iOS
        // But we can configure the view for full content area usage
        viewController.view.wantsLayer = true
    }

    /// Sets transparent background for the view controller
    public static func setTransparentBackground(_ viewController: NSViewController) {
        viewController.view.layer?.backgroundColor = NSColor.clear.cgColor
    }

    /// Gets the appropriate top margin for content positioning
    public static func getTopMargin(for windowStyle: LxAppWindowStyle) -> CGFloat {
        switch windowStyle {
        case .customCapsule:
            return 32  // Custom capsule style needs space for title bar
        case .systemDefault:
            return 0   // System default style uses system title bar
        case .borderless:
            return 0   // Content fills entire window, system buttons float on top
        }
    }

    /// Calculates the appropriate content area frame
    public static func calculateContentAreaFrame(
        containerFrame: CGRect,
        topMargin: CGFloat,
        hasTabBar: Bool,
        tabBarHeight: CGFloat,
        tabBarPosition: TabBarConfig.Position
    ) -> CGRect {
        var contentFrame = containerFrame

        // Apply top margin
        contentFrame.origin.y += topMargin
        contentFrame.size.height -= topMargin

        // Apply tab bar constraints if needed
        if hasTabBar {
            switch tabBarPosition {
            case .bottom:
                contentFrame.size.height -= tabBarHeight
            case .top:
                contentFrame.origin.y += tabBarHeight
                contentFrame.size.height -= tabBarHeight
            case .left:
                contentFrame.origin.x += tabBarHeight
                contentFrame.size.width -= tabBarHeight
            case .right:
                contentFrame.size.width -= tabBarHeight
            }
        }

        return contentFrame
    }

    /// Updates layout constraints for the view controller
    public static func updateLayoutConstraints(_ viewController: NSViewController) {
        // Ensure proper layout updates
        viewController.view.needsLayout = true
        viewController.view.layoutSubtreeIfNeeded()
    }

    /// Brings UI elements to front (equivalent to iOS bringSubviewToFront)
    public static func bringUIElementsToFront(in view: NSView, elements: [NSView]) {
        for element in elements {
            view.addSubview(element, positioned: .above, relativeTo: nil)
        }
    }

    /// Configures WebView container for transparency
    public static func configureWebViewContainerForTransparency(_ container: NSView, isTransparent: Bool) {
        container.wantsLayer = true
        if isTransparent {
            container.layer?.backgroundColor = NSColor.clear.cgColor
        } else {
            container.layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
        }
    }

    /// Sets up notification observers for view controller
    public static func setupNotificationObservers(
        for viewController: NSViewController,
        appId: String,
        onSwitchPage: @escaping (String) -> Void,
        onCloseApp: @escaping () -> Void
    ) -> (switchPageObserver: NSObjectProtocol?, closeAppObserver: NSObjectProtocol?) {

        let switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name("ACTION_SWITCH_PAGE"),
            object: nil,
            queue: .main
        ) { notification in
            guard let notificationAppId = notification.userInfo?["appId"] as? String,
                  let path = notification.userInfo?["path"] as? String,
                  notificationAppId == appId else { return }
            onSwitchPage(path)
        }

        let closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name("ACTION_CLOSE_APP"),
            object: nil,
            queue: .main
        ) { notification in
            guard let notificationAppId = notification.userInfo?["appId"] as? String,
                  notificationAppId == appId else { return }
            onCloseApp()
        }

        return (switchPageObserver, closeAppObserver)
    }

    /// Removes notification observers
    public static func removeNotificationObservers(_ observers: (NSObjectProtocol?, NSObjectProtocol?)) {
        if let switchPageObserver = observers.0 {
            NotificationCenter.default.removeObserver(switchPageObserver)
        }
        if let closeAppObserver = observers.1 {
            NotificationCenter.default.removeObserver(closeAppObserver)
        }
    }

    /// Performs cleanup before view controller replacement
    public static func performCleanupBeforeReplacement(_ viewController: NSViewController) {
        // Remove all subviews
        viewController.view.subviews.forEach { $0.removeFromSuperview() }

        // Clear any cached data
        viewController.view.layer?.contents = nil

        // Force layout update
        viewController.view.needsLayout = true
    }
}

#endif
