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

    /// Parse color string to platform color
    public static func parseColor(_ colorString: String) -> PlatformColor? {
        return TabBarHelper.parseColor(colorString)
    }

    /// Check if color is transparent
    public static func isTransparent(_ colorString: String) -> Bool {
        return TabBarHelper.isTransparent(colorString)
    }

    /// Default color constants
    public static let DEFAULT_SELECTED_COLOR = TabBarHelper.DEFAULT_SELECTED_COLOR
    public static let DEFAULT_UNSELECTED_COLOR = TabBarHelper.DEFAULT_UNSELECTED_COLOR
    public static let DEFAULT_BACKGROUND_COLOR = TabBarHelper.DEFAULT_BACKGROUND_COLOR

    /// Get resolved background color for this configuration
    public func resolvedBackgroundColor(isVertical: Bool) -> PlatformColor {
        return TabBarHelper.resolvedBackgroundColor(background_color.toString(), isVertical: isVertical)
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
    public static let TAB_HEIGHT: CGFloat = 64
    public static let ICON_TOP_MARGIN: CGFloat = 4
    public static let LABEL_BOTTOM_MARGIN: CGFloat = 4
}

/// Extension to add helper methods to swift-bridge generated TabBarItem
extension TabBarItem {
    /// Check if item is visible (always true for now)
    public var visible: Bool { true }
}

/// Helper methods for TabBar styling and color management
public struct TabBarHelper {
    // MARK: - Default Colors
    public static let DEFAULT_SELECTED_COLOR = "#1677FF"
    public static let DEFAULT_UNSELECTED_COLOR = "#666666"
    public static let DEFAULT_BACKGROUND_COLOR = "#ffffff"
    public static let DEFAULT_BORDER_COLOR = "#F0F0F0"

    /// Parse color string to platform color
    public static func parseColor(_ colorString: String) -> PlatformColor? {
        // Handle special "transparent" case
        if colorString.lowercased() == "transparent" {
            return PlatformColor.clear
        }
        return PlatformColor(hexString: colorString)
    }

    /// Check if color is transparent
    public static func isTransparent(_ colorString: String) -> Bool {
        // Handle special "transparent" string case
        if colorString.lowercased() == "transparent" {
            return true
        }

        guard let color = parseColor(colorString) else { return false }

        #if os(macOS)
        return color.alphaComponent < 1.0
        #else
        var alpha: CGFloat = 0
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        return alpha < 1.0
        #endif
    }

    /// Get resolved background color for TabBar
    public static func resolvedBackgroundColor(_ colorString: String, isVertical: Bool) -> PlatformColor {
        if let color = parseColor(colorString) {
            return color
        }

        // Default colors based on orientation
        if isVertical {
            #if os(macOS)
            return PlatformColor(hexString: "#F8F8F8") ?? PlatformColor.controlBackgroundColor
            #else
            return PlatformColor(hexString: "#F8F8F8") ?? PlatformColor.systemGray6
            #endif
        } else {
            return PlatformColor(hexString: DEFAULT_BACKGROUND_COLOR) ?? PlatformColor.white
        }
    }
}

extension PlatformColor {
    /// Initialize color from hex string
    convenience init?(hexString: String) {
        let hex = hexString.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let a, r, g, b: UInt64
        switch hex.count {
        case 3: // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6: // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8: // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            return nil
        }

        #if os(iOS)
        self.init(
            red: CGFloat(r) / 255,
            green: CGFloat(g) / 255,
            blue: CGFloat(b) / 255,
            alpha: CGFloat(a) / 255
        )
        #else
        self.init(
            red: Double(r) / 255,
            green: Double(g) / 255,
            blue: Double(b) / 255,
            alpha: Double(a) / 255
        )
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

    private var config: TabBarConfig?
    private var items: [TabBarItem] = []
    private var selectedPosition = -1
    private var onTabSelectedListener: ((Int, String) -> Void)?
    private var appId: String = ""

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
        return TabBarHelper.resolvedBackgroundColor(config.background_color.toString(), isVertical: isVertical())
    }

    /// Check if background should be transparent
    public func shouldUseTransparentBackground() -> Bool {
        guard let config = config else { return true }
        return TabBarHelper.isTransparent(config.background_color.toString())
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
        guard let config = config else { return TabBarConstants.TAB_HEIGHT }

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
