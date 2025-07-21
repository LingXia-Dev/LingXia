import Foundation

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

#if os(iOS)
/// iOS-specific Navigation Bar implementation
public class iOSNavigationBarImpl: UIView {
    private static let log = OSLog(subsystem: "LingXia", category: "NavigationBar")

    internal static let DEFAULT_BACKGROUND_COLOR = UIColor.white
    internal static let DEFAULT_FRONT_COLOR = UIColor.black
    private static let DEFAULT_TABLET_HEIGHT: CGFloat = 44

    private let titleLabel: UILabel
    private let loadingIndicator: UIActivityIndicatorView
    private let backButton: UIButton
    private var currentConfig: NavigationBarConfig = NavigationBarConfig()
    private var knownStatusBarHeight: CGFloat = 0

    private var currentBackgroundColor = DEFAULT_BACKGROUND_COLOR
    private var currentFrontColor = DEFAULT_FRONT_COLOR

    private var onBackClickListener: (() -> Void)?

    public override init(frame: CGRect) {
        titleLabel = UILabel()
        loadingIndicator = UIActivityIndicatorView(style: .medium)
        backButton = UIButton(type: .custom)

        super.init(frame: frame)
        setupUI()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupUI() {
        backgroundColor = currentBackgroundColor

        let isTablet = UIDevice.current.userInterfaceIdiom == .pad
        let _ = isTablet ? NavigationBar.DEFAULT_TABLET_HEIGHT : 44

        backButton.setTitle("‹", for: .normal)
        backButton.setTitleColor(currentFrontColor, for: .normal)
        backButton.titleLabel?.font = UIFont.systemFont(ofSize: 24, weight: .medium)
        backButton.contentHorizontalAlignment = .center
        backButton.isHidden = true
        backButton.addTarget(self, action: #selector(backButtonTapped), for: .touchUpInside)

        let targetTitleSize: CGFloat = isTablet ? 12 : 17

        titleLabel.textAlignment = .center
        titleLabel.textColor = currentFrontColor
        titleLabel.font = UIFont.systemFont(ofSize: targetTitleSize, weight: .medium)
        titleLabel.numberOfLines = 1

        loadingIndicator.color = currentFrontColor
        loadingIndicator.hidesWhenStopped = true

        addSubview(backButton)
        addSubview(titleLabel)
        addSubview(loadingIndicator)

        setupConstraints()
    }

    private func setupConstraints() {
        backButton.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        loadingIndicator.translatesAutoresizingMaskIntoConstraints = false

        // Use safe area guide for better layout on different devices
        let _ = safeAreaLayoutGuide

        NSLayoutConstraint.activate([
            // Back button constraints
            backButton.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            backButton.topAnchor.constraint(equalTo: topAnchor, constant: 48 + 8 - 2),
            backButton.widthAnchor.constraint(equalToConstant: 44),
            backButton.heightAnchor.constraint(equalToConstant: 32),

            // Title label constraints - ensure it doesn't overlap with back button
            titleLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 48 + 8),
            titleLabel.leadingAnchor.constraint(greaterThanOrEqualTo: backButton.trailingAnchor, constant: 8),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -52), // Leave space for potential right button

            // Loading indicator constraints
            loadingIndicator.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            loadingIndicator.topAnchor.constraint(equalTo: topAnchor, constant: 48 + 8),
            loadingIndicator.widthAnchor.constraint(equalToConstant: 24),
            loadingIndicator.heightAnchor.constraint(equalToConstant: 24)
        ])
    }

    /// Returns the calculated content height based on device type
    public func getCalculatedContentHeight() -> CGFloat {
        let isTablet = UIDevice.current.userInterfaceIdiom == .pad
        return isTablet ? NavigationBar.DEFAULT_TABLET_HEIGHT : 44
    }

    /// Shows the loading indicator
    public func showLoading() {
        loadingIndicator.startAnimating()
    }

    /// Hides the loading indicator
    public func hideLoading() {
        loadingIndicator.stopAnimating()
    }

