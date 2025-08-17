#if os(iOS)
import UIKit
import SwiftUI
import Foundation

/// iOS-specific view controller support utilities with SwiftUI integration
@MainActor
public class iOSViewControllerHelper {

    /// Configures edge-to-edge display for the view controller
    public static func configureEdgeToEdgeDisplay(_ viewController: UIViewController) {
        viewController.edgesForExtendedLayout = [.top, .bottom, .left, .right]
        viewController.extendedLayoutIncludesOpaqueBars = true
        // Modern content inset adjustment behavior (iOS 11+)
        viewController.additionalSafeAreaInsets = UIEdgeInsets.zero
    }

    /// Sets transparent background for the view controller
    public static func setTransparentBackground(_ viewController: UIViewController) {
        viewController.view.backgroundColor = UIColor.clear

        if let navigationController = viewController.navigationController {
            navigationController.view.backgroundColor = UIColor.clear
        }

        if let tabBarController = viewController.tabBarController {
            tabBarController.view.backgroundColor = UIColor.clear
        }
    }

    /// Updates layout margins for the view controller
    public static func updateLayoutMargins(_ viewController: UIViewController) {
        let safeAreaInsets = viewController.view.safeAreaInsets
        viewController.additionalSafeAreaInsets = UIEdgeInsets(
            top: -safeAreaInsets.top,
            left: -safeAreaInsets.left,
            bottom: -safeAreaInsets.bottom,
            right: -safeAreaInsets.right
        )
    }

    /// Calculates the appropriate top anchor for content positioning
    public static func calculateTopAnchor(for viewController: UIViewController) -> (NSLayoutYAxisAnchor, CGFloat) {
        return (viewController.view.safeAreaLayoutGuide.topAnchor, 0)
    }

    /// Calculates the appropriate bottom anchor for content positioning
    public static func calculateBottomAnchor(for viewController: UIViewController, isTransparent: Bool) -> NSLayoutYAxisAnchor {
        return isTransparent ? viewController.view.bottomAnchor : viewController.view.safeAreaLayoutGuide.bottomAnchor
    }
}

/// SwiftUI view modifiers for iOS-specific styling
public extension View {
    /// Applies edge-to-edge display configuration
    func edgeToEdgeDisplay() -> some View {
        self
            .ignoresSafeArea(.all)
            .clipped()
    }

    /// Applies transparent background styling
    func transparentBackground() -> some View {
        self
            .background(Color.clear)
    }

    /// Configures layout margins for iOS
    func iOSLayoutMargins() -> some View {
        self
            .padding(.horizontal, 0)
            .padding(.vertical, 0)
    }
}

#endif