import SwiftUI
import Foundation
import CLingXiaFFI
import os.log

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// Extension to add helper methods to swift-bridge generated TabBarConfig
extension TabBarConfig {
    /// Get position as enum
    public var positionEnum: TabBarPosition {
        switch position {
        case 1: return .left
        case 2: return .right
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
            case .Start:
                startItems.append(item)
            case .End:
                endItems.append(item)
            case .Center:
                centerItems.append(item)
            }
        }

        return (start: startItems, center: centerItems, end: endItems)
    }

    /// Check if color is transparent
    public static func isTransparent(_ colorValue: UInt32) -> Bool {
        return (colorValue >> 24) & 0xFF == 0
    }

    /// Get resolved background color for this configuration
    public func getResolvedBackgroundColor(isVertical: Bool) -> PlatformColor {
        return TabBarHelper.resolvedBackgroundColor(background_color, isVertical: isVertical)
    }
}

/// Position enum for TabBar
public enum TabBarPosition {
    case bottom, left, right
}

/// Essential constants for TabBar layout
public struct TabBarConstants {
    public static let DEFAULT_SPACING: CGFloat = 8
    public static let CENTER_SPACING: CGFloat = 8
    public static let MINIMAL_SPACER_SIZE: CGFloat = 4
}

/// Extension to add helper methods to swift-bridge generated TabBarItem
extension TabBarItem {
    /// Check if item is visible
    public var visible: Bool { true }

    /// Get page path string
    public var cachedPagePath: String {
        return page_path.toString()
    }

    /// Get text string
    public var cachedText: String {
        return text.toString()
    }

    /// Get icon path string
    public var cachedIconPath: String {
        return icon_path.toString()
    }

    /// Get selected icon path string
    public var cachedSelectedIconPath: String {
        return selected_icon_path.toString()
    }
}

/// Helper methods for TabBar styling and color management
public struct TabBarHelper {
    /// Get resolved background color for TabBar
    public static func resolvedBackgroundColor(_ colorValue: UInt32, isVertical: Bool) -> PlatformColor {
        if (colorValue >> 24) & 0xFF == 0 {
            if isVertical {
                return PlatformColor(red: 0.95, green: 0.95, blue: 0.95, alpha: 1.0)
            } else {
                return PlatformColor(red: 0.98, green: 0.98, blue: 0.98, alpha: 1.0)
            }
        }
        return PlatformColor(argb: colorValue)
    }
}

/// Unified SwiftUI TabBar for iOS and macOS
public struct LxAppTabBar: View {
    let appId: String
    let config: TabBarConfig
    @Binding var selectedIndex: Int
    let onTabSelected: (Int, String) -> Void

    public init(
        appId: String,
        config: TabBarConfig,
        selectedIndex: Binding<Int>,
        onTabSelected: @escaping (Int, String) -> Void
    ) {
        self.appId = appId
        self.config = config
        self._selectedIndex = selectedIndex
        self.onTabSelected = onTabSelected
    }

