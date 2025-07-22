import Foundation

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

/// Represents a single tab bar item with its configuration
public struct TabBarItem {
    let pagePath: String
    let text: String?
    let iconPath: String
    let selectedIconPath: String
    let selected: Bool
    let visible: Bool

    public init(
        pagePath: String,
        text: String? = nil,
        iconPath: String,
        selectedIconPath: String,
        selected: Bool = false,
        visible: Bool = true
    ) {
        self.pagePath = pagePath
        self.text = text
        self.iconPath = iconPath
        self.selectedIconPath = selectedIconPath
        self.selected = selected
        self.visible = visible
    }
}

/// Configuration structure for the TabBar component
public struct TabBarConfig {
    let backgroundColor: String?
    let selectedColor: String?
    let color: String?
    let borderStyle: String?
    let height: CGFloat?
    let position: Position
    let list: [TabBarItem]
    let visible: Bool

    public enum Position {
        case top, bottom, left, right
    }

    static let DEFAULT_SELECTED_COLOR = "#1677FF"
    static let DEFAULT_UNSELECTED_COLOR = "#666666"
    static let DEFAULT_BORDER_COLOR = "#F0F0F0"
    static let DEFAULT_BACKGROUND_COLOR = "#FFFFFF"

    public init(
        backgroundColor: String? = nil,
        selectedColor: String? = nil,
        color: String? = nil,
        borderStyle: String? = nil,
        height: CGFloat? = nil,
        position: Position = .bottom,
        list: [TabBarItem] = [],
        visible: Bool = true
    ) {
        self.backgroundColor = backgroundColor
        self.selectedColor = selectedColor
        self.color = color
        self.borderStyle = borderStyle
        self.height = height
        self.position = position
        self.list = list
        self.visible = visible
    }

    public static func fromJson(_ json: String?) -> TabBarConfig? {
        guard let json = json, !json.isEmpty,
              let data = json.data(using: .utf8),
              let jsonObject = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }

        let list: [TabBarItem] = (jsonObject["list"] as? [[String: Any]])?.compactMap { item in
            let finalText = item["text"] as? String
            return TabBarItem(
                pagePath: item["pagePath"] as? String ?? "",
                text: finalText?.isEmpty == false ? finalText : nil,
                iconPath: item["iconPath"] as? String ?? "",
                selectedIconPath: item["selectedIconPath"] as? String ?? "",
                selected: item["selected"] as? Bool ?? false,
                visible: item["visible"] as? Bool ?? true
            )
        } ?? []

        let positionString = jsonObject["position"] as? String ?? "bottom"
        let position: Position
        switch positionString.lowercased() {
        case "top": position = .top
        case "left": position = .left
        case "right": position = .right
        default: position = .bottom
        }

        return TabBarConfig(
            backgroundColor: jsonObject["backgroundColor"] as? String,
            selectedColor: jsonObject["selectedColor"] as? String,
            color: jsonObject["color"] as? String,
            borderStyle: jsonObject["borderStyle"] as? String,
            height: jsonObject["height"] as? CGFloat,
            position: position,
            list: list,
            visible: jsonObject["visible"] as? Bool ?? true
        )
    }

    /// Determines if a color string should be treated as transparent
    public static func isTransparent(_ colorString: String?) -> Bool {
        guard let colorString = colorString else { return true }
        return colorString.lowercased() == "transparent" || colorString.isEmpty
    }

    /// Parse color string to platform color
    public func parseColor(_ colorString: String?) -> PlatformColor? {
        guard let colorString = colorString, !colorString.isEmpty else { return nil }
        if colorString.lowercased() == "transparent" {
            return PlatformColor.clear
        }
        return PlatformColor(hexString: colorString)
    }

    /// Get resolved background color for the tab bar
    public func resolvedBackgroundColor(isVertical: Bool) -> PlatformColor {
        if Self.isTransparent(backgroundColor) {
            return PlatformColor.clear
        }

        if let bgColor = parseColor(backgroundColor) {
            return bgColor
        }

        #if os(iOS)
        let defaultColor = PlatformColor(hexString: Self.DEFAULT_BACKGROUND_COLOR) ?? PlatformColor.systemBackground
        return isVertical ? UIColor(red: 0.97, green: 0.97, blue: 0.97, alpha: 1.0) : defaultColor
        #else
        let defaultColor = PlatformColor(hexString: Self.DEFAULT_BACKGROUND_COLOR) ?? PlatformColor.white
        return isVertical ? NSColor(red: 0.97, green: 0.97, blue: 0.97, alpha: 1.0) : defaultColor
        #endif
    }
}

/// Protocol for tab bar implementations
@MainActor
public protocol TabBarProtocol: AnyObject {
    var config: TabBarConfig { get }
    func setConfig(config: TabBarConfig)
    func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void)
    func findTabIndexByPath(_ path: String) -> Int
    func syncSelectedTabWithCurrentPath(_ currentPath: String)
    func setSelectedIndex(_ index: Int, notifyListener: Bool)
}

/// Shared TabBar business logic controller
/// Handles all platform-independent TabBar operations
@MainActor
public class TabBarController {

    // MARK: - Properties
    private var config: TabBarConfig = TabBarConfig()
    private var items = [TabBarItem]()
    private var selectedPosition = -1
    private var onTabSelectedListener: ((Int, String) -> Void)?

    // MARK: - Public Interface

    /// Set TabBar configuration and return filtered visible items
    public func setConfig(_ config: TabBarConfig) -> [TabBarItem] {
        self.config = config
        items = config.list.filter { $0.visible }
        return items
    }

    /// Get current configuration
    public func getConfig() -> TabBarConfig {
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
        return items.firstIndex { $0.pagePath == path } ?? -1
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
            onTabSelectedListener?(index, items[index].pagePath)
        }
    }

    /// Get current selected position
    public func getSelectedPosition() -> Int {
        return selectedPosition
    }

    /// Check if TabBar should be visible
    public func shouldBeVisible() -> Bool {
        return config.visible && !items.isEmpty
    }

    /// Check if TabBar is vertical (left/right position)
    public func isVertical() -> Bool {
        return config.position == .left || config.position == .right
    }

    /// Get resolved background color for current configuration
    public func getResolvedBackgroundColor() -> PlatformColor {
        return config.resolvedBackgroundColor(isVertical: isVertical())
    }

    /// Check if background should be transparent
    public func shouldUseTransparentBackground() -> Bool {
        return TabBarConfig.isTransparent(config.backgroundColor)
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
        if let customHeight = config.height {
            return customHeight
        }

        switch config.position {
        case .top, .bottom:
            return TabBarConstants.TAB_HEIGHT
        case .left, .right:
            return TabBarConstants.TAB_HEIGHT // Can be adjusted for vertical tabs
        }
    }

    /// Handle tab selection
    public func handleTabSelection(at index: Int) {
        setSelectedIndex(index, notifyListener: true)
    }

    /// Reset controller state
    public func reset() {
        config = TabBarConfig()
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

    public var config: TabBarConfig {
        return controller.getConfig()
    }

    public func setConfig(config: TabBarConfig) {
        let items = controller.setConfig(config)
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
