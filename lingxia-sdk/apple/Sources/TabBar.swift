import Foundation

/// Extension to add helper methods to swift-bridge generated TabBarConfig
extension TabBarConfig {
    /// Get position as enum
    public var positionEnum: TabBarPosition {
        switch position {
        case 1: return .top
        case 2: return .left
        case 3: return .right
        default: return .bottom // 0 or any other value
        }
    }

    /// Get all tab items for this config
    public func getItems(appId: String) -> [TabBarItem] {
        var items: [TabBarItem] = []
        for i in 0..<items_count {
            if let item = getTabBarItem(appId, i) {
                items.append(item)
            }
        }
        return items
    }

    public func getGroupedItems(appId: String) -> (start: [TabBarItem], center: [TabBarItem], end: [TabBarItem]) {
        let allItems = getItems(appId: appId)
        var startItems: [TabBarItem] = []
        var centerItems: [TabBarItem] = []
        var endItems: [TabBarItem] = []

        for item in allItems {
            switch item.group {
            case 1:
                startItems.append(item)
            case 2:
                endItems.append(item)
            default:
                centerItems.append(item)
            }
        }

        return (startItems, centerItems, endItems)
    }

    /// Check if color is transparent
    public static func isTransparent(_ colorValue: UInt32) -> Bool {
        return (colorValue >> 24) & 0xFF == 0
    }

    /// Get resolved background color for this configuration
    public func resolvedBackgroundColor(isVertical: Bool) -> PlatformColor {
        return TabBarHelper.resolvedBackgroundColor(background_color, isVertical: isVertical)
    }
}

/// Position enum for TabBar
public enum TabBarPosition {
    case top, bottom, left, right
}

/// Constants used across TabBar implementations
public struct TabBarConstants {
    public static let ICON_SIZE: CGFloat = 24
    public static let ITEM_FONT_SIZE: CGFloat = 12
    public static let ITEM_SPACING: CGFloat = 8
    public static let BORDER_WIDTH: CGFloat = 1
    public static let ICON_TOP_MARGIN: CGFloat = 4
    public static let LABEL_BOTTOM_MARGIN: CGFloat = 4

    // Group layout spacing constants (for TabBar group feature)
    public static let DEFAULT_SPACING: CGFloat = 12  // Spacing between items in start/end groups
    public static let CENTER_SPACING: CGFloat = 8    // Spacing between items in center group
    public static let MINIMAL_SPACER_SIZE: CGFloat = 4  // Minimum size for flexible spacers
}

/// Extension to add helper methods to swift-bridge generated TabBarItem
extension TabBarItem {
    /// Check if item is visible (always true for now)
    public var visible: Bool { true }
}

/// Helper methods for TabBar styling and color management
public struct TabBarHelper {
    /// Get resolved background color for TabBar
    public static func resolvedBackgroundColor(_ colorValue: UInt32, isVertical: Bool) -> PlatformColor {
        // If the color is transparent (alpha is 0), use a default based on orientation
        if (colorValue >> 24) & 0xFF == 0 {
            if isVertical {
                #if os(macOS)
                return PlatformColor(argb: 0xFFF8F8F8) // Default light gray for vertical
                #else
                return PlatformColor(argb: 0xFFF8F8F8) // Default light gray for vertical
                #endif
            } else {
                return PlatformColor(argb: 0xFFFFFFFF) // Default white for horizontal
            }
        }
        return PlatformColor(argb: colorValue)
    }
}

extension PlatformColor {
    /// Initialize color from a UInt32 ARGB value
    convenience init(argb: UInt32) {
        let alpha = CGFloat((argb >> 24) & 0xFF) / 255.0
        let red = CGFloat((argb >> 16) & 0xFF) / 255.0
        let green = CGFloat((argb >> 8) & 0xFF) / 255.0
        let blue = CGFloat(argb & 0xFF) / 255.0
        
        #if os(iOS)
        self.init(red: red, green: green, blue: blue, alpha: alpha)
        #else
        self.init(red: red, green: green, blue: blue, alpha: alpha)
        #endif
    }
}

/// Protocol for tab bar implementations
@MainActor
public protocol TabBarProtocol: AnyObject {
    var config: TabBarConfig? { get }
    func setConfig(config: TabBarConfig, appId: String)
    func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void)
    func findTabIndexByPath(_ path: String) -> Int
    func syncSelectedTabWithCurrentPath(_ currentPath: String)
    func setSelectedIndex(_ index: Int, notifyListener: Bool)
}

/// Shared TabBar business logic controller
/// Handles all platform-independent TabBar operations
@MainActor
public class TabBarController {

    var config: TabBarConfig?
    var items: [TabBarItem] = []
    var selectedPosition = -1
    var onTabSelectedListener: ((Int, String) -> Void)?
    var appId: String = ""

