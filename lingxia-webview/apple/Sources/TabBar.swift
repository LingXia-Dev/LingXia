import Foundation
import os.log

#if os(iOS)
import UIKit
#elseif os(macOS)
import Cocoa
#endif

/// Constants used across TabBar implementations
public struct TabBarConstants {
    /// Icon size for TabBar items
    public static let ICON_SIZE: CGFloat = 24

    /// Font size for TabBar item text
    public static let ITEM_FONT_SIZE: CGFloat = 12

    /// Spacing between TabBar items
    public static let ITEM_SPACING: CGFloat = 8

    /// Border width for TabBar
    public static let BORDER_WIDTH: CGFloat = 1

    /// Default TabBar height
    public static let TAB_HEIGHT: CGFloat = 64

    /// Top margin for icons
    public static let ICON_TOP_MARGIN: CGFloat = 4

    /// Bottom margin for labels
    public static let LABEL_BOTTOM_MARGIN: CGFloat = 4
}

/// Represents a single tab bar item with its configuration
public struct TabBarItem {
    /// The page path to navigate to when this tab is selected
    let pagePath: String
    /// Optional text label to display below the icon
    let text: String?
    /// Path to the icon image file for the unselected state
    let iconPath: String
    /// Path to the icon image file for the selected state
    let selectedIconPath: String
    /// Whether this tab is currently selected
    let selected: Bool
    /// Whether this tab should be visible in the tab bar
    let visible: Bool

    /// Initializes a new TabBarItem
    /// - Parameters:
    ///   - pagePath: The page path to navigate to when selected
    ///   - text: Optional text label (default: nil)
    ///   - iconPath: Path to the unselected state icon
    ///   - selectedIconPath: Path to the selected state icon
    ///   - selected: Whether this tab is selected (default: false)
    ///   - visible: Whether this tab is visible (default: true)
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
/// Defines the appearance, position, and behavior of the tab bar
public struct TabBarConfig {
    /// Background color of the tab bar as hex string (e.g., "#FFFFFF"). If nil, uses default color
    let backgroundColor: String?
    /// Color for selected tab items as hex string (e.g., "#1677FF")
    let selectedColor: String?
    /// Color for unselected tab items as hex string (e.g., "#666666")
    let color: String?
    /// Border style as hex string (e.g., "#F0F0F0"). If nil, no border
    let borderStyle: String?
    /// Height of the tab bar in points. If nil, uses default height
    let height: CGFloat?
    /// Position of the tab bar relative to the content
    let position: Position
    /// Array of tab bar items to display
    let list: [TabBarItem]
    /// Whether the tab bar should be visible
    let visible: Bool

    /// Enumeration defining possible positions for the tab bar
    public enum Position {
        /// Tab bar positioned at the top of the screen
        case top
        /// Tab bar positioned at the bottom of the screen
        case bottom
        /// Tab bar positioned on the left side of the screen
        case left
        /// Tab bar positioned on the right side of the screen
        case right
    }

    // MARK: - Default Color Strings
    /// Default color for selected tab items (#1677FF - Modern blue)
    static let DEFAULT_SELECTED_COLOR = "#1677FF"
    /// Default color for unselected tab items (#666666 - Dark gray)
    static let DEFAULT_UNSELECTED_COLOR = "#666666"
    /// Default border color (#F0F0F0 - Light gray)
    static let DEFAULT_BORDER_COLOR = "#F0F0F0"
    /// Default background color (White)
    static let DEFAULT_BACKGROUND_COLOR = "#FFFFFF"
    /// Transparency threshold for determining if a color should be treated as transparent
    static let TRANSPARENCY_THRESHOLD: CGFloat = 0.99

    // Platform-specific color parsing and transparency checking should be implemented
    // in platform-specific extensions (iOS/macOS)

    /// Initializes a new TabBarConfig with the specified parameters
    /// - Parameters:
    ///   - backgroundColor: Background color as hex string (e.g., "#FFFFFF")
    ///   - selectedColor: Color for selected tab items as hex string
    ///   - color: Color for unselected tab items as hex string
    ///   - borderStyle: Border color as hex string
    ///   - height: Height of the tab bar in points
    ///   - position: Position of the tab bar (default: .bottom)
    ///   - list: Array of tab bar items (default: empty)
    ///   - visible: Whether the tab bar should be visible (default: true)
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

    /// Creates a TabBarConfig from a JSON string
    /// - Parameter json: JSON string containing tab bar configuration
    /// - Returns: TabBarConfig instance if parsing succeeds, nil otherwise
    public static func fromJson(_ json: String?) -> TabBarConfig? {
        guard let json = json, !json.isEmpty else {
            return nil
        }

        do {
            guard let data = json.data(using: .utf8),
                  let jsonObject = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                os_log("TabBarConfig.fromJson: failed to parse JSON", log: OSLog(subsystem: "LingXia", category: "TabBar"), type: .error)
                return nil
            }

            let backgroundColorString = jsonObject["backgroundColor"] as? String
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

            let config = TabBarConfig(
                backgroundColor: backgroundColorString,
                selectedColor: jsonObject["selectedColor"] as? String,
                color: jsonObject["color"] as? String,
                borderStyle: jsonObject["borderStyle"] as? String,
                height: jsonObject["height"] as? CGFloat,
                position: position,
                list: list,
                visible: jsonObject["visible"] as? Bool ?? true
            )

            return config
        } catch {
            os_log("TabBarConfig.fromJson: JSON parsing error: %@", log: OSLog(subsystem: "LingXia", category: "TabBar"), type: .error, error.localizedDescription)
            return nil
        }
    }

