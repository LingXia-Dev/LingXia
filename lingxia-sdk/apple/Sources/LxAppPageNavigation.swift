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

    /// Universal TabBar click handler
    public static func handleTabClick(appId: String, path: String) {
        #if os(iOS)
        iOSLxApp.navigate(appId: appId, path: path, navigationType: .switchTab)
        #elseif os(macOS)
        macOSLxApp.navigate(appId: appId, path: path, navigationType: .switchTab)
        #endif
    }

    /// Universal page navigation handler - for all navigation types
    /// Future: This will be the main entry point from Rust layer
    public static func handleNavigation(appId: String, path: String, navigationType: NavigationType) {
        // Direct navigation call - no controller dependency
        // Key parameters: appId + path (perfect for Rust integration)
        #if os(iOS)
        iOSLxApp.navigate(appId: appId, path: path, navigationType: navigationType)
        #elseif os(macOS)
        macOSLxApp.navigate(appId: appId, path: path, navigationType: navigationType)
        #endif
    }

    /// Create tab click closure for current Swift-based TabBars
    /// This is temporary - will be removed when Rust takes over
    public static func tabClickHandler(appId: String) -> (Int, String) -> Void {
        return { index, path in
            handleTabClick(appId: appId, path: path)
        }
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

/// Shared navigation logic
@MainActor
public class LxAppSharedNavigation {
    nonisolated(unsafe) private static let log = OSLog(subsystem: "LingXia", category: "Navigation")

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

    /// Resolve navigation type based on path and context
    public static func resolveNavigationType(_ navigationType: NavigationType, for path: String, isTabPage: (String) -> Bool) -> NavigationType {
        switch navigationType {
        case .launch:
            // Launch: check if it's a tab page
            if isTabPage(path) {
                return .switchTab  // Convert to tab switch
            } else {
                return .launch     // Keep as launch (will hide tabbar)
            }
        default:
            return navigationType  // Keep original type
        }
    }

    /// Apply navigation type specific UI updates
    public static func applyNavigationTypeSpecificUpdates(
        navigationType: NavigationType,
        path: String,
        appId: String,
        tabBarController: NavigationTabBarController,
        uiUpdater: NavigationUIUpdater
    ) {
        switch navigationType {
        case .switchTab:
            // TabSwitch: Set selected TabBar item and ensure visibility
            if let tabIndex = tabBarController.findTabIndexByPath(path) {
                tabBarController.setSelectedTabIndex(tabIndex)
                uiUpdater.showTabBar(true)
                uiUpdater.triggerTabBarRefresh()
            }

        case .launch:
            // Launch (non-tab page): Hide TabBar
            uiUpdater.showTabBar(false)

        case .replace:
            // Replace: Hide TabBar, update NavBar
            uiUpdater.showTabBar(false)

        case .forward, .backward:
            // Forward/Backward: Hide TabBar
            uiUpdater.showTabBar(false)
        }
    }

    /// Shared navigation preparation logic - used by both platforms
    public static func prepareNavigation(appId: String, path: String, navigationType: NavigationType) -> NavigationPlan {

        guard !appId.isEmpty else {
            os_log("Empty appId provided for navigation", log: log, type: .error)
            return NavigationPlan.empty
        }

        // Determine UI component states based on navigation type (macOS reference logic)
        let tabBarState = determineTabBarState(appId: appId, path: path, navigationType: navigationType)
        let navBarState = determineNavBarState(appId: appId, path: path, navigationType: navigationType)
        let lifecycleAction = determineLifecycleAction(navigationType: navigationType)

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

    /// Determine TabBar visibility and behavior
    private static func determineTabBarState(appId: String, path: String, navigationType: NavigationType) -> TabBarState {
        switch navigationType {
        case .switchTab:
            return TabBarState(show: true, updateSelection: true, selectedPath: path)
        case .launch:
            return isTabBarPage(appId: appId, path: path) ?
                TabBarState(show: true, updateSelection: true, selectedPath: path) :
                TabBarState(show: false, updateSelection: false, selectedPath: nil)
        default:
            return TabBarState(show: true, updateSelection: false, selectedPath: nil)
        }
    }

    /// Determine NavigationBar state
    private static func determineNavBarState(appId: String, path: String, navigationType: NavigationType) -> NavBarState {
        return NavBarState(shouldUpdate: true, appId: appId, path: path)
    }

    /// Determine lifecycle action based on navigation type
    private static func determineLifecycleAction(navigationType: NavigationType) -> LifecycleAction {
        switch navigationType {
        case .launch: return .openApp
        case .switchTab: return .switchTab
        case .forward: return .pageShow
        case .backward: return .backPressed
        case .replace: return .pageShow
        }
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

    /// Execute lifecycle action
    func executeLifecycleAction(_ action: LifecycleAction, appId: String, path: String)

    /// Get current path for duplicate navigation check
    func getCurrentPath(for appId: String) -> String?
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
    case backPressed
}