    public var body: some View {
        let items = config.getItems(appId: appId)

        Group {
            switch config.positionEnum {
            case .bottom:
                if hasGroupField(items: items) {
                    buildGroupedHorizontalTabBar(items: items)
                        .frame(height: LxAppTheme.Metrics.tabBarHeight)
                } else {
                    buildHorizontalTabBar(items: items)
                        .frame(height: LxAppTheme.Metrics.tabBarHeight)
                }

            case .left, .right:
                if hasGroupField(items: items) {
                    buildGroupedVerticalTabBar(items: items)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    buildVerticalTabBar(items: items)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .background(getTabBarBackgroundColor())
    }

    @ViewBuilder
    private func buildHorizontalTabBar(items: [TabBarItem]) -> some View {
        HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                buildTabItem(item: item, index: index)
                    .frame(maxWidth: .infinity)
            }
        }
        .padding(.horizontal, LxAppTheme.Metrics.largeSpacing)
    }

    @ViewBuilder
    private func buildVerticalTabBar(items: [TabBarItem]) -> some View {
        VStack(spacing: LxAppTheme.Metrics.standardSpacing) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                buildTabItem(item: item, index: index)
            }
        }
        .padding(.vertical, LxAppTheme.Metrics.largeSpacing)
    }

    @ViewBuilder
    private func buildGroupedHorizontalTabBar(items: [TabBarItem]) -> some View {
        HStack(spacing: 0) {
            // Start items (group 1)
            let startItems = getStartItems(items: items)
            if !startItems.isEmpty {
                HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(startItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
                .padding(.leading, LxAppTheme.Metrics.largeSpacing)
            }

            // Flexible spacer
            Spacer()

            // Center items (group 0)
            let centerItems = getCenterItems(items: items)
            if !centerItems.isEmpty {
                HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(centerItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
            }

            // Flexible spacer
            Spacer()

            // End items (group 2)
            let endItems = getEndItems(items: items)
            if !endItems.isEmpty {
                HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(endItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
                .padding(.trailing, LxAppTheme.Metrics.largeSpacing)
            }
        }
    }

    @ViewBuilder
    private func buildGroupedVerticalTabBar(items: [TabBarItem]) -> some View {
        let startItems = getStartItems(items: items)
        let centerItems = getCenterItems(items: items)
        let endItems = getEndItems(items: items)

        VStack(alignment: .center, spacing: 0) {
            // Start items (group 1)
            if !startItems.isEmpty {
                VStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(startItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
                .frame(maxWidth: .infinity)
            }

            Spacer()

            // Center items (group 0)
            if !centerItems.isEmpty {
                VStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(centerItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
                .frame(maxWidth: .infinity)
                Spacer()
            }

            // End items (group 2)
            if !endItems.isEmpty {
                VStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(endItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
                .frame(maxWidth: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }

    @ViewBuilder
    private func buildTabItem(item: TabBarItem, index: Int) -> some View {
        let isSelected = (index == selectedIndex)

        Button(action: {
            selectedIndex = index
            onTabSelected(index, item.cachedPagePath)
        }) {
            VStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                // Tab icon
                if !item.cachedIconPath.isEmpty {
                    buildTabIcon(item: item, isSelected: isSelected)
                }

                // Tab title
                if !item.cachedText.isEmpty {
                    Text(item.cachedText)
                        .font(LxAppTheme.Typography.tabTitle)
                        .foregroundColor(isSelected ?
                            Color(PlatformColor(argb: config.selected_color)) :
                            Color(PlatformColor(argb: config.color)))
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, LxAppTheme.Metrics.smallSpacing)
        }
        .buttonStyle(PlainButtonStyle())
    }

    /// Find index of item in the original items array by path
    private func findItemIndex(for item: TabBarItem, in items: [TabBarItem]) -> Int {
        return items.firstIndex(where: { $0.cachedPagePath == item.cachedPagePath }) ?? 0
    }

    /// Check if any item has group field
    private func hasGroupField(items: [TabBarItem]) -> Bool {
        return items.contains { $0.group != .Center }
    }

    private func getStartItems(items: [TabBarItem]) -> [TabBarItem] {
        return items.filter { $0.group == .Start }
    }

    private func getCenterItems(items: [TabBarItem]) -> [TabBarItem] {
        return items.filter { $0.group == .Center }
    }

    private func getEndItems(items: [TabBarItem]) -> [TabBarItem] {
        return items.filter { $0.group == .End }
    }

    @ViewBuilder
    private func buildTabIcon(item: TabBarItem, isSelected: Bool) -> some View {
        let iconPath = isSelected && !item.cachedSelectedIconPath.isEmpty
            ? item.cachedSelectedIconPath
            : item.cachedIconPath

        let iconColor = isSelected ?
            Color(PlatformColor(argb: config.selected_color)) :
            Color.black  // Use pure black for maximum contrast

        if iconPath.hasPrefix("SF:") {
            // System SF Symbol
            let symbolName = String(iconPath.dropFirst(3))
            Image(systemName: symbolName)
                .font(.system(size: LxAppTheme.Metrics.tabIconSize))
                .foregroundColor(iconColor)
        } else if iconPath.hasPrefix("/") {
            // Absolute path
            if let image = loadPlatformImage(from: iconPath) {
                image
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            }
        } else {
            // Try bundle first, then Resources directory
            if let bundleImage = loadBundleImage(named: iconPath) {
                bundleImage
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            } else {
                // Try Resources directory with appId
                let resourcesPath = getResourcesPath()
                let fullPath = "\(resourcesPath)/\(appId)/\(iconPath)"
                if let resourceImage = loadPlatformImage(from: fullPath) {
                    resourceImage
                        .resizable()
                        .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                        .foregroundColor(iconColor)
                }
            }
        }
    }

    private func getResourcesPath() -> String {
        return Bundle.main.resourcePath ?? ""
    }

    /// Load platform-specific image from path
    private func loadPlatformImage(from path: String) -> Image? {
        #if os(iOS)
        if let uiImage = UIImage(contentsOfFile: path) {
            return Image(uiImage: uiImage)
        }
        #else
        if let nsImage = NSImage(contentsOfFile: path) {
            return Image(nsImage: nsImage)
        }
        #endif
        return nil
    }

    /// Load platform-specific image from bundle
    private func loadBundleImage(named name: String) -> Image? {
        #if os(iOS)
        if let uiImage = UIImage(named: name) {
            return Image(uiImage: uiImage)
        }
        #else
        if let nsImage = NSImage(named: name) {
            return Image(nsImage: nsImage)
        }
        #endif
        return nil
    }

    private func getTabBarBackgroundColor() -> Color {
        let platformColor = PlatformColor(argb: config.background_color)
        return Color(platformColor)
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
    func selectTab(index: Int)
    func setSelectedIndex(_ index: Int, notifyListener: Bool)
}

/// Protocol for TabBar UI implementations
@MainActor
public protocol TabBarUIDelegate: AnyObject {
    func updateTabSelection(selectedIndex: Int)
    func updateConfiguration()
    func updateItems(_ items: [TabBarItem])
}

#if os(iOS)
import UIKit

/// UIKit TabBar implementation for iOS
@MainActor
public class iOSTabBarWrapper: UIView {
    private var tabBarConfig: TabBarConfig?
    private var appId: String = ""
    private var selectedIndex: Int = 0
    private var onTabSelectedCallback: ((Int, String) -> Void)?

    // Public accessor for tabBarConfig
    public var config: TabBarConfig? {
        return tabBarConfig
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        setupView()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }

    private func setupView() {
        backgroundColor = UIColor.clear
    }

    public func setConfig(config: TabBarConfig, appId: String) {
        self.tabBarConfig = config
        self.appId = appId
        updateLayout()
    }

    public func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        self.onTabSelectedCallback = listener
    }

    public func selectTab(index: Int) {
        self.selectedIndex = index
        updateLayout()
    }

    public func getSelectedIndex() -> Int {
        return selectedIndex
    }

    public func syncSelectedTabWithCurrentPath(_ currentPath: String) {
        // Find the tab index for the current path
        guard let config = tabBarConfig else { return }
        let items = config.getItems(appId: appId)

        for (index, item) in items.enumerated() {
            if item.page_path.toString() == currentPath {
                selectTab(index: index)
                break
            }
        }
    }

    public func forceTransparencyMode() {
        backgroundColor = UIColor.clear
        layer.backgroundColor = UIColor.clear.cgColor
    }

    private func updateLayout() {
        guard let config = tabBarConfig else { return }

        let items = config.getItems(appId: appId)

        // Only recreate layout if items have changed
        if shouldRecreateLayout(items: items, config: config) {
            setupUIKitLayout(items: items, config: config)
        } else {
            // Just update selection state
            updateSelectionState()
        }
    }

    private var lastItemsHash: Int = 0
    private var lastConfigHash: Int = 0

    private func shouldRecreateLayout(items: [TabBarItem], config: TabBarConfig) -> Bool {
        let itemsHash = items.map { "\($0.cachedPagePath)-\($0.group)" }.joined().hashValue
        let configHash = "\(config.position)-\(config.items_count)".hashValue

        let shouldRecreate = itemsHash != lastItemsHash || configHash != lastConfigHash

        if shouldRecreate {
            lastItemsHash = itemsHash
            lastConfigHash = configHash
        }

        return shouldRecreate
    }

    private func updateSelectionState() {
        // Update button states without recreating the entire layout
        for subview in subviews {
            updateButtonSelectionState(in: subview)
        }
    }

    private func updateButtonSelectionState(in view: UIView) {
        if let button = view as? UIButton {
            updateSingleButtonState(button)
        } else {
            for subview in view.subviews {
                updateButtonSelectionState(in: subview)
            }
        }
    }

    private func updateSingleButtonState(_ button: UIButton) {
        guard let config = tabBarConfig else { return }
        let items = config.getItems(appId: appId)
        let index = button.tag

        if index < items.count {
            let item = items[index]
            let isSelected = (index == selectedIndex)

            // Update button appearance based on selection state
            if let stackView = button.subviews.first as? UIStackView {
                for arrangedSubview in stackView.arrangedSubviews {
                    if let label = arrangedSubview as? UILabel {
                        label.textColor = isSelected ?
                            PlatformColor(argb: config.selected_color) :
                            PlatformColor(argb: config.color)
                    } else if let imageView = arrangedSubview as? UIImageView {
                        imageView.tintColor = isSelected ? UIColor.systemBlue : UIColor.secondaryLabel
                    }
                }
            }
        }
    }

    private func setupUIKitLayout(items: [TabBarItem], config: TabBarConfig) {
        subviews.forEach { $0.removeFromSuperview() }

        let containerView = UIView()
        containerView.backgroundColor = UIColor.clear
        containerView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(containerView)

        let isVertical = config.position == 1 || config.position == 2
        let hasGroupField = items.contains { $0.group != .Center }

        if isVertical && hasGroupField {
            setupVerticalGroupedLayout(items: items, config: config, containerView: containerView)
        } else if isVertical {
            setupVerticalLayout(items: items, config: config, containerView: containerView)
        } else if hasGroupField {
            setupHorizontalGroupedLayout(items: items, config: config, containerView: containerView)
        } else {
            setupHorizontalLayout(items: items, config: config, containerView: containerView)
        }

        NSLayoutConstraint.activate([
            containerView.topAnchor.constraint(equalTo: topAnchor),
            containerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            containerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            containerView.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }

    private func setupVerticalGroupedLayout(items: [TabBarItem], config: TabBarConfig, containerView: UIView) {
        setupGroupedLayout(items: items, config: config, containerView: containerView, isVertical: true)
    }

    private func setupHorizontalGroupedLayout(items: [TabBarItem], config: TabBarConfig, containerView: UIView) {
        setupGroupedLayout(items: items, config: config, containerView: containerView, isVertical: false)
    }

    private func setupGroupedLayout(items: [TabBarItem], config: TabBarConfig, containerView: UIView, isVertical: Bool) {
        let stackView = UIStackView()
        stackView.axis = isVertical ? .vertical : .horizontal
        stackView.distribution = .fill
        stackView.alignment = .center
        stackView.spacing = isVertical ? 8 : 0
        stackView.translatesAutoresizingMaskIntoConstraints = false
        containerView.addSubview(stackView)

        let startItems = items.filter { $0.group == .Start }
        let centerItems = items.filter { $0.group == .Center }
        let endItems = items.filter { $0.group == .End }

        // Add start items
        addGroupContainer(items: startItems, allItems: items, config: config, to: stackView, isVertical: isVertical)

        // Add flexible spacer (only if we have both start and end items)
        if !startItems.isEmpty && (!centerItems.isEmpty || !endItems.isEmpty) {
            addFlexibleSpacer(to: stackView, isVertical: isVertical)
        }

        // Add center items (only for horizontal layout)
        if !isVertical && !centerItems.isEmpty {
            addGroupContainer(items: centerItems, allItems: items, config: config, to: stackView, isVertical: isVertical)

            // Add spacer after center items if we have end items
            if !endItems.isEmpty {
                addFlexibleSpacer(to: stackView, isVertical: isVertical)
            }
        }

        // Add end items
        addGroupContainer(items: endItems, allItems: items, config: config, to: stackView, isVertical: isVertical)

        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 8),
            stackView.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: 8),
            stackView.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -8),
            stackView.bottomAnchor.constraint(equalTo: containerView.bottomAnchor, constant: -8)
        ])
    }

    private func addGroupContainer(items: [TabBarItem], allItems: [TabBarItem], config: TabBarConfig, to stackView: UIStackView, isVertical: Bool) {
        guard !items.isEmpty else { return }

        if isVertical {
            // For vertical layout, add items directly to the main stack
            for item in items {
                let originalIndex = allItems.firstIndex { $0.page_path.toString() == item.page_path.toString() } ?? 0
                let tabView = createUIKitTabItem(item: item, index: originalIndex, config: config)
                stackView.addArrangedSubview(tabView)
            }
        } else {
            // For horizontal layout, create a container for the group
            let groupContainer = UIStackView()
            groupContainer.axis = .horizontal
            groupContainer.distribution = .fillEqually
            groupContainer.alignment = .center
            groupContainer.spacing = 8
            groupContainer.translatesAutoresizingMaskIntoConstraints = false

            for item in items {
                let originalIndex = allItems.firstIndex { $0.page_path.toString() == item.page_path.toString() } ?? 0
                let tabView = createUIKitTabItem(item: item, index: originalIndex, config: config)
                groupContainer.addArrangedSubview(tabView)
            }
            stackView.addArrangedSubview(groupContainer)
        }
    }

    private func addFlexibleSpacer(to stackView: UIStackView, isVertical: Bool) {
        let spacer = UIView()
        spacer.backgroundColor = UIColor.clear
        spacer.translatesAutoresizingMaskIntoConstraints = false

        if isVertical {
            spacer.setContentHuggingPriority(.defaultLow, for: .vertical)
            spacer.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
        } else {
            spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
            spacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        }

        stackView.addArrangedSubview(spacer)
    }

    private func setupVerticalLayout(items: [TabBarItem], config: TabBarConfig, containerView: UIView) {
        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.distribution = .fillEqually
        stackView.alignment = .center
        stackView.spacing = 8
        stackView.translatesAutoresizingMaskIntoConstraints = false
        containerView.addSubview(stackView)

        for (index, item) in items.enumerated() {
            let tabView = createUIKitTabItem(item: item, index: index, config: config)
            stackView.addArrangedSubview(tabView)
        }

        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 8),
            stackView.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: 8),
            stackView.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -8),
            stackView.bottomAnchor.constraint(equalTo: containerView.bottomAnchor, constant: -8)
        ])
    }

    private func setupHorizontalLayout(items: [TabBarItem], config: TabBarConfig, containerView: UIView) {
        let stackView = UIStackView()
        stackView.axis = .horizontal
        stackView.distribution = .fillEqually
        stackView.alignment = .center
        stackView.spacing = 8
        stackView.translatesAutoresizingMaskIntoConstraints = false
        containerView.addSubview(stackView)

        for (index, item) in items.enumerated() {
            let tabView = createUIKitTabItem(item: item, index: index, config: config)
            stackView.addArrangedSubview(tabView)
        }

        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 8),
            stackView.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: 8),
            stackView.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -8),
            stackView.bottomAnchor.constraint(equalTo: containerView.bottomAnchor, constant: -8)
        ])
    }

    private func createUIKitTabItem(item: TabBarItem, index: Int, config: TabBarConfig) -> UIView {
        let containerView = UIView()
        containerView.translatesAutoresizingMaskIntoConstraints = false

        let button = UIButton(type: .custom)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.tag = index

        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.alignment = .center
        stackView.spacing = 4
        stackView.translatesAutoresizingMaskIntoConstraints = false
        stackView.isUserInteractionEnabled = false

        let isSelected = (index == selectedIndex)

        if !item.icon_path.toString().isEmpty {
            let iconView = createUIKitIcon(item: item, isSelected: isSelected)
            stackView.addArrangedSubview(iconView)
        }

        if !item.text.toString().isEmpty {
            let textLabel = UILabel()
            textLabel.text = item.text.toString()
            textLabel.font = UIFont.systemFont(ofSize: 10, weight: .medium)
            // Use config colors instead of system colors for better visibility
            textLabel.textColor = isSelected ?
                PlatformColor(argb: config.selected_color) :
                PlatformColor(argb: config.color)
            textLabel.textAlignment = .center
            textLabel.translatesAutoresizingMaskIntoConstraints = false
            stackView.addArrangedSubview(textLabel)
        }

        button.addSubview(stackView)
        containerView.addSubview(button)
        button.addTarget(self, action: #selector(uikitTabButtonTapped(_:)), for: .touchUpInside)

        NSLayoutConstraint.activate([
            stackView.centerXAnchor.constraint(equalTo: button.centerXAnchor),
            stackView.centerYAnchor.constraint(equalTo: button.centerYAnchor),
            stackView.leadingAnchor.constraint(greaterThanOrEqualTo: button.leadingAnchor, constant: 8),
            stackView.trailingAnchor.constraint(lessThanOrEqualTo: button.trailingAnchor, constant: -8),

            button.topAnchor.constraint(equalTo: containerView.topAnchor),
            button.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            button.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            button.bottomAnchor.constraint(equalTo: containerView.bottomAnchor),
            button.heightAnchor.constraint(equalToConstant: 60),
            button.widthAnchor.constraint(equalToConstant: 60)
        ])

        return containerView
    }

    private func createUIKitIcon(item: TabBarItem, isSelected: Bool) -> UIImageView {
        let iconView = UIImageView()
        iconView.contentMode = .scaleAspectFit
        iconView.translatesAutoresizingMaskIntoConstraints = false

        let iconPath = isSelected && !item.selected_icon_path.toString().isEmpty
            ? item.selected_icon_path.toString()
            : item.icon_path.toString()

        let iconColor = isSelected ? UIColor.systemBlue : UIColor.secondaryLabel

        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            iconView.image = UIImage(systemName: symbolName)
            iconView.tintColor = iconColor
        } else {
            if let bundleImage = UIImage(named: iconPath) {
                iconView.image = bundleImage
                iconView.tintColor = iconColor
            } else {
                iconView.image = UIImage(systemName: "circle.fill")
                iconView.tintColor = iconColor
            }
        }

        NSLayoutConstraint.activate([
            iconView.widthAnchor.constraint(equalToConstant: 24),
            iconView.heightAnchor.constraint(equalToConstant: 24)
        ])

        return iconView
    }

    @objc private func uikitTabButtonTapped(_ sender: UIButton) {
        let index = sender.tag
        selectedIndex = index

        if let config = tabBarConfig {
            let items = config.getItems(appId: appId)
            if index < items.count {
                let path = items[index].page_path.toString()
                onTabSelectedCallback?(index, path)
                updateUIKitSelection()
            }
        }
    }

    private func updateUIKitSelection() {
        if let config = tabBarConfig {
            let items = config.getItems(appId: appId)
            setupUIKitLayout(items: items, config: config)
        }
    }

}

