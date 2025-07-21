#if os(iOS)
import UIKit
import Foundation

/// iOS-specific NavigationBar support utilities
@MainActor
public class iOSNavigationBarSupport {

    /// Creates a NavigationBar for iOS
    public static func createNavigationBar(frame: CGRect) -> NavigationBar {
        return NavigationBar(frame: frame)
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

    /// Determines if the device is a tablet
    public static func isTablet() -> Bool {
        return UIDevice.current.userInterfaceIdiom == .pad
    }

    /// Gets the appropriate navigation bar height for the device
    public static func getNavigationBarHeight() -> CGFloat {
        return isTablet() ? NavigationBar.DEFAULT_TABLET_HEIGHT : 44
    }

    /// Configures transparent system bars for edge-to-edge display
    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        if #available(iOS 13.0, *) {
            let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene
            let statusBarManager = windowScene?.statusBarManager

            if lightStatusBarIcons {
                viewController.overrideUserInterfaceStyle = .dark
            } else {
                viewController.overrideUserInterfaceStyle = .light
            }
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
