import Foundation

/// Extension to add helper methods to swift-bridge generated NavigationBarConfig
extension NavigationBarConfig {
    /// Check if navbar should be hidden based on style and route
    public func shouldBeHidden(appId: String, path: String) -> Bool {
        // Get initial route to determine if navbar should be hidden
        let lxappInfo = getLxAppInfo(appId)
        let initialRoute = lxappInfo.initial_route.toString()

        // Hide navbar if it's custom style OR if it's the initial route
        return navigation_style == 1 || path == initialRoute // 1 = NAVIGATION_STYLE_CUSTOM
    }

    // Helper constants
    static let DEFAULT_BACKGROUND_COLOR = "#FFFFFF"
    static let DEFAULT_TEXT_COLOR = "#000000"
    static let DEFAULT_HEIGHT: CGFloat = 44
}

/// Protocol for navigation bar implementations
@MainActor
public protocol NavigationBarProtocol: AnyObject {
    func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool,
        disableAnimation: Bool,
        onBackClickListener: (() -> Void)?,
        onAnimationEnd: (() -> Void)?
    ) -> Bool

    func setTitle(_ title: String?)
    func setBackButtonVisible(_ visible: Bool)
    func hide()
    func getCalculatedContentHeight() -> CGFloat
}

#if os(iOS)
import UIKit
public typealias NavigationBar = iOSNavigationBarImpl
public typealias PlatformNavigationBar = iOSNavigationBarImpl
#elseif os(macOS)
import Cocoa
public typealias NavigationBar = macOSNavigationBar
public typealias PlatformNavigationBar = macOSNavigationBar
#endif