    private static func parseColor(_ colorString: String?) -> PlatformColor? {
        guard let colorString = colorString, !colorString.isEmpty else {
            return nil
        }

        if colorString.lowercased() == "transparent" {
            return PlatformColor.clear
        }

        if colorString.hasPrefix("rgba(") {
            return parseRgbaColor(colorString)
        }

        return PlatformColor(hexString: colorString)
    }

    private static func parseRgbaColor(_ rgba: String) -> PlatformColor? {
        let values = rgba.replacingOccurrences(of: "rgba(", with: "")
            .replacingOccurrences(of: ")", with: "")
            .components(separatedBy: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }

        guard values.count == 4,
              let r = Int(values[0]),
              let g = Int(values[1]),
              let b = Int(values[2]),
              let a = Float(values[3]) else {
            return nil
        }

        return PlatformColor(
            red: CGFloat(max(0, min(255, r))) / 255.0,
            green: CGFloat(max(0, min(255, g))) / 255.0,
            blue: CGFloat(max(0, min(255, b))) / 255.0,
            alpha: CGFloat(max(0, min(1, a)))
        )
    }
}

#if os(iOS)
/// TabBar component for mini apps with comprehensive customization support
///
/// Features:
/// - Customizable tab items with icons and text
/// - Four positioning options: top, bottom, left, right
/// - Dynamic styling and content updates
/// - Transparent background support
/// - Auto Layout integration
/// - Gesture-based tab selection
///
/// Usage:
/// ```swift
/// let tabBar = LingXiaTabBar()
/// tabBar.setConfig(config: tabBarConfig)
/// tabBar.setOnTabSelectedListener { index, path in
///     // Handle tab selection
/// }
/// ```
public class LingXiaTabBar: UIView {
    private static let log = OSLog(subsystem: "LingXia", category: "TabBar")
    private static let DEFAULT_TAB_BAR_SIZE: CGFloat = 64
    private static let VERTICAL_TAB_BAR_WIDTH_MULTIPLIER: CGFloat = 1.0

    // Constants for vertical TabBar item styling
    private static let VERTICAL_ITEM_MAX_HEIGHT: CGFloat = 70
    private static let VERTICAL_ITEM_MIN_HEIGHT: CGFloat = 48
    private static let VERTICAL_ITEM_PADDING_HORIZONTAL: CGFloat = 6
    private static let VERTICAL_ITEM_PADDING_VERTICAL: CGFloat = 8
    private static let VERTICAL_ITEM_ICON_SIZE: CGFloat = 22
    private static let HORIZONTAL_ITEM_ICON_SIZE: CGFloat = 24
    private static let VERTICAL_ITEM_TEXT_SIZE: CGFloat = 12
    private static let HORIZONTAL_ITEM_TEXT_SIZE: CGFloat = 12

    // Padding for individual TabBarItems when TabBar is horizontal
    private static let HORIZONTAL_ITEM_PADDING_SIDES: CGFloat = 4
    private static let HORIZONTAL_ITEM_PADDING_VERTICAL: CGFloat = 2

    private static let VERTICAL_BORDER_COLOR = UIColor(red: 0.88, green: 0.88, blue: 0.88, alpha: 1.0) // #E0E0E0
    private static let VERTICAL_TABBAR_BACKGROUND_COLOR = UIColor(red: 0.97, green: 0.97, blue: 0.97, alpha: 1.0) // #F8F8F8
    private static let VERTICAL_SELECTED_ITEM_BACKGROUND_COLOR = UIColor(red: 0.9, green: 0.94, blue: 1.0, alpha: 1.0) // #E6F0FF
    private static let SELECTED_ITEM_CORNER_RADIUS: CGFloat = 12

    public var config = TabBarConfig()
    private var items = [TabBarItem]()
    private var tabViews = [UIView]()
    private var itemsContainer: UIStackView?
    private var selectedPosition = -1
    private var onTabSelectedListener: ((Int, String) -> Void)?
    private var onVisibilityChangedListener: ((Bool) -> Void)?

