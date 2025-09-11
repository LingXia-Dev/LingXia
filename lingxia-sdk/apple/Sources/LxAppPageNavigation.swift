import SwiftUI
import Foundation
import os.log
import WebKit

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

/// Unified Page Navigation management for LxApp - supports both iOS and macOS
@MainActor
public class LxAppPageNavigation {

    /// Handle TabBar item selection - unified UI event system
    public static func handleTabBarItemSelected(appId: String, index: Int) {
        let _ = onUiEvent(appId, LxAppUIEvent.tabBarClick, String(index))
    }
}

/// Protocol for TabBar control operations
@MainActor
public protocol NavigationTabBarController {
    func findTabIndexByPath(_ path: String) -> Int?
    func setSelectedTabIndex(_ index: Int)
}

/// Protocol for UI update operations
@MainActor
public protocol NavigationUIUpdater {
    func showTabBar(_ show: Bool)
    func triggerTabBarRefresh()
}

/// Shared navigation operations for view controllers
@MainActor
public class LxAppViewControllerBase {
    private static let log = OSLog(subsystem: "LingXia", category: "ViewControllerBase")

    /// Shared navigation logic
    public static func handleNavigation(
        appId: String,
        path: String,
        navigationType: NavigationType,
        tabBarController: NavigationTabBarController,
        uiUpdater: NavigationUIUpdater,
        isTabPage: @escaping (String) -> Bool
    ) {
        // Swift should only display based on Rust-provided state
        // TabBar and NavBar state is managed by Rust, Swift just renders
        if let tabBarState = lingxia.getTabBar(appId) {
            uiUpdater.showTabBar(tabBarState.is_visible)
            if tabBarState.is_visible {
                tabBarController.setSelectedTabIndex(Int(tabBarState.selected_index))
                uiUpdater.triggerTabBarRefresh()
            }
        }
    }

    /// Shared TabBar sync logic
    public static func syncTabBarWithPath(_ path: String, appId: String, tabBarController: NavigationTabBarController) {
        if let tabIndex = tabBarController.findTabIndexByPath(path) {
            tabBarController.setSelectedTabIndex(tabIndex)
        }
    }
}

/// Navigation logic
@MainActor
public class LxAppNavigation {
    private static let log = OSLog(subsystem: "LingXia", category: "Navigation")

    /// Shared WebView switching logic - used by both platforms
    public static func switchToWebView(appId: String, path: String, currentWebView: WKWebView?, targetWebView: WKWebView?) -> Bool {
        guard let target = targetWebView else {
            os_log("Target WebView not found for %@:%@", log: log, type: .info, appId, path)
            return false
        }

        // Hide current WebView if different
        if let current = currentWebView, current != target {
            WebViewManager.switchWebView(from: current, to: target)
        }

        // Setup target WebView
        target.setup(appId: appId, path: path)
        return true
    }

    /// Shared navigation preparation logic - used by both platforms
    /// Rust prepares all states before navigate, Swift only reads them once here
    public static func prepareNavigation(appId: String, path: String, navigationType: NavigationType) -> NavigationPlan {

        guard !appId.isEmpty else {
            os_log("Empty appId provided for navigation", log: log, type: .error)
            return NavigationPlan.empty
        }

        // Get TabBar state from Rust once - Rust has prepared it before navigate
        let rustTabBarConfig = lingxia.getTabBar(appId)
        let shouldShowTabBar = rustTabBarConfig?.is_visible ?? false

        let tabBarState = TabBarState(
            show: shouldShowTabBar,
            updateSelection: true,
            selectedPath: path
        )
        let navBarState = NavBarState(
            shouldUpdate: true,
            appId: appId,
            path: path
        )
        let lifecycleAction = LifecycleAction.pageShow

        return NavigationPlan(
            appId: appId,
            path: path,
            navigationType: navigationType,
            tabBarState: tabBarState,
            navBarState: navBarState,
            lifecycleAction: lifecycleAction,
            shouldProceed: true
        )
    }

    /// Check if path corresponds to a TabBar item
    private static func isTabBarPage(appId: String, path: String) -> Bool {
        guard let tabConfig = lingxia.getTabBar(appId) else { return false }
        let items = tabConfig.getItems(appId: appId)
        return items.contains { $0.cachedPagePath == path }
    }
}

/// Core page navigation utilities
@MainActor
public struct LxPageNavigation {
    /// Gets navigation bar state from Rust layer
    public static func getNavigationBarState(appId: String, path: String) -> NavigationBarState? {
        guard LxAppCore.isInitialized(), !appId.isEmpty, !path.isEmpty else {
            return nil
        }

        return lingxia.getNavigationBarState(appId, path)
    }

    /// Finds tab index by path in tab bar configuration
    public static func findTabIndexByPath(_ targetPath: String, in config: TabBar, appId: String) -> Int {
        let items = config.getItems(appId: appId)
        return items.firstIndex { $0.page_path.toString() == targetPath } ?? -1
    }
}

/// Platform-specific rendering interface
@MainActor
public protocol LxAppRenderer {
    /// Handle platform-specific openLxApp setup (window/view creation)
    func openLxApp(appId: String, path: String)

    /// Handle platform-specific navigation logic
    func handlePlatformSpecificNavigation(_ plan: NavigationPlan)

    /// Render TabBar based on state
    func renderTabBar(_ state: TabBarState, appId: String, path: String)

    /// Render NavigationBar based on state
    func renderNavigationBar(_ state: NavBarState)

    /// Render Capsule button - only home app hides it, others show it
    func renderCapsuleButton(appId: String)
}

/// Plan for navigation execution
public struct NavigationPlan: Sendable {
    let appId: String
    let path: String
    let navigationType: NavigationType
    let tabBarState: TabBarState
    let navBarState: NavBarState
    let lifecycleAction: LifecycleAction
    let shouldProceed: Bool

    @MainActor
    static let empty = NavigationPlan(
        appId: "", path: "", navigationType: .launch,
        tabBarState: TabBarState.hidden,
        navBarState: NavBarState.noUpdate,
        lifecycleAction: .pageShow,
        shouldProceed: false
    )
}

/// TabBar state configuration
public struct TabBarState: Sendable {
    let show: Bool
    let updateSelection: Bool
    let selectedPath: String?

    @MainActor
    static let hidden = TabBarState(show: false, updateSelection: false, selectedPath: nil)
}

/// NavigationBar state configuration
public struct NavBarState: Sendable {
    let shouldUpdate: Bool
    let appId: String
    let path: String

    @MainActor
    static let noUpdate = NavBarState(shouldUpdate: false, appId: "", path: "")
}

/// Lifecycle actions to be executed
public enum LifecycleAction: Sendable {
    case openApp
    case switchTab
    case pageShow
}
