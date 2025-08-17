import SwiftUI
import Foundation

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// Navigation bar style enumeration
public enum NavigationBarStyle: Int32 {
    case `default` = 0  // Default navigation bar style
    case custom = 1     // Custom/transparent navigation bar style

    /// Check if this style should hide the navigation bar
    public var shouldHide: Bool {
        return self == .custom
    }

    /// Check if this style should be transparent
    public var isTransparent: Bool {
        return self == .custom
    }
}

/// Extension to add helper methods to swift-bridge generated NavigationBarConfig
extension NavigationBarConfig {
    /// Get the navigation bar style as an enum
    public var style: NavigationBarStyle {
        return NavigationBarStyle(rawValue: navigation_style) ?? .default
    }

    /// Check if navbar should be hidden based on style and route
    public func shouldBeHidden(appId: String, path: String) -> Bool {
        #if os(macOS)
        // macOS always shows NavigationBar, never hide
        return false
        #else
        // iOS platform: hide for custom style OR initial route
        let lxappInfo = getLxAppInfo(appId)
        let initialRoute = lxappInfo.initial_route.toString()
        return style.shouldHide || path == initialRoute
        #endif
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

/// Unified SwiftUI Navigation Bar
public struct LxAppNavigationBarView: View {
    let config: NavigationBarConfig?
    let isBackNavigation: Bool
    let onBackTapped: () -> Void
    @State private var isLoading: Bool = false

    public init(
        config: NavigationBarConfig?,
        isBackNavigation: Bool = false,
        onBackTapped: @escaping () -> Void = {}
    ) {
        self.config = config
        self.isBackNavigation = isBackNavigation
        self.onBackTapped = onBackTapped
    }

    public var body: some View {
        if let config = config {
            navigationContent(config: config)
                .frame(height: LxAppTheme.Metrics.navigationBarHeight + LxAppTheme.platform.statusBarHeight)
                .background(getBackgroundColor(config: config))
        }
    }

    private func navigationContent(config: NavigationBarConfig) -> some View {
        VStack(spacing: 0) {
            // Status bar spacer (iOS only)
            #if os(iOS)
            Rectangle()
                .fill(Color.clear)
                .frame(height: LxAppTheme.platform.statusBarHeight)
            #endif

            // Navigation bar content
            HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                // Leading content
                leadingContent(config: config)

                // Center content
                Spacer()
                centerContent(config: config)
                Spacer()

                // Trailing content
                trailingContent(config: config)
            }
            .padding(.horizontal, LxAppTheme.Metrics.largeSpacing)
            .frame(height: LxAppTheme.Metrics.navigationBarHeight)
        }
    }

    @ViewBuilder
    private func leadingContent(config: NavigationBarConfig) -> some View {
        if isBackNavigation {
            Button(action: onBackTapped) {
                HStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                    LxAppIcons.back
                        .font(.system(size: 18, weight: .medium))
                }
                .foregroundColor(getTextColor(config: config))
            }
            .buttonStyle(PlainButtonStyle())
        } else {
            // Placeholder for consistent spacing
            Color.clear.frame(width: 44, height: 44)
        }
    }

    @ViewBuilder
    private func centerContent(config: NavigationBarConfig) -> some View {
        if isLoading {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle())
                .scaleEffect(0.8)
        } else {
            Text(config.title_text.toString())
                .font(LxAppTheme.Typography.navigationTitle)
                .foregroundColor(getTextColor(config: config))
                .lineLimit(1)
                .offset(y: -8) // Move title up more to align with capsule button
        }
    }

    @ViewBuilder
    private func trailingContent(config: NavigationBarConfig) -> some View {
        // Placeholder for future actions (search, menu, etc.)
        Color.clear.frame(width: 44, height: 44)
    }

    private func getBackgroundColor(config: NavigationBarConfig) -> Color {
        let platformColor = PlatformColor(argb: config.background_color)
        return Color(platformColor)
    }

    private func getTextColor(config: NavigationBarConfig) -> Color {
        let textStyle = config.text_style.toString()
        return textStyle == "white" ? Color.white : Color.black
    }
}

/// SwiftUI ViewModifier for adding navigation bar to any view
public struct LxAppNavigationBarModifier: ViewModifier {
    let config: NavigationBarConfig?
    let isBackNavigation: Bool
    let onBackTapped: () -> Void

    public init(
        config: NavigationBarConfig?,
        isBackNavigation: Bool = false,
        onBackTapped: @escaping () -> Void = {}
    ) {
        self.config = config
        self.isBackNavigation = isBackNavigation
        self.onBackTapped = onBackTapped
    }

    public func body(content: Content) -> some View {
        VStack(spacing: 0) {
            LxAppNavigationBarView(
                config: config,
                isBackNavigation: isBackNavigation,
                onBackTapped: onBackTapped
            )
            content
        }
    }
}