    /// Set TabBar configuration and return filtered visible items
    public func setConfig(_ config: TabBarConfig, appId: String) -> [TabBarItem] {
        self.config = config
        self.appId = appId
        items = config.getItems(appId: appId)
        return items
    }

    /// Get current configuration
    public func getConfig() -> TabBarConfig? {
        return config
    }

    /// Get current items
    public func getItems() -> [TabBarItem] {
        return items
    }

    /// Set tab selection listener
    public func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        self.onTabSelectedListener = listener
    }

    /// Find tab index by path
    public func findTabIndexByPath(_ path: String) -> Int {
        return items.firstIndex { $0.page_path.toString() == path } ?? -1
    }

    /// Sync selected tab with current path
    public func syncSelectedTabWithCurrentPath(_ currentPath: String) {
        let index = findTabIndexByPath(currentPath)
        if index >= 0 {
            setSelectedIndex(index, notifyListener: false)
        }
    }

    /// Set selected index
    public func setSelectedIndex(_ index: Int, notifyListener: Bool) {
        guard index >= 0 && index < items.count else { return }

        selectedPosition = index

        if notifyListener {
            onTabSelectedListener?(index, items[index].page_path.toString())
        }
    }

    /// Get current selected position
    public func getSelectedPosition() -> Int {
        return selectedPosition
    }

    /// Check if TabBar should be visible
    public func shouldBeVisible() -> Bool {
        return config != nil && !items.isEmpty
    }

    /// Check if TabBar is vertical (left/right position)
    public func isVertical() -> Bool {
        guard let config = config else { return false }
        return config.position == 2 || config.position == 3 // left=2, right=3
    }

    /// Get resolved background color for current configuration
    public func getResolvedBackgroundColor() -> PlatformColor {
        guard let config = config else { return PlatformColor.clear }
        return TabBarHelper.resolvedBackgroundColor(config.background_color, isVertical: isVertical())
    }

    /// Check if background should be transparent
    public func shouldUseTransparentBackground() -> Bool {
        guard let config = config else { return true }
        return TabBarConfig.isTransparent(config.background_color)
    }

    /// Get tab item at index
    public func getTabItem(at index: Int) -> TabBarItem? {
        guard index >= 0 && index < items.count else { return nil }
        return items[index]
    }

    /// Check if tab at index is selected
    public func isTabSelected(at index: Int) -> Bool {
        return index == selectedPosition
    }

    /// Get effective height for TabBar
    public func getEffectiveHeight() -> CGFloat {
        guard let config = config else { return 64 } // Default fallback

        // Use the configured dimension directly
        return CGFloat(config.dimension)
    }

    /// Handle tab selection
    public func handleTabSelection(at index: Int) {
        setSelectedIndex(index, notifyListener: true)
    }

    /// Reset controller state
    public func reset() {
        config = nil
        items.removeAll()
        selectedPosition = -1
        onTabSelectedListener = nil
    }
}

/// Protocol for TabBar UI implementations to conform to
@MainActor
public protocol TabBarUIDelegate: AnyObject {
    /// Update UI when tab selection changes
    func updateTabSelection(selectedIndex: Int)

    /// Update UI when configuration changes
    func updateConfiguration()

    /// Update UI when items change
    func updateItems(_ items: [TabBarItem])
}

/// Enhanced TabBar protocol that uses the shared controller
@MainActor
public protocol EnhancedTabBarProtocol: TabBarProtocol {
    var controller: TabBarController { get }
    var uiDelegate: TabBarUIDelegate? { get set }
}

/// Default implementation for enhanced TabBar protocol
extension EnhancedTabBarProtocol {

    public var config: TabBarConfig? {
        return controller.getConfig()
    }

    public func setConfig(config: TabBarConfig, appId: String) {
        let items = controller.setConfig(config, appId: appId)
        uiDelegate?.updateConfiguration()
        uiDelegate?.updateItems(items)
    }

    public func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        controller.setOnTabSelectedListener(listener)
    }

    public func findTabIndexByPath(_ path: String) -> Int {
        return controller.findTabIndexByPath(path)
    }

    public func syncSelectedTabWithCurrentPath(_ currentPath: String) {
        controller.syncSelectedTabWithCurrentPath(currentPath)
        uiDelegate?.updateTabSelection(selectedIndex: controller.getSelectedPosition())
    }

    public func setSelectedIndex(_ index: Int, notifyListener: Bool) {
        controller.setSelectedIndex(index, notifyListener: notifyListener)
        uiDelegate?.updateTabSelection(selectedIndex: controller.getSelectedPosition())
    }
}

#if os(iOS)
import UIKit
public typealias LingXiaTabBar = iOSLingXiaTabBar
public typealias PlatformTabBar = iOSLingXiaTabBar
#elseif os(macOS)
import Cocoa
public typealias LingXiaTabBar = macOSTabBar
public typealias PlatformTabBar = macOSTabBar
#endif
