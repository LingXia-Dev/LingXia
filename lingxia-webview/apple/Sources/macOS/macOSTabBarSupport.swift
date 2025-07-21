#if os(macOS)
import Cocoa
import Foundation

/// macOS-specific TabBar support utilities
@MainActor
public class macOSTabBarSupport {

    /// Creates a TabBar for macOS
    public static func createTabBar(frame: CGRect) -> macOSTabBar {
        return macOSTabBar(frame: NSRect(x: frame.origin.x, y: frame.origin.y, width: frame.width, height: frame.height))
    }

    /// Gets the tab bar height for macOS
    public static func getTabBarHeight() -> CGFloat {
        return TabBarConstants.TAB_HEIGHT
    }

    /// Configures tab bar positioning for macOS layout
    public static func configureTabBarLayout(_ tabBar: macOSTabBar, position: TabBarConfig.Position, containerFrame: CGRect) {
        let tabBarHeight = getTabBarHeight()
        var tabBarFrame: NSRect

        switch position {
        case .bottom:
            tabBarFrame = NSRect(x: 0, y: 0, width: containerFrame.width, height: tabBarHeight)
        case .top:
            tabBarFrame = NSRect(x: 0, y: containerFrame.height - tabBarHeight, width: containerFrame.width, height: tabBarHeight)
        case .left:
            tabBarFrame = NSRect(x: 0, y: 0, width: tabBarHeight, height: containerFrame.height)
        case .right:
            tabBarFrame = NSRect(x: containerFrame.width - tabBarHeight, y: 0, width: tabBarHeight, height: containerFrame.height)
        }

        tabBar.frame = tabBarFrame
    }

    /// Gets the appropriate content area frame considering tab bar position
    public static func getContentAreaFrame(containerFrame: CGRect, tabBarPosition: TabBarConfig.Position, hasTabBar: Bool) -> CGRect {
        guard hasTabBar else { return containerFrame }

        let tabBarHeight = getTabBarHeight()

        switch tabBarPosition {
        case .bottom:
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case .top:
            return CGRect(x: 0, y: 0, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case .left:
            return CGRect(x: tabBarHeight, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        case .right:
            return CGRect(x: 0, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        }
    }
}

#endif
