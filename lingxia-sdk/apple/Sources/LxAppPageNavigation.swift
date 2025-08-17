import SwiftUI
import Foundation
import os.log

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
    func setupWebViewWithoutNavBarUpdate(appId: String, path: String)
    func setupWebViewIfReady(appId: String, path: String)
    func performLxAppClose()

    // Navigation bar operations
    func ensureNavigationBarExists()
    func removeNavigationBar()
    func removeNavigationBarForTabSwitch()
    func applyTransparencyEffectsAfterTabSwitch()

    // Tab bar operations
    func getTabBar() -> (any TabBarProtocol)?
    func syncTabBarWithCurrentPath(_ path: String)
}

/// Default implementations for LxAppViewControllerProtocol
extension LxAppViewControllerProtocol {
    /// Default implementation - does nothing (useful for platforms that don't need transparency effects)
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

        // Skip if already on this tab
        if getCurrentPath(from: viewController) == targetPath { return }

        // Update TabBar UI first
        tabBar.setSelectedIndex(tabIndex, notifyListener: false)

        let appId = viewController.appId

        // Optimized NavigationBar handling for TabBar switches
        let pageConfig = LxPageNavigation.getNavigationBarConfig(appId: appId, path: targetPath)
        let shouldShowNavigationBar = LxPageNavigation.shouldShowNavigationBar(pageConfig: pageConfig)
        let currentHasNavBar = hasNavigationBar(viewController)

        // Only update NavigationBar if state actually changes
        if currentHasNavBar != shouldShowNavigationBar {
            if shouldShowNavigationBar {
                updateNavigationBar(
                    appId: appId,
                    path: targetPath,
                    isBackNavigation: false,
                    disableAnimation: true,
                    in: viewController
                )
            } else {
                // Remove NavigationBar if not needed
                viewController.removeNavigationBarForTabSwitch()
            }
        } else if shouldShowNavigationBar {
            // NavigationBar exists and should stay - just update content without animation
            updateNavigationBar(
                appId: appId,
                path: targetPath,
                isBackNavigation: false,
                disableAnimation: true,
                in: viewController
            )
        }