    public override init(frame: CGRect) {
        super.init(frame: frame)
        setupUI()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupUI() {
        isHidden = true

        // Set default white background to avoid black appearance during initialization
        backgroundColor = UIColor(hexString: TabBarConfig.DEFAULT_BACKGROUND_COLOR) ?? UIColor.white

        itemsContainer = UIStackView()
        updateItemsContainerLayout(config: self.config)
        performLayoutForPosition()
    }

    private func updateItemsContainerLayout(config: TabBarConfig) {
        guard let itemsContainer = itemsContainer else { return }

        let isVerticalTabBar = config.position == .left || config.position == .right

        itemsContainer.axis = isVerticalTabBar ? .vertical : .horizontal
        itemsContainer.distribution = .fillEqually
        itemsContainer.alignment = .fill

        // Only set background if this is not the default config (i.e., setConfig has been called)
        // Check if config has been properly initialized by looking for non-default values
        let isDefaultConfig = config.backgroundColor == TabBarConfig.DEFAULT_BACKGROUND_COLOR &&
                             config.list.isEmpty

        if !isDefaultConfig {
            // Set background for itemsContainer to match TabBar background
            if TabBarConfig.isTransparent(config.backgroundColor) {
                // CRITICAL: For transparent backgrounds, set both backgroundColor and layer.backgroundColor
                itemsContainer.backgroundColor = UIColor.clear
                itemsContainer.layer.backgroundColor = UIColor.clear.cgColor
                itemsContainer.isOpaque = false
                itemsContainer.layer.isOpaque = false
            } else {
                // Use platform extension to resolve background color
                let backgroundColor = config.resolvedBackgroundColor(isVertical: isVerticalTabBar)
                itemsContainer.backgroundColor = backgroundColor
                itemsContainer.layer.backgroundColor = backgroundColor.cgColor
            }
        }
    }

    private func performLayoutForPosition() {
        subviews.forEach { $0.removeFromSuperview() }

        let isBackgroundTransparent = TabBarConfig.isTransparent(config.backgroundColor)

        guard let itemsContainer = itemsContainer else { return }

        switch config.position {
        case .top:
            addSubview(itemsContainer)
            if !isBackgroundTransparent {
                addBorderView(position: .bottom)
            }
        case .bottom:
            if !isBackgroundTransparent {
                addBorderView(position: .top)
            }
            addSubview(itemsContainer)
        case .left:
            addSubview(itemsContainer)
            if !isBackgroundTransparent {
                addBorderView(position: .right)
            }
        case .right:
            if !isBackgroundTransparent {
                addBorderView(position: .left)
            }
            addSubview(itemsContainer)
        }

        setupContainerConstraints()
    }

    private func addBorderView(position: TabBarConfig.Position) {
        let borderView = UIView()
        let borderColorString = config.borderStyle ?? TabBarConfig.DEFAULT_BORDER_COLOR
        borderView.backgroundColor = config.parseColor(borderColorString) ?? UIColor.lightGray
        borderView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(borderView)

        switch position {
        case .top:
            NSLayoutConstraint.activate([
                borderView.topAnchor.constraint(equalTo: topAnchor),
                borderView.leadingAnchor.constraint(equalTo: leadingAnchor),
                borderView.trailingAnchor.constraint(equalTo: trailingAnchor),
                borderView.heightAnchor.constraint(equalToConstant: 1)
            ])
        case .bottom:
            NSLayoutConstraint.activate([
                borderView.bottomAnchor.constraint(equalTo: bottomAnchor),
                borderView.leadingAnchor.constraint(equalTo: leadingAnchor),
                borderView.trailingAnchor.constraint(equalTo: trailingAnchor),
                borderView.heightAnchor.constraint(equalToConstant: 1)
            ])
        case .left:
            NSLayoutConstraint.activate([
                borderView.leadingAnchor.constraint(equalTo: leadingAnchor),
                borderView.topAnchor.constraint(equalTo: topAnchor),
                borderView.bottomAnchor.constraint(equalTo: bottomAnchor),
                borderView.widthAnchor.constraint(equalToConstant: 1)
            ])
        case .right:
            NSLayoutConstraint.activate([
                borderView.trailingAnchor.constraint(equalTo: trailingAnchor),
                borderView.topAnchor.constraint(equalTo: topAnchor),
                borderView.bottomAnchor.constraint(equalTo: bottomAnchor),
                borderView.widthAnchor.constraint(equalToConstant: 1)
            ])
        }
    }

    private func setupContainerConstraints() {
        guard let itemsContainer = itemsContainer else { return }

        itemsContainer.translatesAutoresizingMaskIntoConstraints = false

        // For bottom position, handle safe area properly for transparent TabBars
        if config.position == .bottom {
            let isTransparent = TabBarConfig.isTransparent(config.backgroundColor)

            if isTransparent {
                // For transparent TabBars, fill the entire TabBar area (including safe area)
                os_log("TabBar.setupContainerConstraints: Transparent TabBar - no bottom padding",
                       log: LingXiaTabBar.log, type: .info)
                NSLayoutConstraint.activate([
                    itemsContainer.topAnchor.constraint(equalTo: topAnchor),
                    itemsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
                    itemsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
                    itemsContainer.bottomAnchor.constraint(equalTo: bottomAnchor)
                ])
            } else {
                // For non-transparent TabBars, add bottom padding for safe area
                os_log("TabBar.setupContainerConstraints: Opaque TabBar - adding bottom padding",
                       log: LingXiaTabBar.log, type: .info)
                let bottomPadding = UIView()
                bottomPadding.backgroundColor = itemsContainer.backgroundColor
                bottomPadding.translatesAutoresizingMaskIntoConstraints = false
                addSubview(bottomPadding)
                os_log("TabBar.setupContainerConstraints: Bottom padding backgroundColor=%{public}@",
                       log: LingXiaTabBar.log, type: .info, bottomPadding.backgroundColor?.description ?? "nil")

                NSLayoutConstraint.activate([
                    // Items container constraints
                    itemsContainer.topAnchor.constraint(equalTo: topAnchor),
                    itemsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
                    itemsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
                    itemsContainer.heightAnchor.constraint(equalToConstant: LingXiaTabBar.DEFAULT_TAB_BAR_SIZE),

                    // Bottom padding constraints
                    bottomPadding.topAnchor.constraint(equalTo: itemsContainer.bottomAnchor),
                    bottomPadding.leadingAnchor.constraint(equalTo: leadingAnchor),
                    bottomPadding.trailingAnchor.constraint(equalTo: trailingAnchor),
                    bottomPadding.bottomAnchor.constraint(equalTo: bottomAnchor)
                ])
            }
        } else {
            NSLayoutConstraint.activate([
                itemsContainer.topAnchor.constraint(equalTo: topAnchor),
                itemsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
                itemsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
                itemsContainer.bottomAnchor.constraint(equalTo: bottomAnchor)
            ])
        }
    }

    /// Configures the tab bar with the provided configuration
    /// - Parameter config: TabBarConfig containing all styling and content settings
    /// - Note: This method will validate the config and show the tab bar if valid
    public func setConfig(config: TabBarConfig) {
        guard isValidConfig(config: config) else {
            os_log("TabBar.setConfig: Invalid TabBar config provided", log: LingXiaTabBar.log, type: .error)
            return
        }

        self.config = config
        let isBackgroundTransparent = TabBarConfig.isTransparent(config.backgroundColor)
        let isVertical = config.position == .left || config.position == .right
        let tabBarBackgroundColor = config.resolvedBackgroundColor(isVertical: isVertical)

        // Apply initial background settings
        if isBackgroundTransparent {
            forceTransparencyMode()
        } else {
            backgroundColor = tabBarBackgroundColor
            alpha = 1.0
        }

        // Configure visual effects
        layer.shadowOpacity = isBackgroundTransparent ? 0 : 0.1
        layer.shadowOffset = CGSize(width: 0, height: 2)
        layer.shadowRadius = 4
        layer.borderWidth = isBackgroundTransparent ? 0 : 0
        layer.masksToBounds = isBackgroundTransparent ? false : false

        updateItemsContainerLayout(config: self.config)

        // Configure itemsContainer for transparency
        if let itemsContainer = itemsContainer, isBackgroundTransparent {
            itemsContainer.backgroundColor = UIColor.clear
            itemsContainer.layer.backgroundColor = UIColor.clear.cgColor
            itemsContainer.isOpaque = false
            itemsContainer.layer.isOpaque = false
            setAllSubviewsTransparent(view: itemsContainer)
        }

        performLayoutForPosition()
        setItems(newItems: config.list)
        isHidden = false

        // Force layout and final background refresh
        setNeedsLayout()
        layoutIfNeeded()

        // Ensure transparency is maintained after layout (synchronously)
        let isTransparent = TabBarConfig.isTransparent(config.backgroundColor)
        if isTransparent {
            forceTransparencyMode()
        }
    }

    /// Sets the tab bar items to display
    /// - Parameter newItems: Array of TabBarItem objects. Only visible items will be shown
    /// - Note: This method filters out invisible items and rebuilds the tab bar UI
    public func setItems(newItems: [TabBarItem]) {
        items = newItems.filter { $0.visible }  // Only show items where visible is true

        guard let container = itemsContainer else { return }

        // Remove existing tab views
        container.arrangedSubviews.forEach { container.removeArrangedSubview($0); $0.removeFromSuperview() }
        tabViews.removeAll()

        if !items.isEmpty {
            // Reset selected position to avoid stale state
            selectedPosition = -1

            // Find selected item index (default to 0 if none specified)
            let initialSelectedIdx = items.firstIndex { $0.selected } ?? 0
            selectedPosition = initialSelectedIdx

            for (index, item) in items.enumerated() {
                let tabView = createTabView(item: item, config: config, isSelected: index == selectedPosition)
                tabView.tag = index  // Set the correct index as tag
                tabViews.append(tabView)
                container.addArrangedSubview(tabView)
            }
        }
    }

    private func isValidConfig(config: TabBarConfig) -> Bool {
        return !config.list.isEmpty
    }

    /// Sets the callback for tab selection events
    /// - Parameter listener: Closure called when a tab is selected, receives (index, pagePath)
    public func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        onTabSelectedListener = listener
    }