#if os(iOS)
import UIKit

/// UIKit wrapper for SwiftUI LxAppNavigationBarView on iOS
@MainActor
public class iOSNavigationBarWrapper: UIView, NavigationBarProtocol {
    private var hostingController: UIHostingController<LxAppNavigationBarView>?
    private var currentConfig: NavigationBarConfig?
    private var isBackNavigation: Bool = false
    private var onBackClickListener: (() -> Void)?

    public override init(frame: CGRect) {
        super.init(frame: frame)
        setupWrapper()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupWrapper()
    }

    private func setupWrapper() {
        backgroundColor = UIColor.clear
    }

    public func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool,
        disableAnimation: Bool,
        onBackClickListener: (() -> Void)?,
        onAnimationEnd: (() -> Void)?
    ) -> Bool {
        self.currentConfig = pageConfig
        self.isBackNavigation = isBackNavigation
        self.onBackClickListener = onBackClickListener

        // Check if NavigationBar should be hidden
        let shouldHide = pageConfig?.style.shouldHide ?? false
        if shouldHide {
            hide()
            onAnimationEnd?()
            return false
        }

        updateSwiftUINavigationBar()
        onAnimationEnd?()
        return true
    }

    public func setTitle(_ title: String?) {
        if var config = currentConfig {
            config.title_text = RustString(title ?? "")
            currentConfig = config
            updateSwiftUINavigationBar()
        }
    }

    public func setBackButtonVisible(_ visible: Bool) {
        isBackNavigation = visible
        updateSwiftUINavigationBar()
    }

    public func hide() {
        isHidden = true
        hostingController?.view.isHidden = true
    }

    public func getCalculatedContentHeight() -> CGFloat {
        guard let config = currentConfig else {
            return NavigationBarConfig.DEFAULT_HEIGHT + PLATFORM_STATUS_BAR_HEIGHT
        }

        if config.shouldBeHidden(appId: "", path: "") {
            return 0
        }

        return NavigationBarConfig.DEFAULT_HEIGHT + PLATFORM_STATUS_BAR_HEIGHT
    }

    /// Set back button click listener (compatibility method)
    public func setOnBackButtonClickListener(_ listener: @escaping () -> Void) {
        self.onBackClickListener = listener
    }

    /// Update navigation bar state and animate (compatibility method)
    public func updateStateAndAnimate(
        title: String,
        bgColor: PlatformColor,
        textColor: PlatformColor,
        showBackButton: Bool,
        isBackNavigation: Bool,
        disableAnimation: Bool,
        onBackClickListener: @escaping () -> Void,
        onAnimationEnd: (() -> Void)? = nil
    ) {
        // Create a temporary config for the update
        var tempConfig = currentConfig ?? NavigationBarConfig(
            background_color: 0xFFFFFFFF,
            text_style: RustString(""),
            title_text: RustString(title),
            navigation_style: 0
        )
        tempConfig.title_text = RustString(title)
        // Convert PlatformColor to UInt32 ARGB
        tempConfig.background_color = bgColor.toARGB()

        let success = updateWithConfig(
            pageConfig: tempConfig,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            onBackClickListener: onBackClickListener,
            onAnimationEnd: onAnimationEnd
        )

        if !success {
            onAnimationEnd?()
        }
    }

    private func updateSwiftUINavigationBar() {
        // Remove existing hosting controller
        if let existingController = hostingController {
            existingController.view.removeFromSuperview()
            existingController.removeFromParent()
        }

        guard let config = currentConfig else { return }

        // Create new SwiftUI view
        let navigationBarView = LxAppNavigationBarView(
            config: config,
            isBackNavigation: isBackNavigation
        ) { [weak self] in
            self?.onBackClickListener?()
        }

        // Create hosting controller
        let hostingController = UIHostingController(rootView: navigationBarView)
        hostingController.view.backgroundColor = UIColor.clear
        self.hostingController = hostingController

        // Add to view hierarchy
        addSubview(hostingController.view)
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: topAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])

        isHidden = false
    }
}

public typealias LingXiaNavigationBar = iOSNavigationBarWrapper
public typealias PlatformNavigationBar = iOSNavigationBarWrapper
#elseif os(macOS)
import AppKit
public typealias LingXiaNavigationBar = LxAppNavigationBarView
public typealias PlatformNavigationBar = LxAppNavigationBarView
#endif

public extension View {
    /// Adds navigation bar to the view
    func lxAppNavigationBar(
        config: NavigationBarConfig?,
        isBackNavigation: Bool = false,
        onBackTapped: @escaping () -> Void = {}
    ) -> some View {
        self.modifier(
            LxAppNavigationBarModifier(
                config: config,
                isBackNavigation: isBackNavigation,
                onBackTapped: onBackTapped
            )
        )
    }
}
