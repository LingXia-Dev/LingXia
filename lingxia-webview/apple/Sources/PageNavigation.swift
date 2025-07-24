import Foundation

/// Core page navigation logic shared between iOS and macOS
@MainActor
public struct PageNavigationCore {

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
    public static func parseNavigationParams(from targetPath: String) -> NavigationParams {
        let isReplace = targetPath.contains("?replace=true")
        let cleanPath = targetPath.replacingOccurrences(of: "?replace=true", with: "")

        return NavigationParams(
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
        // If pageConfig exists, check navigation_style (1 = hidden)
        guard let config = pageConfig else { return false }
        return config.navigation_style != 1
    }

    /// Gets text color from navigation bar text style
    public static func getTextColorFromStyle(_ textStyle: String?) -> TextColorInfo {
        let style = textStyle ?? "black"
        let isWhiteText = style.lowercased() == "white"

        return TextColorInfo(
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

/// Navigation parameters parsed from target path
public struct NavigationParams {
    public let cleanPath: String
    public let isReplace: Bool

    public init(cleanPath: String, isReplace: Bool) {
        self.cleanPath = cleanPath
        self.isReplace = isReplace
    }
}

/// Text color information for navigation bar
public struct TextColorInfo {
    public let isWhiteText: Bool
    public let colorString: String

    public init(isWhiteText: Bool, colorString: String) {
        self.isWhiteText = isWhiteText
        self.colorString = colorString
    }
}

/// Navigation event types
public enum NavigationEventType {
    case pageSwitch
    case tabSwitch
    case backNavigation
    case replace
}

/// Navigation context information
public struct NavigationContext {
    public let appId: String
    public let targetPath: String
    public let eventType: NavigationEventType
    public let pageConfig: NavigationBarConfig?

    public init(appId: String, targetPath: String, eventType: NavigationEventType, pageConfig: NavigationBarConfig? = nil) {
        self.appId = appId
        self.targetPath = targetPath
        self.eventType = eventType
        self.pageConfig = pageConfig
    }
}

/// Protocol for platform-specific navigation implementations
@MainActor
public protocol PlatformNavigationHandler {
    associatedtype ViewControllerType

    /// Updates navigation bar for the platform
    static func updateNavigationBar(
        context: NavigationContext,
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