    /// Finds the index of a tab item by its page path
    /// - Parameter path: The page path to search for
    /// - Returns: The index of the matching tab, or -1 if not found
    public func findTabIndexByPath(_ path: String) -> Int {
        return items.firstIndex { $0.pagePath == path } ?? -1
    }

    /// Synchronizes the selected tab based on the current page path
    /// - Parameter currentPath: The current page path
    public func syncSelectedTabWithCurrentPath(_ currentPath: String) {

        let targetIndex = findTabIndexByPath(currentPath)

        if targetIndex >= 0 && targetIndex != selectedPosition {
            setSelectedIndex(targetIndex, notifyListener: false)
        } else if targetIndex < 0 {
            os_log("TabBar.syncSelectedTabWithCurrentPath: Path %{public}@ not found in tabs",
                   log: LingXiaTabBar.log, type: .error, currentPath)
        } else {
            os_log("TabBar.syncSelectedTabWithCurrentPath: Already at correct index %d for path %{public}@",
                   log: LingXiaTabBar.log, type: .debug, targetIndex, currentPath)
        }
    }

    /// Programmatically selects a tab at the specified index
    /// - Parameters:
    ///   - index: The index of the tab to select
    ///   - notifyListener: Whether to notify the selection listener (default: true)
    public func setSelectedIndex(_ index: Int, notifyListener: Bool = true) {
        guard index >= 0 && index < items.count && index < tabViews.count else {
            os_log("setSelectedIndex: Invalid index %d (items.count=%d, tabViews.count=%d)",
                   log: LingXiaTabBar.log, type: .error, index, items.count, tabViews.count)
            return
        }

        os_log("setSelectedIndex: Changing from %d to %d for path %{public}@",
               log: LingXiaTabBar.log, type: .info, selectedPosition, index, items[index].pagePath)

        if index != selectedPosition {
            let previousIndex = selectedPosition
            selectedPosition = index

            // Update UI state
            if previousIndex >= 0 && previousIndex < tabViews.count {
                os_log("setSelectedIndex: Deselecting previous tab at index %d",
                       log: LingXiaTabBar.log, type: .info, previousIndex)
                updateTabState(tabView: tabViews[previousIndex], item: items[previousIndex], selected: false)
            }

            os_log("setSelectedIndex: Selecting new tab at index %d",
                   log: LingXiaTabBar.log, type: .info, index)
            updateTabState(tabView: tabViews[index], item: items[index], selected: true)

            // Notify listener
            if notifyListener {
                os_log("TabBar.setSelectedIndex: Notifying listener for index %d, path=%{public}@", log: LingXiaTabBar.log, type: .info, index, items[index].pagePath)
                onTabSelectedListener?(index, items[index].pagePath)
            } else {
                os_log("TabBar.setSelectedIndex: Not notifying listener (notifyListener=false)", log: LingXiaTabBar.log, type: .debug)
            }
        } else {
            os_log("setSelectedIndex: Already at index %d, no change needed",
                   log: LingXiaTabBar.log, type: .info, index)
        }
    }

