import Foundation
import os.log

#if os(iOS)
import UIKit
#elseif os(macOS)
import Cocoa
#endif

/// Configuration data class for the NavigationBar
public struct NavigationBarConfig {
    let hidden: Bool
    let navigationBarBackgroundColor: PlatformColor?
    let navigationBarTextStyle: String?
    let navigationBarTitleText: String?
    let navigationStyle: String?

    static let DEFAULT_BACKGROUND_COLOR = PlatformColor.white
    static let DEFAULT_TEXT_COLOR = PlatformColor.black
    static let DEFAULT_HEIGHT: CGFloat = 44

    public init(
        hidden: Bool = false,
        navigationBarBackgroundColor: PlatformColor? = nil,
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
                navigationBarBackgroundColor: parseColor(jsonObject["navigationBarBackgroundColor"] as? String, defaultColor: DEFAULT_BACKGROUND_COLOR),
                navigationBarTextStyle: textStyle,
                navigationBarTitleText: jsonObject["navigationBarTitleText"] as? String ?? "",
                navigationStyle: navStyle
            )
        } catch {
            return NavigationBarConfig(hidden: true)
        }
    }

    private static func parseColor(_ colorString: String?, defaultColor: PlatformColor) -> PlatformColor {
        guard let colorString = colorString, !colorString.isEmpty else { return defaultColor }

        if colorString.hasPrefix("#") {
            return PlatformColor(hexString: colorString) ?? defaultColor
        }
        return defaultColor
    }
}

/// Cross-platform NavigationBar for both iOS and macOS
@MainActor
public class NavigationBar {
    #if os(iOS)
    private let iOSNavigationBar: iOSNavigationBarImpl

    public init(frame: CGRect) {
        self.iOSNavigationBar = iOSNavigationBarImpl(frame: frame)
    }

    public var view: UIView { return iOSNavigationBar }
    public var bottomAnchor: NSLayoutYAxisAnchor { return view.bottomAnchor }
    public var isHidden: Bool {
        get { return iOSNavigationBar.isHidden }
        set { iOSNavigationBar.isHidden = newValue }
    }

    public func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool = false,
        disableAnimation: Bool = false,
        onBackClickListener: (() -> Void)? = nil,
        onAnimationEnd: (() -> Void)? = nil
    ) -> Bool {
        return iOSNavigationBar.updateWithConfig(
            pageConfig: pageConfig,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            onBackClickListener: onBackClickListener ?? {},
            onAnimationEnd: onAnimationEnd
        )
    }

    public func setTitle(_ title: String?) {
        iOSNavigationBar.setTitle(title)
    }

    public func setBackButtonVisible(_ visible: Bool) {
        iOSNavigationBar.setBackButtonVisible(visible)
    }

    public func hide() {
        iOSNavigationBar.hide()
    }

    public func getCalculatedContentHeight() -> CGFloat {
        return iOSNavigationBar.getCalculatedContentHeight()
    }

    public func setOnBackButtonClickListener(_ listener: @escaping () -> Void) {
        iOSNavigationBar.setOnBackButtonClickListener(listener)
    }

    public func updateStateAndAnimate(
        title: String,
        bgColor: UIColor,
        textColor: UIColor,
        showBackButton: Bool,
        isBackNavigation: Bool = false,
        disableAnimation: Bool = false,
        onBackClickListener: @escaping () -> Void = {},
        onAnimationEnd: (() -> Void)? = nil
    ) {
        iOSNavigationBar.updateStateAndAnimate(
            title: title,
            bgColor: bgColor,
            textColor: textColor,
            showBackButton: showBackButton,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            onBackClickListener: onBackClickListener,
            onAnimationEnd: onAnimationEnd
        )
    }
    #elseif os(macOS)
    private let macOSNavigationBar: macOSNavigationBar

    public init(frame: CGRect) {
        let nsRect = NSRect(x: frame.origin.x, y: frame.origin.y, width: frame.width, height: frame.height)
        self.macOSNavigationBar = lingxia.macOSNavigationBar(frame: nsRect)
    }

    public var view: NSView { return macOSNavigationBar }

    public func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool = false,
        disableAnimation: Bool = false,
        onBackClickListener: (() -> Void)? = nil,
        onAnimationEnd: (() -> Void)? = nil
    ) -> Bool {
        macOSNavigationBar.updateWithConfig(
            pageConfig: pageConfig,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            onBackClickListener: onBackClickListener ?? {},
            onAnimationEnd: onAnimationEnd
        )
        return true
    }

    public func setTitle(_ title: String?) {
        // macOS implementation handles title in updateWithConfig
    }

    public func setBackButtonVisible(_ visible: Bool) {
        // macOS doesn't have back button in navigation bar
    }

    public func hide() {
        macOSNavigationBar.isHidden = true
    }

    public func getCalculatedContentHeight() -> CGFloat {
        return 44 // Default height for macOS
    }
    #endif
}
