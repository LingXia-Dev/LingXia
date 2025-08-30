import SwiftUI
import Foundation
import os.log

/// Navigation type enum shared between iOS and macOS
public enum NavigationType: Sendable {
    case launch
    case forward
    case backward
    case replace
    case switchTab
}

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// Protocol for platform-specific view controller operations
@MainActor
public protocol LxAppViewControllerProtocol: AnyObject {
    var appId: String { get }
    var isDestroyed: Bool { get }

    // Navigation operations
    func setupWebView(appId: String, path: String)
    func setupWebViewIfReady(appId: String, path: String)
    func performLxAppClose()

    // Navigation bar operations
    func createNavigationBarIfNeeded()
    func hideNavigationBar()
    func applyTransparencyEffects()

    // Tab bar operations
    func getTabBar() -> (any TabBarProtocol)?
    func syncTabBarWithCurrentPath(_ path: String)
}

/// Default implementations for LxAppViewControllerProtocol
extension LxAppViewControllerProtocol {
    public func applyTransparencyEffectsAfterTabSwitch() {
        // Default: no transparency effects needed
    }
}

/// Unified Page Navigation management for LxApp - supports both iOS and macOS
@MainActor
public class LxAppPageNavigation {

    /// Switches to a specific tab - unified implementation
    public static func switchToTab<T: LxAppViewControllerProtocol>(targetPath: String, in viewController: T) {
        guard let tabBar = viewController.getTabBar() else {
            // No tab bar, just switch page
            switchPage(targetPath: targetPath, in: viewController)
            return
        }

        let tabIndex = tabBar.findTabIndexByPath(targetPath)
        guard tabIndex >= 0 else { return }

        if getCurrentPath(from: viewController) == targetPath { return }

        tabBar.setSelectedIndex(tabIndex, notifyListener: false)
        let appId = viewController.appId
        let pageConfig = LxPageNavigation.getNavigationBarState(appId: appId, path: targetPath)
        let shouldShowNavigationBar = pageConfig?.show_navbar ?? false
        let currentHasNavBar = hasNavigationBar(viewController)

        if currentHasNavBar != shouldShowNavigationBar {
            if shouldShowNavigationBar {
                updateNavigationBar(
                    appId: appId,
                    path: targetPath,
                    disableAnimation: true,
                    in: viewController
                )
            } else {
                viewController.hideNavigationBar()
            }
        } else if shouldShowNavigationBar {
            // Only update navbar if the path actually changed or navbar config changed
            let currentPath = getCurrentPath(from: viewController)
            if currentPath != targetPath {
                updateNavigationBar(
                    appId: appId,
                    path: targetPath,
                    disableAnimation: true,
                    in: viewController
                )
            }
            // If same path, skip navbar update to avoid height changes
        }

        // Setup WebView
        viewController.setupWebView(appId: appId, path: targetPath)
        LxAppCore.setLastActivePath(targetPath, for: appId)
        viewController.applyTransparencyEffects()
    }

    /// Switches to a specific page - unified implementation
    public static func switchPage<T: LxAppViewControllerProtocol>(targetPath: String, in viewController: T) {
        guard !viewController.isDestroyed else { return }

        let params = LxPageNavigation.parseNavigationParams(from: targetPath)

        navigateToPage(
            targetPath: params.cleanPath,
            isReplace: params.isReplace,
            in: viewController
        )
    }

    /// Navigates to a specific page - unified implementation
    public static func navigateToPage<T: LxAppViewControllerProtocol>(
        targetPath: String,
        isReplace: Bool = false,
        in viewController: T
    ) {
        guard !viewController.isDestroyed else { return }

        let appId = viewController.appId

        // Update navigation bar
        updateNavigationBar(
            appId: appId,
            path: targetPath,
            in: viewController
        )

        // Setup WebView for the new page
        viewController.setupWebViewIfReady(appId: appId, path: targetPath)

        // Update tab bar selection if needed
        viewController.syncTabBarWithCurrentPath(targetPath)
    }