    private func createTabView(item: TabBarItem, config: TabBarConfig, isSelected: Bool) -> UIView {
        let isVertical = config.position == .left || config.position == .right

        let containerView = UIView()
        // Force transparent background for tab item containers
        containerView.backgroundColor = UIColor.clear

        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.alignment = .center
        stackView.distribution = .fill
        stackView.spacing = 2
        // Force transparent background for stackView too
        stackView.backgroundColor = UIColor.clear

        // Add icon
        let iconImageView = UIImageView()
        iconImageView.contentMode = .scaleAspectFit
        iconImageView.image = getIconImage(item: item, selected: isSelected)

        // Ensure tintColor is applied for template images using platform extension
        let iconColorString = isSelected ?
            (config.selectedColor ?? TabBarConfig.DEFAULT_SELECTED_COLOR) :
            (config.color ?? TabBarConfig.DEFAULT_UNSELECTED_COLOR)
        iconImageView.tintColor = config.parseColor(iconColorString) ?? UIColor.systemBlue

        let iconSize = isVertical ? LingXiaTabBar.VERTICAL_ITEM_ICON_SIZE : LingXiaTabBar.HORIZONTAL_ITEM_ICON_SIZE
        iconImageView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            iconImageView.widthAnchor.constraint(equalToConstant: iconSize),
            iconImageView.heightAnchor.constraint(equalToConstant: iconSize)
        ])

        stackView.addArrangedSubview(iconImageView)

        // Add text if available
        if let text = item.text, !text.isEmpty {
            let textLabel = UILabel()
            textLabel.text = text
            let textColorString = isSelected ?
                (config.selectedColor ?? TabBarConfig.DEFAULT_SELECTED_COLOR) :
                (config.color ?? TabBarConfig.DEFAULT_UNSELECTED_COLOR)
            textLabel.textColor = config.parseColor(textColorString) ?? UIColor.systemBlue
            textLabel.font = UIFont.systemFont(ofSize: isVertical ? LingXiaTabBar.VERTICAL_ITEM_TEXT_SIZE : LingXiaTabBar.HORIZONTAL_ITEM_TEXT_SIZE)
            textLabel.textAlignment = .center
            textLabel.numberOfLines = 1

            stackView.addArrangedSubview(textLabel)
        }

        containerView.addSubview(stackView)
        stackView.translatesAutoresizingMaskIntoConstraints = false

        // For transparent TabBars, center the content vertically for better positioning
        let isTransparent = TabBarConfig.isTransparent(config.backgroundColor)

        if isTransparent {
            // For transparent TabBars, position content in the visible safe area (not center of entire container)
            NSLayoutConstraint.activate([
                stackView.centerXAnchor.constraint(equalTo: containerView.centerXAnchor),
                stackView.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 8), // Top-aligned within safe area
                stackView.leadingAnchor.constraint(greaterThanOrEqualTo: containerView.leadingAnchor, constant: 4),
                stackView.trailingAnchor.constraint(lessThanOrEqualTo: containerView.trailingAnchor, constant: -4)
            ])
        } else {
            // For opaque TabBars, use the original top-aligned layout
            NSLayoutConstraint.activate([
                stackView.centerXAnchor.constraint(equalTo: containerView.centerXAnchor),
                stackView.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 4),
                stackView.leadingAnchor.constraint(greaterThanOrEqualTo: containerView.leadingAnchor, constant: 4),
                stackView.trailingAnchor.constraint(lessThanOrEqualTo: containerView.trailingAnchor, constant: -4),
                stackView.bottomAnchor.constraint(lessThanOrEqualTo: containerView.bottomAnchor, constant: -4)
            ])
        }

        // Add tap gesture
        let tapGesture = UITapGestureRecognizer(target: self, action: #selector(tabTapped(_:)))
        containerView.addGestureRecognizer(tapGesture)
        // Tag will be set properly in setItems method

        return containerView
    }

    @objc private func tabTapped(_ gesture: UITapGestureRecognizer) {
        guard let view = gesture.view else {
            os_log("TabBar.tabTapped: gesture.view is nil", log: LingXiaTabBar.log, type: .error)
            return
        }
        let index = view.tag

        if index < items.count {
            let item = items[index]
            os_log("TabBar.tabTapped: Tapping tab at index %d, path=%{public}@", log: LingXiaTabBar.log, type: .info, index, item.pagePath)
            // Update TabBar's internal state first
            setSelectedIndex(index, notifyListener: true)
        } else {
            os_log("TabBar.tabTapped: Invalid index %d >= items.count %d", log: LingXiaTabBar.log, type: .error, index, items.count)
        }
    }

    private func getIconImage(item: TabBarItem, selected: Bool) -> UIImage? {
        let iconPath = selected && !item.selectedIconPath.isEmpty ? item.selectedIconPath : item.iconPath

        // Determine the color to use for the icon using platform extension
        let iconColorString = selected ?
            (config.selectedColor ?? TabBarConfig.DEFAULT_SELECTED_COLOR) :
            (config.color ?? TabBarConfig.DEFAULT_UNSELECTED_COLOR)
        let iconColor = config.parseColor(iconColorString) ?? UIColor.systemBlue

        // First try as SF Symbol (iOS system icons)
        if let systemImage = UIImage(systemName: iconPath) {
            // Use .alwaysTemplate for proper color rendering
            let coloredImage = systemImage.withTintColor(iconColor, renderingMode: .alwaysTemplate)
            os_log("getIconImage: Created SF Symbol with template rendering",
                   log: OSLog(subsystem: "LingXia", category: "TabBar"), type: .debug)
            return coloredImage
        }

        // Try as bundle resource from main bundle (example app resources)
        if let bundleImage = UIImage(named: iconPath) {
            // Apply color to bundle image using template rendering
            let coloredImage = bundleImage.withTintColor(iconColor, renderingMode: .alwaysTemplate)
            os_log("getIconImage: Created bundle image with template rendering",
                   log: OSLog(subsystem: "LingXia", category: "TabBar"), type: .debug)
            return coloredImage
        }

        // Handle absolute paths from native (based on data dir from init)
        let resolvedPath = resolveIconPath(iconPath)
        if let image = UIImage(contentsOfFile: resolvedPath) {
            let coloredImage = image.withTintColor(iconColor, renderingMode: .alwaysTemplate)
            os_log("getIconImage: Created file image with template rendering",
                   log: OSLog(subsystem: "LingXia", category: "TabBar"), type: .debug)
            return coloredImage
        }

        // Create default icon if file doesn't exist
        if let defaultIcon = createDefaultIcon(selected: selected) {
            os_log("getIconImage: Created default icon",
                   log: OSLog(subsystem: "LingXia", category: "TabBar"), type: .debug)
            return defaultIcon
        }

        // Final fallback - use a simple SF Symbol with color
        let fallbackIcon = UIImage(systemName: "circle.fill") ?? UIImage()
        let coloredFallback = fallbackIcon.withTintColor(iconColor, renderingMode: .alwaysTemplate)
        os_log("getIconImage: Using fallback icon with template rendering",
               log: OSLog(subsystem: "LingXia", category: "TabBar"), type: .debug)
        return coloredFallback
    }

    /// Resolves icon path, handling both absolute paths from native and relative paths
    /// - Parameter iconPath: The icon path from TabBar configuration
    /// - Returns: Resolved absolute path within app sandbox
    private func resolveIconPath(_ iconPath: String) -> String {
        // If it's already an absolute path and starts with app directories, use as-is
        if iconPath.hasPrefix("/") {
            let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
            let cachesPath = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first?.path ?? ""

            // Verify the path is within our app sandbox for security
            if iconPath.hasPrefix(documentsPath) || iconPath.hasPrefix(cachesPath) {
                return iconPath
            }
        }

        // For relative paths, try to resolve relative to documents directory
        if !iconPath.hasPrefix("/") {
            let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
            return "\(documentsPath)/\(iconPath)"
        }

        // Fallback: return original path
        return iconPath
    }

    private func createDefaultIcon(selected: Bool) -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        let colorString = selected ?
            (config.selectedColor ?? TabBarConfig.DEFAULT_SELECTED_COLOR) :
            (config.color ?? TabBarConfig.DEFAULT_UNSELECTED_COLOR)
        let color = config.parseColor(colorString) ?? UIColor.systemBlue

        UIGraphicsBeginImageContextWithOptions(size, false, 0)
        defer { UIGraphicsEndImageContext() }

        guard UIGraphicsGetCurrentContext() != nil else {
            return nil
        }

        color.setFill()
        UIBezierPath(ovalIn: CGRect(origin: .zero, size: size)).fill()
        return UIGraphicsGetImageFromCurrentImageContext()
    }

    private func updateTabState(tabView: UIView, item: TabBarItem, selected: Bool) {
        // Find the stack view and update icon and text colors
        if let stackView = tabView.subviews.first as? UIStackView {
            // Update icon
            if let iconImageView = stackView.arrangedSubviews.first as? UIImageView {
                iconImageView.image = getIconImage(item: item, selected: selected)

                // Also update tint color using platform extension
                let iconColorString = selected ?
                    (config.selectedColor ?? TabBarConfig.DEFAULT_SELECTED_COLOR) :
                    (config.color ?? TabBarConfig.DEFAULT_UNSELECTED_COLOR)
                iconImageView.tintColor = config.parseColor(iconColorString) ?? UIColor.systemBlue
            } else {
                os_log("updateTabState: Could not find iconImageView", log: LingXiaTabBar.log, type: .error)
            }

            // Update text color if text exists using platform extension
            if stackView.arrangedSubviews.count > 1,
               let textLabel = stackView.arrangedSubviews[1] as? UILabel {
                let textColorString = selected ?
                    (config.selectedColor ?? TabBarConfig.DEFAULT_SELECTED_COLOR) :
                    (config.color ?? TabBarConfig.DEFAULT_UNSELECTED_COLOR)
                textLabel.textColor = config.parseColor(textColorString) ?? UIColor.systemBlue
            }
        } else {
            os_log("updateTabState: Could not find stackView", log: LingXiaTabBar.log, type: .error)
        }
    }

    /// Recursively sets all subviews to have transparent background
    private func setAllSubviewsTransparent(view: UIView) {
        // Only set transparent background for container views, not for tab content
        if view is UIStackView || view.subviews.count > 0 {
            view.backgroundColor = UIColor.clear
            view.layer.backgroundColor = UIColor.clear.cgColor
            view.isOpaque = false
            view.layer.isOpaque = false

            // Remove any visual effects
            view.layer.shadowOpacity = 0
            view.layer.borderWidth = 0
        }

        for subview in view.subviews {
            setAllSubviewsTransparent(view: subview)
        }
    }

    /// Public method to enforce transparency on the TabBar and all its subviews
    public func forceTransparencyMode() {
        guard TabBarConfig.isTransparent(config.backgroundColor) else {
            return
        }

        // Force main TabBar transparency
        backgroundColor = UIColor.clear
        layer.backgroundColor = UIColor.clear.cgColor
        isOpaque = false
        layer.isOpaque = false
        layer.shadowOpacity = 0
        layer.borderWidth = 0
        alpha = 1.0
        layer.masksToBounds = false

        // Apply transparency to itemsContainer and all tab views
        if let itemsContainer = itemsContainer {
            itemsContainer.backgroundColor = UIColor.clear
            itemsContainer.layer.backgroundColor = UIColor.clear.cgColor
            itemsContainer.isOpaque = false
            itemsContainer.layer.isOpaque = false

            // Apply to all tab item views
            for arrangedSubview in itemsContainer.arrangedSubviews {
                arrangedSubview.backgroundColor = UIColor.clear
                arrangedSubview.layer.backgroundColor = UIColor.clear.cgColor
                arrangedSubview.isOpaque = false
                arrangedSubview.layer.isOpaque = false

                // Apply to nested views
                for subview in arrangedSubview.subviews {
                    subview.backgroundColor = UIColor.clear
                    subview.layer.backgroundColor = UIColor.clear.cgColor
                    subview.isOpaque = false
                    subview.layer.isOpaque = false
                }
            }
        }
    }

    /// Override draw to ensure transparent background when configured
    public override func draw(_ rect: CGRect) {
        // Ensure transparency before drawing if configured
        if TabBarConfig.isTransparent(config.backgroundColor) {
            backgroundColor = UIColor.clear
            layer.backgroundColor = UIColor.clear.cgColor
            isOpaque = false
            layer.isOpaque = false
        }

        super.draw(rect)
    }
}