    /// Sets the title text
    public func setTitle(_ title: String?) {
        titleLabel.text = title ?? ""
    }

    /// Sets the background and front colors
    public func setColor(backgroundColor: UIColor, frontColor: UIColor) {
        currentBackgroundColor = backgroundColor
        currentFrontColor = frontColor

        self.backgroundColor = currentBackgroundColor
        titleLabel.textColor = currentFrontColor
        loadingIndicator.color = currentFrontColor
        backButton.setTitleColor(currentFrontColor, for: .normal)
    }

    /// Sets the visibility of the back button
    public func setBackButtonVisible(_ visible: Bool) {
        backButton.isHidden = !visible
    }

    /// Sets a listener for back button clicks
    public func setOnBackButtonClickListener(_ listener: @escaping () -> Void) {
        onBackClickListener = listener
    }

    @objc private func backButtonTapped() {
        onBackClickListener?()
    }

    /// Hides the navigation bar
    public func hide() {
        isHidden = true
    }

    /// Updates the NavigationBar with provided configuration
    /// Returns true if NavigationBar should be shown, false if it should be hidden
    public func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool,
        disableAnimation: Bool,
        onBackClickListener: @escaping () -> Void,
        onAnimationEnd: (() -> Void)? = nil
    ) -> Bool {
        // Check if NavigationBar should be hidden
        let shouldHide = pageConfig?.hidden ?? false
        if shouldHide {
            hide()
            return false
        }

        // Extract configuration values with defaults
        let titleText = pageConfig?.navigationBarTitleText ?? ""
        let backgroundColor = pageConfig?.navigationBarBackgroundColor ?? NavigationBarConfig.DEFAULT_BACKGROUND_COLOR
        let textStyle = pageConfig?.navigationBarTextStyle ?? "black"
        let textColor = textStyle == "white" ? UIColor.white : UIColor.black
        let showBackButton = !disableAnimation

        // Update state with provided configuration
        updateStateAndAnimate(
            title: titleText,
            bgColor: backgroundColor,
            textColor: textColor,
            showBackButton: showBackButton,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            onBackClickListener: onBackClickListener,
            onAnimationEnd: onAnimationEnd
        )

        return true
    }

    /// Updates the state of the NavigationBar and optionally animates the transition
    public func updateStateAndAnimate(
        title: String,
        bgColor: UIColor,
        textColor: UIColor,
        showBackButton: Bool,
        isBackNavigation: Bool,
        disableAnimation: Bool,
        onBackClickListener: @escaping () -> Void,
        onAnimationEnd: (() -> Void)? = nil
    ) {
        isHidden = false

        setTitle(title)
        setColor(backgroundColor: bgColor, frontColor: textColor)
        setBackButtonVisible(showBackButton)
        setOnBackButtonClickListener(onBackClickListener)

        if !disableAnimation {
            let animStartX: CGFloat = isBackNavigation ? -frame.width : frame.width
            let duration: TimeInterval = 0.25

            transform = CGAffineTransform(translationX: animStartX, y: 0)

            UIView.animate(withDuration: duration, animations: {
                self.transform = .identity
            }) { _ in
                self.transform = .identity
                onAnimationEnd?()
            }
        } else {
            transform = .identity
            onAnimationEnd?()
        }
    }

    /// Updates status bar height for layout calculations
    public func setExternalStatusBarHeight(_ sbh: CGFloat) {
        if knownStatusBarHeight != sbh {
            knownStatusBarHeight = sbh
            setNeedsUpdateConstraints()
        }
    }
}

#endif

#if os(macOS)
/// macOS NavigationBar implementation
@MainActor
public class macOSNavigationBar: NSView {
    private static let HEIGHT: CGFloat = 32
    private var titleLabel: NSTextField!
    private var bottomBorder: NSView!

    public override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.white.cgColor

