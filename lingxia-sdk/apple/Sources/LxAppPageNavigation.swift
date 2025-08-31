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



/// Unified Page Navigation management for LxApp - supports both iOS and macOS
@MainActor
public class LxAppPageNavigation {

    /// Universal TabBar click handler - the ONLY way to handle tab clicks
    /// Future: This will be called directly by Rust layer with appId + path
    public static func handleTabClick(appId: String, path: String) {
        // Direct navigation call - no controller dependency
        // This is the pattern for future Rust integration
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
        iOSLxApp.navigate(appId: appId, path: path, navigationType: .switchTab)
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

/// Core page navigation logic shared between iOS and macOS
@MainActor
public struct LxPageNavigation {
    /// Cache for initial routes of each app
    private static var initialRouteCache: [String: String] = [:]

    /// Cache initial route for an app (called explicitly when app opens)
    public static func cacheInitialRoute(appId: String, initialRoute: String) {
        initialRouteCache[appId] = initialRoute
    }

    /// Gets page configuration from Rust layer using typed API
    public static func getNavigationBarState(appId: String, path: String) -> NavigationBarState? {
        return lingxia.getNavigationBarState(appId, path)
    }

    /// Finds tab index by path in tab bar configuration
    public static func findTabIndexByPath(_ targetPath: String, in config: TabBar, appId: String) -> Int {
        let items = config.getItems(appId: appId)
        return items.firstIndex { $0.page_path.toString() == targetPath } ?? -1
    }
}
