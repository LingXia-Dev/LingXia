#if os(iOS)
import UIKit
import Foundation

/// iOS-specific TabBar support utilities
@MainActor
public class iOSTabBarSupport {

    /// Creates a TabBar for iOS
    public static func createTabBar(frame: CGRect) -> LingXiaTabBar {
        return LingXiaTabBar(frame: frame)
    }

    /// Gets the tab bar height for iOS
    public static func getTabBarHeight() -> CGFloat {
        return TabBarConstants.TAB_HEIGHT
    }

    /// Configures tab bar transparency mode
    public static func configureTabBarTransparencyMode(_ tabBar: LingXiaTabBar, isTransparent: Bool) {
        if isTransparent {
            tabBar.backgroundColor = UIColor.clear
            tabBar.layer.backgroundColor = UIColor.clear.cgColor
        } else {
            // Use the configured background color or default
            let config = tabBar.config
            if let bgColor = config.parseColor(config.backgroundColor) {
                tabBar.backgroundColor = bgColor
                tabBar.layer.backgroundColor = bgColor.cgColor
            } else {
                tabBar.backgroundColor = UIColor.systemBackground
                tabBar.layer.backgroundColor = UIColor.systemBackground.cgColor
            }
        }
    }

    /// Applies tab bar layout parameters
    public static func applyTabBarLayoutParams(tabBar: LingXiaTabBar, config: TabBarConfig) {
        let position = config.position
        let isVertical = position == .left || position == .right

        // Configure orientation
        if isVertical {
            tabBar.transform = CGAffineTransform(rotationAngle: .pi / 2)
        } else {
            tabBar.transform = .identity
        }

        // Apply height if specified
        if let height = config.height {
            tabBar.frame.size.height = height
        }

        // Configure background - CRITICAL: Don't override transparent backgrounds!
        if TabBarConfig.isTransparent(config.backgroundColor) {
            // For transparent backgrounds, force transparency mode instead of using resolved color
            tabBar.forceTransparencyMode()
        } else {
            // For non-transparent backgrounds, use resolved color
            let resolvedColor = config.resolvedBackgroundColor(isVertical: isVertical)
            tabBar.backgroundColor = resolvedColor
            tabBar.layer.backgroundColor = resolvedColor.cgColor
        }
    }

    /// Gets the appropriate content area frame considering tab bar position
    public static func getContentAreaFrame(containerFrame: CGRect, tabBarPosition: TabBarConfig.Position, tabBarHeight: CGFloat, hasTabBar: Bool) -> CGRect {
        guard hasTabBar else { return containerFrame }

        switch tabBarPosition {
        case .bottom:
            return CGRect(x: 0, y: 0, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case .top:
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case .left:
            return CGRect(x: tabBarHeight, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        case .right:
            return CGRect(x: 0, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        }
    }

    /// Calculates the appropriate anchor points for tab bar positioning
    public static func calculateTabBarAnchors(for position: TabBarConfig.Position, in containerView: UIView, safeArea: UILayoutGuide) -> (top: NSLayoutYAxisAnchor, bottom: NSLayoutYAxisAnchor, leading: NSLayoutXAxisAnchor, trailing: NSLayoutXAxisAnchor) {
        switch position {
        case .bottom:
            return (
                top: containerView.bottomAnchor,
                bottom: safeArea.bottomAnchor,
                leading: safeArea.leadingAnchor,
                trailing: safeArea.trailingAnchor
            )
        case .top:
            return (
                top: safeArea.topAnchor,
                bottom: containerView.topAnchor,
                leading: safeArea.leadingAnchor,
                trailing: safeArea.trailingAnchor
            )
        case .left:
            return (
                top: safeArea.topAnchor,
                bottom: safeArea.bottomAnchor,
                leading: safeArea.leadingAnchor,
                trailing: containerView.leadingAnchor
            )
        case .right:
            return (
                top: safeArea.topAnchor,
                bottom: safeArea.bottomAnchor,
                leading: containerView.trailingAnchor,
                trailing: safeArea.trailingAnchor
            )
        }
    }
}

#endif