#endif

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

#if os(iOS)
// MARK: - iOS Platform Extensions
extension TabBarConfig {
    /// Determines if a color string should be treated as transparent
    /// - Parameter colorString: The color string to check (can be nil)
    /// - Returns: true if the color is nil, "transparent", or has very low alpha
    static func isTransparent(_ colorString: String?) -> Bool {
        guard let colorString = colorString else { return true }
        return colorString.lowercased() == "transparent" || colorString.isEmpty
    }

    /// Converts a hex color string to UIColor
    /// - Parameter hexString: The hex color string (e.g., "#FFFFFF")
    /// - Returns: UIColor or nil if parsing fails
    func parseColor(_ hexString: String?) -> UIColor? {
        guard let hexString = hexString, !hexString.isEmpty else { return nil }
        return UIColor(hexString: hexString)
    }

    /// Gets the resolved background color for the tab bar
    /// - Parameter isVertical: Whether the tab bar is positioned vertically
    /// - Returns: The appropriate UIColor
    func resolvedBackgroundColor(isVertical: Bool) -> UIColor {
        if Self.isTransparent(backgroundColor) {
            return UIColor.clear
        }

        if let bgColor = parseColor(backgroundColor) {
            return bgColor
        }

        let defaultColor = UIColor(hexString: Self.DEFAULT_BACKGROUND_COLOR) ?? UIColor.white
        return isVertical ? UIColor(red: 0.97, green: 0.97, blue: 0.97, alpha: 1.0) : defaultColor
    }
}