        bottomBorder = NSView()
        bottomBorder.wantsLayer = true
        bottomBorder.layer?.backgroundColor = NSColor.lightGray.withAlphaComponent(0.5).cgColor
        bottomBorder.translatesAutoresizingMaskIntoConstraints = false
        addSubview(bottomBorder)

        titleLabel = NSTextField(labelWithString: "")
        titleLabel.font = NSFont.systemFont(ofSize: 17, weight: .semibold)
        titleLabel.textColor = NSColor.black
        titleLabel.alignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(titleLabel)

        NSLayoutConstraint.activate([
            bottomBorder.leadingAnchor.constraint(equalTo: leadingAnchor),
            bottomBorder.trailingAnchor.constraint(equalTo: trailingAnchor),
            bottomBorder.bottomAnchor.constraint(equalTo: bottomAnchor),
            bottomBorder.heightAnchor.constraint(equalToConstant: 1),
            titleLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor)
        ])
    }

    public func updateWithConfig(pageConfig: NavigationBarConfig?) {
        guard let config = pageConfig else {
            titleLabel.stringValue = ""
            titleLabel.textColor = NSColor.black
            layer?.backgroundColor = NSColor.white.cgColor
            return
        }

        titleLabel.stringValue = config.navigationBarTitleText ?? ""
        titleLabel.textColor = config.navigationBarTextStyle == "white" ? NSColor.white : NSColor.black
        layer?.backgroundColor = config.navigationBarBackgroundColor?.cgColor ?? NSColor.white.cgColor
    }

    public static func createForWindow(_ window: NSWindow) -> macOSNavigationBar? {
        guard let contentView = window.contentView else { return nil }

        let navigationBar = macOSNavigationBar(frame: NSRect(
            x: 0,
            y: contentView.frame.height - HEIGHT,
            width: contentView.frame.width,
            height: HEIGHT
        ))
        navigationBar.autoresizingMask = [.width, .minYMargin]
        return navigationBar
    }
}
#endif

/// Cross-platform NavigationBar for both iOS and macOS
@MainActor
public class NavigationBar {
    #if os(iOS)
    private let iOSNavigationBar: iOSNavigationBarImpl

    public init(frame: CGRect) {
        self.iOSNavigationBar = iOSNavigationBarImpl(frame: frame)
    }

    public var view: UIView { return iOSNavigationBar }

    #elseif os(macOS)
    private let macOSNavigationBar: macOSNavigationBar

    public init(frame: CGRect) {
        let nsRect = NSRect(x: frame.origin.x, y: frame.origin.y, width: frame.width, height: frame.height)
        self.macOSNavigationBar = lingxia.macOSNavigationBar(frame: nsRect)
    }

    public var view: NSView { return macOSNavigationBar }

    #endif

    public func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool = false,
        disableAnimation: Bool = false,
        onBackClickListener: (() -> Void)? = nil,
        onAnimationEnd: (() -> Void)? = nil
    ) -> Bool {
        #if os(iOS)
        return iOSNavigationBar.updateWithConfig(
            pageConfig: pageConfig,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            onBackClickListener: onBackClickListener ?? {},
            onAnimationEnd: onAnimationEnd
        )
        #elseif os(macOS)
        macOSNavigationBar.updateWithConfig(pageConfig: pageConfig)
        return true
        #endif
    }

    public func setTitle(_ title: String?) {
        #if os(iOS)
        iOSNavigationBar.setTitle(title)
        #elseif os(macOS)
        // macOS implementation handles title in updateWithConfig
        #endif
    }

    public func setBackButtonVisible(_ visible: Bool) {
        #if os(iOS)
        iOSNavigationBar.setBackButtonVisible(visible)
        #elseif os(macOS)
        // macOS doesn't have back button in navigation bar
        #endif
    }

    public func hide() {
        #if os(iOS)
        iOSNavigationBar.hide()
        #elseif os(macOS)
        macOSNavigationBar.isHidden = true
        #endif
    }
}

// Keep the original iOS implementation for compatibility
#if os(iOS)
extension iOSNavigationBarImpl {
    // Already implements all required methods
}
#endif
