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
    public static func configureTabBarLayout(_ tabBar: macOSTabBar, position: Int32, containerFrame: CGRect) {
        let tabBarHeight = getTabBarHeight()
        var tabBarFrame: NSRect

        switch position {
        case 0: // bottom
            tabBarFrame = NSRect(x: 0, y: 0, width: containerFrame.width, height: tabBarHeight)
        case 1: // top
            tabBarFrame = NSRect(x: 0, y: containerFrame.height - tabBarHeight, width: containerFrame.width, height: tabBarHeight)
        case 2: // left
            tabBarFrame = NSRect(x: 0, y: 0, width: tabBarHeight, height: containerFrame.height)
        case 3: // right
            tabBarFrame = NSRect(x: containerFrame.width - tabBarHeight, y: 0, width: tabBarHeight, height: containerFrame.height)
        default:
            tabBarFrame = NSRect(x: 0, y: 0, width: containerFrame.width, height: tabBarHeight)
        }

        tabBar.frame = tabBarFrame
    }

    /// Gets the appropriate content area frame considering tab bar position
    public static func getContentAreaFrame(containerFrame: CGRect, tabBarPosition: Int32, hasTabBar: Bool) -> CGRect {
        guard hasTabBar else { return containerFrame }

        let tabBarHeight = getTabBarHeight()

        switch tabBarPosition {
        case 0: // bottom
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case 1: // top
            return CGRect(x: 0, y: 0, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case 2: // left
            return CGRect(x: tabBarHeight, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        case 3: // right
            return CGRect(x: 0, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        default:
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        }
    }
}

#endif