extension LingXiaTabBar: TabBarProtocol {
    // Already implements all required methods
}

#endif

#if os(macOS)
// MARK: - macOS Platform Extensions
extension TabBarConfig {
    static func isTransparent(_ colorString: String?) -> Bool {
        guard let colorString = colorString else { return true }
        return colorString.lowercased() == "transparent" || colorString.isEmpty
    }

    func parseColor(_ hexString: String?) -> NSColor? {
        guard let hexString = hexString, !hexString.isEmpty else { return nil }
        return NSColor(hexString: hexString)
    }

    func resolvedBackgroundColor(isVertical: Bool) -> NSColor {
        if Self.isTransparent(backgroundColor) {
            return NSColor.clear
        }

        if let bgColor = parseColor(backgroundColor) {
            return bgColor
        }

        let defaultColor = NSColor(hexString: Self.DEFAULT_BACKGROUND_COLOR) ?? NSColor.white
        return isVertical ? NSColor(red: 0.97, green: 0.97, blue: 0.97, alpha: 1.0) : defaultColor
    }
}

class TabBarButton: NSButton {
    // Use NSButton's default hit testing for proper click detection
}

@MainActor
public class macOSTabBar: NSView, TabBarProtocol {
    public var config: TabBarConfig = TabBarConfig()
    public var onTabSelectedListener: ((String) -> Void)?

