#if os(iOS)
import UIKit
import Foundation
import os.log

/// iOS-specific Navigation Bar implementation
public class iOSNavigationBarImpl: UIView {
    private static let log = OSLog(subsystem: "LingXia", category: "NavigationBar")

    /// Compatibility property for view access
    public var view: UIView { return self }

    internal static let DEFAULT_BACKGROUND_COLOR = UIColor.white
    internal static let DEFAULT_FRONT_COLOR = UIColor.black
    internal static let DEFAULT_TABLET_HEIGHT: CGFloat = 44

    private let titleLabel: UILabel
    private let loadingIndicator: UIActivityIndicatorView
    private let backButton: UIButton
    private var currentConfig: NavigationBarConfig = NavigationBarConfig(
        background_color: RustString(""),
        text_style: RustString(""),
        title_text: RustString(""),
        navigation_style: 0
    )
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
        let _ = isTablet ? iOSNavigationBarImpl.DEFAULT_TABLET_HEIGHT : 44

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

        NSLayoutConstraint.activate([
            // Back button constraints
            backButton.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            backButton.topAnchor.constraint(equalTo: topAnchor, constant: 48 + 8 - 2),
            backButton.widthAnchor.constraint(equalToConstant: 44),
            backButton.heightAnchor.constraint(equalToConstant: 32),

            // Title label constraints
            titleLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 48 + 8),
            titleLabel.leadingAnchor.constraint(greaterThanOrEqualTo: backButton.trailingAnchor, constant: 8),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -52),

            // Loading indicator constraints
            loadingIndicator.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            loadingIndicator.topAnchor.constraint(equalTo: topAnchor, constant: 48 + 8),
            loadingIndicator.widthAnchor.constraint(equalToConstant: 24),
            loadingIndicator.heightAnchor.constraint(equalToConstant: 24)
        ])
    }

    public func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool,
        disableAnimation: Bool,
        onBackClickListener: @escaping () -> Void,
        onAnimationEnd: (() -> Void)?
    ) -> Bool {
        // Check if NavigationBar should be hidden (using navigation_style)
        let shouldHide = pageConfig?.navigation_style == 1 // 1 = hidden
        if shouldHide {
            hide()
            return false
        }

        // Extract configuration values with defaults
        let titleText = pageConfig?.title_text.toString() ?? ""
        let backgroundColorString = pageConfig?.background_color.toString() ?? NavigationBarConfig.DEFAULT_BACKGROUND_COLOR
        let backgroundColor = UIColor(hexString: backgroundColorString) ?? UIColor.white
        let textStyle = pageConfig?.text_style.toString() ?? "black"
        let textColor = textStyle == "white" ? UIColor.white : UIColor.black
        let showBackButton = isBackNavigation && !disableAnimation

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

    public func setTitle(_ title: String?) {
        titleLabel.text = title ?? ""
    }

    public func setBackButtonVisible(_ visible: Bool) {
        backButton.isHidden = !visible
    }

    public func hide() {
        isHidden = true
    }

    public func getCalculatedContentHeight() -> CGFloat {
        let isTablet = UIDevice.current.userInterfaceIdiom == .pad
        return isTablet ? iOSNavigationBarImpl.DEFAULT_TABLET_HEIGHT : 44
    }

    public func showLoading() {
        loadingIndicator.startAnimating()
    }

    public func hideLoading() {
        loadingIndicator.stopAnimating()
    }

    public func setColor(backgroundColor: UIColor, frontColor: UIColor) {
        currentBackgroundColor = backgroundColor
        currentFrontColor = frontColor

        self.backgroundColor = currentBackgroundColor
        titleLabel.textColor = currentFrontColor
        loadingIndicator.color = currentFrontColor
        backButton.setTitleColor(currentFrontColor, for: .normal)
    }

    public func setOnBackButtonClickListener(_ listener: @escaping () -> Void) {
        onBackClickListener = listener
    }

    @objc private func backButtonTapped() {
        onBackClickListener?()
    }

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

        // No animation for NavigationBar - it should appear instantly
        transform = .identity
        onAnimationEnd?()
    }

    public func setExternalStatusBarHeight(_ sbh: CGFloat) {
        if knownStatusBarHeight != sbh {
            knownStatusBarHeight = sbh
            setNeedsUpdateConstraints()
        }
    }
}

/// iOS-specific NavigationBar support utilities
@MainActor
public class iOSNavigationBarSupport {

    /// Creates a NavigationBar for iOS
    public static func createNavigationBar(frame: CGRect) -> iOSNavigationBarImpl {
        return iOSNavigationBarImpl(frame: frame)
    }

    /// Determines if the device is a tablet
    public static func isTablet() -> Bool {
        return UIDevice.current.userInterfaceIdiom == .pad
    }

    /// Gets the safe area insets for the current device
    public static func getSafeAreaInsets() -> UIEdgeInsets {
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
           let window = windowScene.windows.first {
            return window.safeAreaInsets
        }
        return UIEdgeInsets.zero
    }

    /// Gets the status bar height
    public static func getStatusBarHeight() -> CGFloat {
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
            return windowScene.statusBarManager?.statusBarFrame.height ?? 0
        }
        return 0
    }

    /// Gets the appropriate navigation bar height for the device
    public static func getNavigationBarHeight() -> CGFloat {
        let isTablet = UIDevice.current.userInterfaceIdiom == .pad
        return isTablet ? iOSNavigationBarImpl.DEFAULT_TABLET_HEIGHT : 44
    }

    /// Configures transparent system bars for edge-to-edge display
    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        if #available(iOS 13.0, *) {
            let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene
            let _ = windowScene?.statusBarManager
        }
    }

    /// Updates navigation bar transparency based on tab bar configuration
    public static func updateNavigationBarTransparency(viewController: UIViewController, isTabBarTransparent: Bool, tabBarBackgroundColor: UIColor? = nil) {
        guard let navigationController = viewController.navigationController else { return }

        if #available(iOS 13.0, *) {
            let appearance = UINavigationBarAppearance()

            if isTabBarTransparent {
                appearance.configureWithTransparentBackground()
                appearance.backgroundColor = UIColor.clear
            } else {
                appearance.configureWithOpaqueBackground()
                appearance.backgroundColor = tabBarBackgroundColor ?? UIColor.systemBackground
            }

            navigationController.navigationBar.standardAppearance = appearance
            navigationController.navigationBar.scrollEdgeAppearance = appearance
        }
    }
}

#endif
