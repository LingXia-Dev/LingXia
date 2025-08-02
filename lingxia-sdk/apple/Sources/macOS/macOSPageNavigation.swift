#if os(macOS)
import Cocoa
import Foundation

/// macOS Page Navigation management for LxApp
@MainActor
public class macOSPageNavigation {
    
    /// Switches to a specific tab
    public static func switchToTab(targetPath: String, in viewController: macOSLxAppViewController) {
        // Find tab index and switch to tab
        guard let tabBarConfig = viewController.tabBarConfig else { return }
        if let tabIndex = findTabIndexByPath(targetPath, in: tabBarConfig, appId: viewController.appId) {
            viewController.switchToTab(targetPath: targetPath, tabIndex: tabIndex)
        }
    }
    
    /// Switches to a specific page
    public static func switchPage(targetPath: String, in viewController: macOSLxAppViewController) {
        let params = PageNavigationCore.parseNavigationParams(from: targetPath)

        navigateToPage(targetPath: params.cleanPath, isReplace: params.isReplace, in: viewController)
    }
    
    /// Navigates to a specific page
    public static func navigateToPage(
        targetPath: String,
        isReplace: Bool = false,
        in viewController: macOSLxAppViewController
    ) {
        // Update window title through window controller
        if let windowController = viewController.view.window?.windowController as? macOSWindowController {
            windowController.updateWindowTitle(for: targetPath)
        }
    }
    
    /// Updates navigation bar (window title) for a page
    public static func updateNavigationBar(
        appId: String,
        path: String,
        in viewController: macOSLxAppViewController
    ) {
        // For macOS, navigation bar is handled by the window controller
        if let windowController = viewController.view.window?.windowController as? macOSWindowController {
            windowController.updateWindowTitle(for: path)
        }
    }
    
    /// Gets page configuration
    public static func getNavigationBarConfig(appId: String, path: String) -> NavigationBarConfig? {
        return PageNavigationCore.getNavigationBarConfig(appId: appId, path: path)
    }
    
    /// Handles back button click (keyboard shortcut on macOS)
    public static func handleBackButtonClick(in viewController: macOSLxAppViewController) {
        // just close the window for now
        viewController.view.window?.close()
    }
    
    /// Finds tab index by path
    public static func findTabIndexByPath(_ targetPath: String, in config: TabBarConfig, appId: String) -> Int? {
        let index = PageNavigationCore.findTabIndexByPath(targetPath, in: config, appId: appId)
        return index >= 0 ? index : nil
    }
    
    /// Updates tab bar selection for current path
    public static func updateTabBarSelection(
        for currentPath: String,
        in viewController: macOSLxAppViewController
    ) {
        // For now, it's a placeholder for consistency with iOS
    }
    
    /// Performs LxApp close operation
    public static func performLxAppClose(in viewController: macOSLxAppViewController) {
        // Notify the system that the app is closing
        // lingxia.closeLxApp(viewController.appId) // API not available yet

        // Close the window
        viewController.view.window?.close()
    }
    
    private static func shouldShowBackButton(for path: String, appId: String) -> Bool {
        // Don't show back button for home app
        if appId == LxAppCore.getHomeLxAppId() {
            return false
        }

        return true // Default to showing back button
    }
    
    private static func getTextColor(from textStyle: String?) -> NSColor {
        switch textStyle?.lowercased() {
        case "white":
            return NSColor.white
        case "black", nil:
            return NSColor.black
        default:
            return NSColor.black
        }
    }
}

// MARK: - Extensions for macOSLxAppViewController

extension macOSLxAppViewController {
    
    /// Switches to tab (public interface)
    public func switchToTab(targetPath: String) {
        macOSPageNavigation.switchToTab(targetPath: targetPath, in: self)
    }
    
    /// Switches page (public interface) - replaces existing method
    public func switchPageViaNavigation(targetPath: String) {
        macOSPageNavigation.switchPage(targetPath: targetPath, in: self)
    }
    
    /// Navigates to page (public interface)
    public func navigateToPage(targetPath: String, isReplace: Bool = false) {
        macOSPageNavigation.navigateToPage(targetPath: targetPath, isReplace: isReplace, in: self)
    }
    
    /// Updates navigation bar (public interface)
    public func updateNavigationBar(appId: String, path: String) {
        macOSPageNavigation.updateNavigationBar(appId: appId, path: path, in: self)
    }
    
    /// Gets page config (public interface)
    public func getNavigationBarConfig(appId: String, path: String) -> NavigationBarConfig? {
        return macOSPageNavigation.getNavigationBarConfig(appId: appId, path: path)
    }
    
    /// Handles back button click (public interface)
    public func handleBackButtonClick() {
        macOSPageNavigation.handleBackButtonClick(in: self)
    }
    
    /// Performs LxApp close (public interface)
    public func performLxAppClose() {
        macOSPageNavigation.performLxAppClose(in: self)
    }
    
    /// Updates tab bar selection for path
    public func updateTabBarSelection(for path: String) {
        macOSPageNavigation.updateTabBarSelection(for: path, in: self)
    }
}

#endif