    /// Updates the navigation bar for a page - unified implementation
    public static func updateNavigationBar<T: LxAppViewControllerProtocol>(
        appId: String,
        path: String,
        disableAnimation: Bool = false,
        in viewController: T
    ) {
        let pageConfig = LxPageNavigation.getNavigationBarState(appId: appId, path: path)

        // Determine NavigationBar visibility using LxPageNavigation
        let shouldShowNavigationBar = pageConfig?.show_navbar ?? false

        if shouldShowNavigationBar {
            #if os(macOS)
            // For macOS, navigation bar is handled by the window controller
            if let macOSViewController = viewController as? macOSLxAppViewController,
               let windowController = macOSViewController.view.window?.windowController as? LxAppWindowController {
                windowController.updateWindowTitle(for: path)
            }
            #elseif os(iOS)
            // For iOS, use the new unified navigation bar approach
            viewController.createNavigationBarIfNeeded()

            if viewController is LxAppViewController {
                NavigationBarStateManager.shared.updateState(appId: appId, path: path)
            }
            #endif
        } else {
            viewController.createNavigationBarIfNeeded() // Ensure it exists first
        }
    }

    /// Handles back button click - unified implementation
    public static func handleBackButtonClick<T: LxAppViewControllerProtocol>(in viewController: T) {
        let result = onBackPressed(viewController.appId)
        if result {
            viewController.performLxAppClose()
        }
    }

    private static func getCurrentPath<T: LxAppViewControllerProtocol>(from viewController: T) -> String? {
        #if os(iOS)
        if let iOSManager = viewController as? LxAppViewController {
            // For manager, get current path from current app state
            return iOSManager.getCurrentPath()
        }
        #elseif os(macOS)
        if let macOSViewController = viewController as? macOSLxAppViewController {
            return macOSViewController.currentPath
        }
        #endif
        return nil
    }

    private static func hasNavigationBar<T: LxAppViewControllerProtocol>(_ viewController: T) -> Bool {
        #if os(iOS)
        if let iOSManager = viewController as? LxAppViewController {
            return iOSManager.hasNavigationBar()
        }
        return false
        #elseif os(macOS)
        // macOS always has a "navigation bar" (window title bar)
        return true
        #else
        return false
        #endif
    }

    /// Finds tab index by path
    public static func findTabIndexByPath(_ targetPath: String, in config: TabBar, appId: String) -> Int? {
        let index = LxPageNavigation.findTabIndexByPath(targetPath, in: config, appId: appId)
        return index >= 0 ? index : nil
    }

    /// Updates tab bar selection for current path
    public static func updateTabBarSelection<T: LxAppViewControllerProtocol>(
        for currentPath: String,
        in viewController: T
    ) {
        viewController.syncTabBarWithCurrentPath(currentPath)
    }

    /// Performs LxApp close operation
    public static func performLxAppClose<T: LxAppViewControllerProtocol>(in viewController: T) {
        viewController.performLxAppClose()
    }

    private static func shouldShowBackButton(for path: String, appId: String, tabBarConfig: TabBar? = nil) -> Bool {
        // Use Rust-provided NavigationBarState instead of hardcoded logic
        let pageConfig = LxPageNavigation.getNavigationBarState(appId: appId, path: path)
        return pageConfig?.show_back_button ?? false
    }

    #if os(iOS)
    private static func getTextColor(from textStyle: String?) -> UIColor {
        switch textStyle?.lowercased() {
        case "white":
            return UIColor.white
        case "black", nil:
            return UIColor.black
        default:
            return UIColor.black
        }
    }
    #elseif os(macOS)
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
    #endif
}


#if os(iOS)
extension LxAppViewController: LxAppViewControllerProtocol {
    /// Switches to tab (public interface)
    public func switchToTab(targetPath: String) {
        guard let currentAppId = currentAppId else { return }
        navigate(appId: currentAppId, to: targetPath, with: .switchTab)
    }

    /// Switches page (deprecated - use navigate instead)
    public func switchPage(targetPath: String) {
        guard let currentAppId = currentAppId else { return }
        navigate(appId: currentAppId, to: targetPath, with: .forward)
    }

    /// Navigates to page (deprecated - use navigate instead)
    public func navigateToPage(targetPath: String, isReplace: Bool = false, isBackNavigation: Bool = false) {
        guard let currentAppId = currentAppId else { return }
        let navigationType: NavigationType = isBackNavigation ? .backward : (isReplace ? .replace : .forward)
        navigate(appId: currentAppId, to: targetPath, with: navigationType)
    }

    /// Updates navigation bar (deprecated - handled automatically)
    public func updateNavigationBar(appId: String, path: String) {
        // Navigation bar updates are now handled automatically by the state manager
    }