        // Setup WebView
        viewController.setupWebViewWithoutNavBarUpdate(appId: appId, path: targetPath)
        LxAppCore.setLastActivePath(targetPath, for: appId)
        viewController.applyTransparencyEffectsAfterTabSwitch()
    }

    /// Switches to a specific page - unified implementation
    public static func switchPage<T: LxAppViewControllerProtocol>(targetPath: String, in viewController: T) {
        guard !viewController.isDestroyed else { return }

        let params = LxPageNavigation.parseNavigationParams(from: targetPath)

        navigateToPage(
            targetPath: params.cleanPath,
            isReplace: params.isReplace,
            isBackNavigation: false,
            in: viewController
        )
    }

    /// Navigates to a specific page - unified implementation
    public static func navigateToPage<T: LxAppViewControllerProtocol>(
        targetPath: String,
        isReplace: Bool = false,
        isBackNavigation: Bool = false,
        in viewController: T
    ) {
        guard !viewController.isDestroyed else { return }

        let appId = viewController.appId

        // Update navigation bar
        updateNavigationBar(
            appId: appId,
            path: targetPath,
            isBackNavigation: isBackNavigation,
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
        isBackNavigation: Bool = false,
        disableAnimation: Bool = false,
        in viewController: T
    ) {
        let pageConfig = LxPageNavigation.getNavigationBarConfig(appId: appId, path: path)

        // Determine NavigationBar visibility using LxPageNavigation
        let shouldShowNavigationBar = LxPageNavigation.shouldShowNavigationBar(pageConfig: pageConfig)

        if shouldShowNavigationBar {
            #if os(macOS)
            // For macOS, navigation bar is handled by the window controller
            if let macOSViewController = viewController as? macOSLxAppViewController,
               let windowController = macOSViewController.view.window?.windowController as? LxAppWindowController {
                windowController.updateWindowTitle(for: path)
            }
            #elseif os(iOS)
            // For iOS, use the traditional navigation bar approach
            viewController.ensureNavigationBarExists()

            if let iOSViewController = viewController as? iOSLxAppViewController,
               let navigationBar = iOSViewController.navigationBar {

                let title = pageConfig?.title_text.toString() ?? ""
                let bgColor = PlatformColor(argb: pageConfig?.background_color ?? 0xFFFFFFFF)
                let textColor = getTextColor(from: pageConfig?.text_style.toString())

                // Get TabBar config to check if this is a tab root page
                let tabBarConfig: TabBarConfig?
                if let existingConfig = viewController.getTabBar()?.config {
                    tabBarConfig = existingConfig
                } else {
                    // Fallback: get TabBar config directly from the app configuration
                    tabBarConfig = getTabBarConfig(appId)
                }

                let showBackButton = shouldShowBackButton(for: path, appId: appId, tabBarConfig: tabBarConfig)

                navigationBar.updateStateAndAnimate(
                    title: title,
                    bgColor: bgColor,
                    textColor: textColor,
                    showBackButton: showBackButton,
                    isBackNavigation: isBackNavigation,
                    disableAnimation: disableAnimation,
                    onBackClickListener: {
                        handleBackButtonClick(in: viewController)
                    }
                )
            }
            #endif
        } else {
            // NavigationBar should be hidden - remove it completely
            viewController.removeNavigationBar()
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
        if let iOSViewController = viewController as? iOSLxAppViewController {
            return iOSViewController.currentWebView?.currentPath
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
        if let iOSViewController = viewController as? iOSLxAppViewController {
            return iOSViewController.navigationBar != nil
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
    public static func findTabIndexByPath(_ targetPath: String, in config: TabBarConfig, appId: String) -> Int? {
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

    private static func shouldShowBackButton(for path: String, appId: String, tabBarConfig: TabBarConfig? = nil) -> Bool {
        return LxPageNavigation.shouldShowBackButton(for: path, appId: appId, tabBarConfig: tabBarConfig)
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
extension iOSLxAppViewController: LxAppViewControllerProtocol {
    public func getTabBar() -> (any TabBarProtocol)? {
        return tabBar as? (any TabBarProtocol)
    }

    public func syncTabBarWithCurrentPath(_ path: String) {
        tabBar?.syncSelectedTabWithCurrentPath(path)
    }

    /// Switches to tab (public interface)
    public func switchToTab(targetPath: String) {
        LxAppPageNavigation.switchToTab(targetPath: targetPath, in: self)
    }

    /// Switches page (public interface)
    public func switchPage(targetPath: String) {
        LxAppPageNavigation.switchPage(targetPath: targetPath, in: self)
    }

    /// Switches page via navigation (alias for consistency with unified API)
    public func switchPageViaNavigation(targetPath: String) {
        LxAppPageNavigation.switchPage(targetPath: targetPath, in: self)
    }

    /// Navigates to page (public interface)
    public func navigateToPage(targetPath: String, isReplace: Bool = false, isBackNavigation: Bool = false) {
        LxAppPageNavigation.navigateToPage(
            targetPath: targetPath,
            isReplace: isReplace,
            isBackNavigation: isBackNavigation,
            in: self
        )
    }

    /// Updates navigation bar (public interface) - unified signature
    public func updateNavigationBar(appId: String, path: String) {
        LxAppPageNavigation.updateNavigationBar(appId: appId, path: path, in: self)
    }

    // Removed duplicate getNavigationBarConfig - use LxPageNavigation.getNavigationBarConfig directly

    /// Handles back button click (public interface)
    public func handleBackButtonClick() {
        LxAppPageNavigation.handleBackButtonClick(in: self)
    }

    /// Updates tab bar selection for current path (public interface)
    public func updateTabBarSelection(for path: String) {
        LxAppPageNavigation.updateTabBarSelection(for: path, in: self)
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

    /// Check if a path is the initial route using cached data
    public static func isInitialRoute(appId: String, path: String) -> Bool {
        return initialRouteCache[appId] == path
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
    /// Returns nil if this is an initial route (should hide navbar) - except on macOS
    public static func getNavigationBarConfig(appId: String, path: String) -> NavigationBarConfig? {
        #if os(macOS)
        // macOS always returns config, even for initial route
        return lingxia.getNavigationBarConfig(appId, path)
        #else
        // iOS: return nil for initial route to hide navbar
        if isInitialRoute(appId: appId, path: path) {
            return nil
        }
        return lingxia.getNavigationBarConfig(appId, path)
        #endif
    }

    /// Determines if back button should be shown
    public static func shouldShowBackButton(for path: String, appId: String, tabBarConfig: TabBarConfig? = nil) -> Bool {
        // TODO: Implement proper logic for determining when to show back button
        // For now, never show back button to avoid navigation issues
        return false
    }

    /// Finds tab index by path in tab bar configuration
    public static func findTabIndexByPath(_ targetPath: String, in config: TabBarConfig, appId: String) -> Int {
        let items = config.getItems(appId: appId)
        return items.firstIndex { $0.page_path.toString() == targetPath } ?? -1
    }

    /// Determines navigation bar visibility from page config
    public static func shouldShowNavigationBar(pageConfig: NavigationBarConfig?) -> Bool {
        // If pageConfig is nil, it means this is an initial route - hide navbar
        // If pageConfig exists, check if style should hide navbar
        guard let config = pageConfig else { return false }
        return !config.style.shouldHide
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
        // Basic validation - path should not be empty and should be a valid page path
        return !path.isEmpty && !path.hasPrefix("http") && !path.hasPrefix("javascript:")
    }

    /// Extracts page title from configuration
    public static func getPageTitle(from pageConfig: NavigationBarConfig?, defaultTitle: String = "") -> String {
        return pageConfig?.title_text.toString() ?? defaultTitle
    }

    /// Determines if this is a tab navigation
    public static func isTabNavigation(targetPath: String, tabBarConfig: TabBarConfig?, appId: String) -> Bool {
        guard let config = tabBarConfig else { return false }
        return findTabIndexByPath(targetPath, in: config, appId: appId) >= 0
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
    public let pageConfig: NavigationBarConfig?

    public init(appId: String, targetPath: String, eventType: LxNavigationEventType, pageConfig: NavigationBarConfig? = nil) {
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

