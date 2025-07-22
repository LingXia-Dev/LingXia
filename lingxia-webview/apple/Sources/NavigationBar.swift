import Foundation

/// Configuration data class for the NavigationBar
public struct NavigationBarConfig {
    let hidden: Bool
    let navigationBarBackgroundColor: String?
    let navigationBarTextStyle: String?
    let navigationBarTitleText: String?
    let navigationStyle: String?

    static let DEFAULT_BACKGROUND_COLOR = "#FFFFFF"
    static let DEFAULT_TEXT_COLOR = "#000000"
    static let DEFAULT_HEIGHT: CGFloat = 44

    public init(
        hidden: Bool = false,
        navigationBarBackgroundColor: String? = nil,
        navigationBarTextStyle: String? = nil,
        navigationBarTitleText: String? = nil,
        navigationStyle: String? = nil
    ) {
        self.hidden = hidden
        self.navigationBarBackgroundColor = navigationBarBackgroundColor
        self.navigationBarTextStyle = navigationBarTextStyle
        self.navigationBarTitleText = navigationBarTitleText
        self.navigationStyle = navigationStyle
    }

    public static func fromJson(_ json: String?) -> NavigationBarConfig? {
        guard let json = json, !json.isEmpty else {
            return NavigationBarConfig(hidden: true)
        }

        do {
            guard let data = json.data(using: .utf8),
                  let jsonObject = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                return NavigationBarConfig(hidden: true)
            }

            let navStyle = jsonObject["navigationStyle"] as? String ?? "default"
            let isHidden = (jsonObject["hidden"] as? Bool ?? false) || navStyle == "custom"
            let textStyle = jsonObject["navigationBarTextStyle"] as? String ?? "black"

            return NavigationBarConfig(
                hidden: isHidden,
                navigationBarBackgroundColor: jsonObject["navigationBarBackgroundColor"] as? String,
                navigationBarTextStyle: textStyle,
                navigationBarTitleText: jsonObject["navigationBarTitleText"] as? String ?? "",
                navigationStyle: navStyle
            )
        } catch {
            return NavigationBarConfig(hidden: true)
        }
    }
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