public typealias LingXiaTabBar = iOSTabBarWrapper
public typealias PlatformTabBar = iOSTabBarWrapper
#elseif os(macOS)
import AppKit
import SwiftUI

/// NSView wrapper for SwiftUI LxAppTabBar on macOS
@MainActor
public class macOSTabBarWrapper: NSView, TabBarProtocol, ObservableObject {
    private var hostingController: NSHostingController<AnyView>?
    private var tabBarConfig: TabBarConfig?
    private var appId: String = ""
    @Published private var selectedIndex: Int = 0
    private var onTabSelectedCallback: ((Int, String) -> Void)?

    public var config: TabBarConfig? {
        return tabBarConfig
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupView()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }

    private func setupView() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
    }

    public func setConfig(config: TabBarConfig, appId: String) {
        self.tabBarConfig = config
        self.appId = appId
        updateSwiftUIView()
    }

    public func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        self.onTabSelectedCallback = listener
    }

    public func findTabIndexByPath(_ path: String) -> Int {
        guard let config = tabBarConfig else { return -1 }
        let items = config.getItems(appId: appId)
        for (index, item) in items.enumerated() {
            if item.page_path.toString() == path {
                return index
            }
        }
        return -1
    }

    public func syncSelectedTabWithCurrentPath(_ currentPath: String) {
        let index = findTabIndexByPath(currentPath)
        if index >= 0 {
            selectTab(index: index)
        }
    }

    public func selectTab(index: Int) {
        setSelectedIndex(index, notifyListener: false)
    }

    public func setSelectedIndex(_ index: Int, notifyListener: Bool) {
        selectedIndex = index
        updateSwiftUIView()

        if notifyListener, let callback = onTabSelectedCallback, let config = tabBarConfig {
            let items = config.getItems(appId: appId)
            if index < items.count {
                callback(index, items[index].page_path.toString())
            }
        }
    }

    private func updateSwiftUIView() {
        guard let config = tabBarConfig else { return }

        // Remove existing hosting controller
        if let existingController = hostingController {
            existingController.view.removeFromSuperview()
            existingController.removeFromParent()
        }

        // Create wrapper view that can observe changes
        let wrapperView = TabBarWrapperView(
            wrapper: self,
            appId: appId,
            config: config
        )

        // Create hosting controller
        let controller = NSHostingController(rootView: AnyView(wrapperView))
        hostingController = controller

        // Add to view hierarchy
        addSubview(controller.view)
        controller.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            controller.view.topAnchor.constraint(equalTo: topAnchor),
            controller.view.leadingAnchor.constraint(equalTo: leadingAnchor),
            controller.view.trailingAnchor.constraint(equalTo: trailingAnchor),
            controller.view.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }

    // Helper SwiftUI view that observes the wrapper
    private struct TabBarWrapperView: View {
        @ObservedObject var wrapper: macOSTabBarWrapper
        let appId: String
        let config: TabBarConfig

        var body: some View {
            LxAppTabBar(
                appId: appId,
                config: config,
                selectedIndex: $wrapper.selectedIndex
            ) { index, path in
                wrapper.setSelectedIndex(index, notifyListener: true)
            }
        }
    }
}

public typealias LingXiaTabBar = macOSTabBarWrapper
public typealias PlatformTabBar = macOSTabBarWrapper
#endif