    private var tabButtons: [NSButton] = []
    private var stackView: NSStackView?
    private var selectedIndex: Int = 0
    private var appId: String = ""

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
    }

    public func setConfig(config: TabBarConfig) {
        self.config = config
        setupTabButtons()
        updateAppearance()
    }

    public func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        self.onTabSelectedListener = { path in
            if let index = self.findTabIndexByPath(path) {
                listener(index, path)
            }
        }
    }

    public func findTabIndexByPath(_ path: String) -> Int? {
        return config.list.firstIndex { $0.pagePath == path }
    }

    public func syncSelectedTabWithCurrentPath(_ currentPath: String) {
        if let index = findTabIndexByPath(currentPath) {
            setSelectedIndex(index, notifyListener: false)
        }
    }

    public func setSelectedIndex(_ index: Int, notifyListener: Bool) {
        guard index >= 0 && index < tabButtons.count else { return }

        selectedIndex = index
        updateButtonStates()

        if notifyListener {
            let selectedPath = config.list[index].pagePath
            onTabSelectedListener?(selectedPath)
        }
    }

    private func setupTabButtons() {
        tabButtons.forEach { $0.removeFromSuperview() }
        tabButtons.removeAll()
        stackView?.removeFromSuperview()

        guard !config.list.isEmpty else { return }

        for (index, item) in config.list.enumerated() {
            let button = createTabButton(for: item, at: index)
            tabButtons.append(button)
        }

        layoutTabButtons()
        setSelectedIndex(0, notifyListener: false)
    }

    private func createTabButton(for item: TabBarItem, at index: Int) -> NSButton {
        let button = TabBarButton()
        button.title = item.text ?? ""
        button.font = NSFont.systemFont(ofSize: TabBarConstants.ITEM_FONT_SIZE, weight: .medium)
        button.isBordered = false
        button.target = self
        button.action = #selector(tabButtonTapped(_:))
        button.tag = index
        button.translatesAutoresizingMaskIntoConstraints = false

        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor

        if !item.iconPath.isEmpty {
            setButtonIcon(button: button, iconPath: item.iconPath, selected: false)
        }

        button.imagePosition = .imageAbove
        button.imageScaling = .scaleProportionallyDown

        return button
    }

    private func layoutTabButtons() {
        guard !tabButtons.isEmpty else { return }

        stackView = NSStackView(views: tabButtons)
        stackView!.translatesAutoresizingMaskIntoConstraints = false
        stackView!.distribution = .fillEqually
        stackView!.alignment = .centerY
        addSubview(stackView!)

        NSLayoutConstraint.activate([
            stackView!.leadingAnchor.constraint(equalTo: leadingAnchor),
            stackView!.trailingAnchor.constraint(equalTo: trailingAnchor),
            stackView!.topAnchor.constraint(equalTo: topAnchor),
            stackView!.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }

    private func updateAppearance() {
        let resolvedColor = config.resolvedBackgroundColor(isVertical: false)
        layer?.backgroundColor = resolvedColor.cgColor
    }

    private func updateButtonStates() {
        for (index, button) in tabButtons.enumerated() {
            let isSelected = index == selectedIndex
            let item = config.list[index]

            let iconPath = isSelected ? item.selectedIconPath : item.iconPath
            if !iconPath.isEmpty {
                setButtonIcon(button: button, iconPath: iconPath, selected: isSelected)
            }

            let textColor = getTabColor(selected: isSelected)
            button.contentTintColor = textColor
        }
    }

    private func getTabColor(selected: Bool) -> NSColor {
        let colorString = selected ? config.selectedColor : config.color
        let defaultColor = selected ? TabBarConfig.DEFAULT_SELECTED_COLOR : TabBarConfig.DEFAULT_UNSELECTED_COLOR

        if let color = config.parseColor(colorString ?? defaultColor) {
            return color
        }

        return selected ? NSColor.systemBlue : NSColor.secondaryLabelColor
    }

    private func setButtonIcon(button: NSButton, iconPath: String, selected: Bool) {
        var image: NSImage?

        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            if #available(macOS 11.0, *) {
                image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)
                image?.isTemplate = true
            }
        } else if iconPath.hasPrefix("/") {
            image = NSImage(contentsOfFile: iconPath)
        } else {
            image = NSImage(named: iconPath)

            if image == nil && !appId.isEmpty {
                let resourcesPath = getResourcesPath()
                let fullPath = "\(resourcesPath)/\(appId)/\(iconPath)"
                image = NSImage(contentsOfFile: fullPath)
            }
        }

        if let image = image {
            let resizedImage = resizeImage(image, to: NSSize(width: TabBarConstants.ICON_SIZE, height: TabBarConstants.ICON_SIZE))
            button.image = resizedImage
        }
    }

    @objc private func tabButtonTapped(_ sender: NSButton) {
        let index = sender.tag
        setSelectedIndex(index, notifyListener: true)
    }

    private func getResourcesPath() -> String {
        return Bundle.main.resourcePath ?? ""
    }

    private func resizeImage(_ image: NSImage, to size: NSSize) -> NSImage {
        let resizedImage = NSImage(size: size)
        resizedImage.lockFocus()
        image.draw(in: NSRect(origin: .zero, size: size))
        resizedImage.unlockFocus()
        return resizedImage
    }
}

extension macOSTabBar {
    public func findTabIndexByPath(_ path: String) -> Int {
        return config.list.firstIndex { $0.pagePath == path } ?? -1
    }
}

#endif