    /// Handles back button click (deprecated - use navigate with .backward)
    public func handleBackButtonClick() {
        guard let currentAppId = currentAppId else { return }
        // Back navigation should be handled by the specific app logic
        navigate(appId: currentAppId, to: "/", with: .backward)
    }

    /// Get TabBar (protocol requirement)
    public func getTabBar() -> (any TabBarProtocol)? {
        return getTabBarInternal()
    }

    /// Sync TabBar with current path (protocol requirement)
    public func syncTabBarWithCurrentPath(_ path: String) {
        syncTabBarWithCurrentPathInternal(path)
    }


}
#endif

/// Core page navigation logic shared between iOS and macOS
@MainActor
public struct LxPageNavigation {

    /// Cache for initial routes of each app
    private static var initialRouteCache: [String: String] = [:]

    /// Cache initial route for an app (called explicitly when app opens)
    public static func cacheInitialRoute(appId: String, initialRoute: String) {
        initialRouteCache[appId] = initialRoute
    }

    /// Parses navigation parameters from target path
    public static func parseNavigationParams(from targetPath: String) -> LxNavigationParams {
        let isReplace = targetPath.contains("?replace=true")
        let cleanPath = targetPath.replacingOccurrences(of: "?replace=true", with: "")

        return LxNavigationParams(
            cleanPath: cleanPath,
            isReplace: isReplace
        )
    }

    /// Gets page configuration from Rust layer using typed API
    /// Always returns the Rust-provided configuration
    public static func getNavigationBarState(appId: String, path: String) -> NavigationBarState? {
        return lingxia.getNavigationBarState(appId, path)
    }

    /// Finds tab index by path in tab bar configuration
    public static func findTabIndexByPath(_ targetPath: String, in config: TabBar, appId: String) -> Int {
        let items = config.getItems(appId: appId)
        return items.firstIndex { $0.page_path.toString() == targetPath } ?? -1
    }

    /// Gets text color from navigation bar text style
    public static func getTextColorFromStyle(_ textStyle: String?) -> LxTextColorInfo {
        let style = textStyle ?? "black"
        let isWhiteText = style.lowercased() == "white"

        return LxTextColorInfo(
            isWhiteText: isWhiteText,
            colorString: isWhiteText ? "#FFFFFF" : "#000000"
        )
    }

    /// Validates navigation target
    public static func isValidNavigationTarget(_ path: String) -> Bool {
        return !path.isEmpty && !path.hasPrefix("http") && !path.hasPrefix("javascript:")
    }

    /// Extracts page title from configuration
    public static func getPageTitle(from pageConfig: NavigationBarState?, defaultTitle: String = "") -> String {
        return pageConfig?.title_text.toString() ?? defaultTitle
    }
}

/// Parameters for navigation operations
public struct LxNavigationParams {
    public let cleanPath: String
    public let isReplace: Bool

    public init(cleanPath: String, isReplace: Bool) {
        self.cleanPath = cleanPath
        self.isReplace = isReplace
    }
}

/// Text color information for navigation bar
public struct LxTextColorInfo {
    public let isWhiteText: Bool
    public let colorString: String

    public init(isWhiteText: Bool, colorString: String) {
        self.isWhiteText = isWhiteText
        self.colorString = colorString
    }
}

/// Navigation event types
public enum LxNavigationEventType {
    case pageSwitch
    case tabSwitch
    case backNavigation
    case replace
}

/// Navigation context information
public struct LxNavigationContext {
    public let appId: String
    public let targetPath: String
    public let eventType: LxNavigationEventType
    public let pageConfig: NavigationBarState?

    public init(appId: String, targetPath: String, eventType: LxNavigationEventType, pageConfig: NavigationBarState? = nil) {
        self.appId = appId
        self.targetPath = targetPath
        self.eventType = eventType
        self.pageConfig = pageConfig
    }
}

/// Protocol for platform-specific navigation implementations
@MainActor
public protocol LxPlatformNavigationHandler {
    associatedtype ViewControllerType

    /// Updates navigation bar for the platform
    static func updateNavigationBar(
        context: LxNavigationContext,
        in viewController: ViewControllerType
    )

    /// Handles tab switching for the platform
    static func handleTabSwitch(
        targetPath: String,
        tabIndex: Int,
        in viewController: ViewControllerType
    )

    /// Handles back navigation for the platform
    static func handleBackNavigation(in viewController: ViewControllerType)

    /// Sets up WebView for the new page
    static func setupWebView(
        appId: String,
        path: String,
        in viewController: ViewControllerType
    )
}

