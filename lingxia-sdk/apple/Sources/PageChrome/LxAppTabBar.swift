import SwiftUI
import Foundation
import CLingXiaRustAPI
import os.log

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

extension Notification.Name {
    static let tabBarStateChanged = Notification.Name("TabBarDataChanged")
    static let navBarStateChanged = Notification.Name("NavBarDataChanged")
    #if os(macOS)
    static let sidebarNeedsRefresh = Notification.Name("SidebarNeedsRefresh")
    #endif
}

/// Extensions for TabBar
extension TabBar {
    var positionEnum: TabBarPosition {
        switch position {
        case 1: return .left
        case 2: return .right
        default: return .bottom
        }
    }

    func getItems(appId: String) -> [TabBarItem] {
        var items: [TabBarItem] = []
        for i in 0..<items_count {
            if let item = getTabBarItem(appId, i) {
                items.append(item)
            }
        }
        return items
    }
}

enum TabBarPosition {
    case bottom, left, right
}

// Shared TabBar Helper Functions
fileprivate struct TabBarHelpers {
    @ViewBuilder
    static func buildBadge(text: String) -> some View {
        Text(text)
            .font(.system(size: 10, weight: .medium))
            .foregroundColor(.white)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Capsule().fill(lxBadgeRed))
            .zIndex(1)
    }

    @ViewBuilder
    static func buildRedDot() -> some View {
        Circle()
            .fill(lxBadgeRed)
            .frame(width: 8, height: 8)
            .zIndex(1)
    }

    @ViewBuilder
    static func buildBorder(
        position: TabBarPosition,
        color: Color,
        thickness: CGFloat,
        visible: Bool
    ) -> some View {
        if visible {
            switch position {
            case .bottom:
                VStack(spacing: 0) {
                    Rectangle().fill(color).frame(height: thickness)
                    Spacer(minLength: 0)
                }
            case .left:
                HStack(spacing: 0) {
                    Spacer(minLength: 0)
                    Rectangle().fill(color).frame(width: thickness)
                }
            case .right:
                HStack(spacing: 0) {
                    Rectangle().fill(color).frame(width: thickness)
                    Spacer(minLength: 0)
                }
            }
        }
    }
}

/// Extensions for TabBarItem
extension TabBarItem {
    var cachedPagePath: String { page_path.toString() }
    var cachedText: String { text.toString() }
    var cachedIconPath: String { icon_path.toString() }
    var cachedSelectedIconPath: String { selected_icon_path.toString() }
}

/// TabBar styling helpers
struct TabBarHelper {
    static func isTransparent(_ colorValue: UInt32) -> Bool {
        return (colorValue >> 24) & 0xFF == 0
    }
}

/// Manifest tab-bar colors are optional. Keep declared colors exact, while
/// resolving omitted fields through platform semantic colors so native chrome
/// follows the host appearance just like the page WebView does.
extension TabBar {
    enum StyleField: UInt32 {
        case color = 1
        case selectedColor = 2
        case backgroundColor = 4
        case borderStyle = 8
    }

    private func styledMask(for colorScheme: ColorScheme) -> UInt32 {
        colorScheme == .dark ? dark_styled_mask : light_styled_mask
    }

    private func colorValue(selected: Bool, for colorScheme: ColorScheme) -> UInt32 {
        if colorScheme == .dark {
            return selected ? dark_selected_color : dark_color
        }
        return selected ? light_selected_color : light_color
    }

    func declares(_ field: StyleField, for colorScheme: ColorScheme) -> Bool {
        styledMask(for: colorScheme) & field.rawValue != 0
    }

    func itemColor(selected: Bool, for colorScheme: ColorScheme) -> PlatformColor {
        let field: StyleField = selected ? .selectedColor : .color
        if declares(field, for: colorScheme) {
            return PlatformColor(argb: colorValue(selected: selected, for: colorScheme))
        }
        #if os(iOS)
        return selected ? .systemBlue : .secondaryLabel
        #else
        return selected ? .systemBlue : .secondaryLabelColor
        #endif
    }

    func resolvedBackgroundValue(for colorScheme: ColorScheme) -> UInt32 {
        if colorScheme == .dark {
            return dark_background_color
        }
        return light_background_color
    }

    func resolvedBackgroundColor(for colorScheme: ColorScheme) -> PlatformColor {
        if declares(.backgroundColor, for: colorScheme) {
            return PlatformColor(argb: resolvedBackgroundValue(for: colorScheme))
        }
        #if os(iOS)
        return .systemBackground
        #else
        return .windowBackgroundColor
        #endif
    }

    private func resolvedBorderValue(for colorScheme: ColorScheme) -> UInt32 {
        colorScheme == .dark ? dark_border_style : light_border_style
    }

    func resolvedBorderColor(for colorScheme: ColorScheme) -> PlatformColor {
        if declares(.borderStyle, for: colorScheme) {
            return PlatformColor(argb: resolvedBorderValue(for: colorScheme))
        }
        #if os(iOS)
        return .separator
        #else
        return .separatorColor
        #endif
    }

    func drawsBorder(for colorScheme: ColorScheme) -> Bool {
        (resolvedBackgroundValue(for: colorScheme) >> 24) & 0xFF != 0
    }
}

/// Unified SwiftUI TabBar for iOS and macOS
/// Badge / red-dot red, unified across iOS, Android, and Harmony (#FA5151).
let lxBadgeRed = Color(red: 0xFA / 255.0, green: 0x51 / 255.0, blue: 0x51 / 255.0)

struct LxAppTabBar: View {
    let appId: String
    let config: TabBar
    @Binding var selectedIndex: Int
    let onTabSelected: (Int, String) -> Void
    // Simple refresh trigger for UI updates
    @State private var refreshTrigger = false
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.displayScale) private var displayScale

    init(
        appId: String,
        config: TabBar,
        selectedIndex: Binding<Int>,
        onTabSelected: @escaping (Int, String) -> Void
    ) {
        self.appId = appId
        self.config = config
        self._selectedIndex = selectedIndex
        self.onTabSelected = onTabSelected
    }

    var body: some View {
        // Get fresh data from Rust every time body is called
        let items = config.getItems(appId: appId)

        Group {
            switch config.positionEnum {
            case .bottom:
                buildHorizontalTabBar(items: items)
                    .frame(height: config.dimensionPoints)

            case .left, .right:
                buildVerticalTabBar(items: items)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(getTabBarBackgroundColor())
        .overlay {
            TabBarHelpers.buildBorder(
                position: config.positionEnum,
                color: Color(config.resolvedBorderColor(for: colorScheme)),
                thickness: 1 / max(displayScale, 1),
                visible: config.drawsBorder(for: colorScheme)
            )
            .allowsHitTesting(false)
        }
        .id("tabbar-\(selectedIndex)-\(refreshTrigger)")
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
    private func buildTabItem(item: TabBarItem, index: Int) -> some View {
        let isSelected = (index == selectedIndex)
        // Get state directly from Rust
        let rustItem = getTabBarItem(appId, Int32(index))

        let forceColor = Color(config.itemColor(selected: isSelected, for: colorScheme))

        Button(action: {
            // Always trigger callback - let parent decide if action is needed
            onTabSelected(index, item.cachedPagePath)
        }) {
            VStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                // Tab icon with badge and red dot overlay
                ZStack {
                    if !item.cachedIconPath.isEmpty {
                        buildTabIcon(item: item, isSelected: isSelected, forceColor: forceColor)
                    }

                    // Badge overlay (from Rust state)
                    if let rustItem = rustItem, !rustItem.badge.toString().isEmpty {
                        TabBarHelpers.buildBadge(text: rustItem.badge.toString())
                            .offset(x: 16, y: -6)
                    }
                    // Red dot overlay (only show if no badge)
                    else if let rustItem = rustItem, rustItem.has_red_dot {
                        TabBarHelpers.buildRedDot()
                            .offset(x: 16, y: -4)
                    }
                }

                // Tab title
                if !item.cachedText.isEmpty {
                    Text(item.cachedText)
                        .font(LxAppTheme.Typography.tabTitle)
                        .foregroundColor(forceColor)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, LxAppTheme.Metrics.smallSpacing)
        }
        .buttonStyle(PlainButtonStyle())
    }

    @ViewBuilder
    private func buildTabIcon(item: TabBarItem, isSelected: Bool, forceColor: Color) -> some View {
        let iconPath = isSelected && !item.cachedSelectedIconPath.isEmpty
            ? item.cachedSelectedIconPath
            : item.cachedIconPath

        let iconColor = forceColor

        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            Image(systemName: symbolName)
                .font(.system(size: LxAppTheme.Metrics.tabIconSize))
                .foregroundColor(iconColor)
        } else if iconPath.hasPrefix("/") {
            if let image = loadPlatformImage(from: iconPath) {
                image
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            }
        } else {
            if let bundleImage = loadBundleImage(named: iconPath) {
                bundleImage
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            } else {
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
        Color(config.resolvedBackgroundColor(for: colorScheme))
    }
}

/// macOS TabBar that accepts external state manager
struct MacOSLxAppTabBar: View {
    let appId: String
    let config: TabBar
    @Binding var selectedIndex: Int
    let onTabSelected: (Int, String) -> Void
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.displayScale) private var displayScale

    init(
        appId: String,
        config: TabBar,
        selectedIndex: Binding<Int>,
        onTabSelected: @escaping (Int, String) -> Void
    ) {
        self.appId = appId
        self.config = config
        self._selectedIndex = selectedIndex
        self.onTabSelected = onTabSelected
    }

    var body: some View {
        let items = config.getItems(appId: appId)

        Group {
            switch config.positionEnum {
            case .bottom:
                buildHorizontalTabBar(items: items)
                    .frame(height: config.dimensionPoints)

            case .left, .right:
                buildVerticalTabBar(items: items)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(getTabBarBackgroundColor())
        .overlay {
            TabBarHelpers.buildBorder(
                position: config.positionEnum,
                color: Color(config.resolvedBorderColor(for: colorScheme)),
                thickness: 1 / max(displayScale, 1),
                visible: config.drawsBorder(for: colorScheme)
            )
            .allowsHitTesting(false)
        }
    }

    @ViewBuilder
    private func buildTabItem(item: TabBarItem, index: Int) -> some View {
        let isSelected = (index == selectedIndex)
        // Get state directly from Rust
        let rustItem = getTabBarItem(appId, Int32(index))

        let forceColor = Color(config.itemColor(selected: isSelected, for: colorScheme))

        Button(action: {
            // Always trigger callback - let parent decide if action is needed
            onTabSelected(index, item.cachedPagePath)
        }) {
            VStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                // Tab icon with badge and red dot overlay
                ZStack {
                    if !item.cachedIconPath.isEmpty {
                        buildTabIcon(item: item, isSelected: isSelected, forceColor: forceColor)
                    }

                    // Badge overlay (from Rust state)
                    if let rustItem = rustItem, !rustItem.badge.toString().isEmpty {
                        TabBarHelpers.buildBadge(text: rustItem.badge.toString())
                            .offset(x: 16, y: -6)
                    }
                    // Red dot overlay (only show if no badge)
                    else if let rustItem = rustItem, rustItem.has_red_dot {
                        TabBarHelpers.buildRedDot()
                            .offset(x: 16, y: -4)
                    }
                }

                // Tab title
                if !item.cachedText.isEmpty {
                    Text(item.cachedText)
                        .font(LxAppTheme.Typography.tabTitle)
                        .foregroundColor(forceColor)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.vertical, LxAppTheme.Metrics.smallSpacing)
            .contentShape(Rectangle())
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .contentShape(Rectangle())
        .buttonStyle(PlainButtonStyle())
    }

    @ViewBuilder
    private func buildHorizontalTabBar(items: [TabBarItem]) -> some View {
        HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                buildTabItem(item: item, index: index)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(.horizontal, LxAppTheme.Metrics.largeSpacing)
        .contentShape(Rectangle())
    }

    @ViewBuilder
    private func buildVerticalTabBar(items: [TabBarItem]) -> some View {
        VStack(spacing: LxAppTheme.Metrics.standardSpacing) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                buildTabItem(item: item, index: index)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(.vertical, LxAppTheme.Metrics.largeSpacing)
        .contentShape(Rectangle())
    }

    @ViewBuilder
    private func buildTabIcon(item: TabBarItem, isSelected: Bool, forceColor: Color) -> some View {
        let iconPath = isSelected && !item.cachedSelectedIconPath.isEmpty
            ? item.cachedSelectedIconPath
            : item.cachedIconPath

        let iconColor = forceColor

        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            Image(systemName: symbolName)
                .font(.system(size: LxAppTheme.Metrics.tabIconSize))
                .foregroundColor(iconColor)
        } else if iconPath.hasPrefix("/") {
            if let image = loadPlatformImage(from: iconPath) {
                image
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            }
        } else {
            if let bundleImage = loadBundleImage(named: iconPath) {
                bundleImage
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            } else {
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
        Color(config.resolvedBackgroundColor(for: colorScheme))
    }
}

/// Protocol for tab bar implementations
extension TabBar {
    /// Configured bar thickness in points (height when horizontal, width when
    /// vertical). Rust guarantees a positive default; the theme constant is a
    /// last-resort fallback so a malformed config can't collapse the bar.
    var dimensionPoints: CGFloat {
        dimension > 0 ? CGFloat(dimension) : LxAppTheme.Metrics.tabBarHeight
    }
}

@MainActor
protocol TabBarProtocol: AnyObject {
    var config: TabBar? { get }
    var appId: String { get set }
    func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void)
    func setSelectedIndex(_ index: Int, notifyListener: Bool)
    func refreshLayout()
}

#if os(iOS)
import UIKit

/// UIKit TabBar implementation for iOS
@MainActor
class iOSTabBarWrapper: UIView, TabBarProtocol {
    private var tabBarConfig: TabBar?
    var appId: String = ""
    private var selectedIndex: Int = 0
    private var onTabSelectedCallback: ((Int, String) -> Void)?

    // Public accessor for tabBarConfig
    var config: TabBar? {
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

    override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        guard previousTraitCollection?.userInterfaceStyle != traitCollection.userInterfaceStyle,
              tabBarConfig != nil else { return }
        refreshLayout()
    }

    private func setupView() {
        backgroundColor = UIColor.clear
    }

    func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        self.onTabSelectedCallback = listener
    }

    /// Initialize TabBar with config and appId
    func initialize(config: TabBar, appId: String) {
        self.tabBarConfig = config
        self.appId = appId

        // Initialize local selection from Rust state so UI reflects correct tab on first render
        self.selectedIndex = Int(config.selected_index)
        refreshLayout()
    }

    func setSelectedIndex(_ index: Int, notifyListener: Bool) {
        let previousIndex = Int(tabBarConfig?.selected_index ?? 0)
        self.selectedIndex = index

        if previousIndex != index {
            refreshLayout()
        }

        if notifyListener, let callback = onTabSelectedCallback, let config = tabBarConfig {
            let items = config.getItems(appId: appId)
            if index < items.count {
                callback(index, items[index].page_path.toString())
            }
        }
    }

    func refreshLayout() {
        // Get fresh config from Rust instead of using cached tabBarConfig
        guard let freshConfig = getTabBar(appId) else {
            // If no config exists, hide the view.
            self.isHidden = true
            return
        }

        // Update cached config with fresh data
        self.tabBarConfig = freshConfig

        // Update selected index from fresh config
        self.selectedIndex = Int(freshConfig.selected_index)

        // Apply background color from config
        let colorScheme: ColorScheme = traitCollection.userInterfaceStyle == .dark ? .dark : .light
        let bgColor = freshConfig.resolvedBackgroundColor(for: colorScheme)
        self.backgroundColor = bgColor
        self.layer.backgroundColor = bgColor.cgColor
        self.isOpaque = ((freshConfig.resolvedBackgroundValue(for: colorScheme) >> 24) & 0xFF) == 0xFF

        let items = freshConfig.getItems(appId: appId)

        // Always recreate layout to ensure fresh badge/red dot data
        setupUIKitLayout(items: items, config: freshConfig)

        // Apply visibility state
        self.isHidden = !freshConfig.is_visible
        self.alpha = freshConfig.is_visible ? 1.0 : 0.0
    }

    private func createRedDotView() -> UIView {
        let redDot = UIView()
        redDot.backgroundColor = UIColor(red: 0xFA / 255.0, green: 0x51 / 255.0, blue: 0x51 / 255.0, alpha: 1.0)
        redDot.layer.cornerRadius = 4
        redDot.translatesAutoresizingMaskIntoConstraints = false
        return redDot
    }

    private func setupUIKitLayout(items: [TabBarItem], config: TabBar) {
        subviews.forEach { $0.removeFromSuperview() }

        let containerView = UIView()
        // Keep container clear so parent background shows through
        containerView.backgroundColor = UIColor.clear
        containerView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(containerView)

        let isVertical = config.position == 1 || config.position == 2

        if isVertical {
            setupVerticalLayout(items: items, config: config, containerView: containerView)
        } else {
            setupHorizontalLayout(items: items, config: config, containerView: containerView)
        }

        NSLayoutConstraint.activate([
            containerView.topAnchor.constraint(equalTo: topAnchor),
            containerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            containerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            containerView.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])

        let colorScheme: ColorScheme = traitCollection.userInterfaceStyle == .dark ? .dark : .light
        addUIKitBorder(config: config, colorScheme: colorScheme)
    }

    private func addUIKitBorder(config: TabBar, colorScheme: ColorScheme) {
        guard config.drawsBorder(for: colorScheme) else { return }

        let border = UIView()
        border.backgroundColor = config.resolvedBorderColor(for: colorScheme)
        border.isUserInteractionEnabled = false
        border.translatesAutoresizingMaskIntoConstraints = false
        addSubview(border)

        let thickness = 1 / max(traitCollection.displayScale, 1)
        switch config.positionEnum {
        case .bottom:
            NSLayoutConstraint.activate([
                border.topAnchor.constraint(equalTo: topAnchor),
                border.leadingAnchor.constraint(equalTo: leadingAnchor),
                border.trailingAnchor.constraint(equalTo: trailingAnchor),
                border.heightAnchor.constraint(equalToConstant: thickness)
            ])
        case .left:
            NSLayoutConstraint.activate([
                border.topAnchor.constraint(equalTo: topAnchor),
                border.trailingAnchor.constraint(equalTo: trailingAnchor),
                border.bottomAnchor.constraint(equalTo: bottomAnchor),
                border.widthAnchor.constraint(equalToConstant: thickness)
            ])
        case .right:
            NSLayoutConstraint.activate([
                border.topAnchor.constraint(equalTo: topAnchor),
                border.leadingAnchor.constraint(equalTo: leadingAnchor),
                border.bottomAnchor.constraint(equalTo: bottomAnchor),
                border.widthAnchor.constraint(equalToConstant: thickness)
            ])
        }
    }

    private func setupVerticalLayout(items: [TabBarItem], config: TabBar, containerView: UIView) {
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

    private func setupHorizontalLayout(items: [TabBarItem], config: TabBar, containerView: UIView) {
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

    private func createUIKitTabItem(item: TabBarItem, index: Int, config: TabBar) -> UIView {
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
        let colorScheme: ColorScheme = traitCollection.userInterfaceStyle == .dark ? .dark : .light

        if !item.icon_path.toString().isEmpty {
            let iconView = createUIKitIcon(
                item: item,
                index: index,
                isSelected: isSelected,
                color: config.itemColor(selected: isSelected, for: colorScheme)
            )
            stackView.addArrangedSubview(iconView)
        }

        if !item.text.toString().isEmpty {
            let textLabel = UILabel()
            textLabel.text = item.text.toString()
            textLabel.font = UIFont.systemFont(ofSize: 10, weight: .medium)
            // Use config colors instead of system colors for better visibility
            textLabel.textColor = config.itemColor(selected: isSelected, for: colorScheme)
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

    private func createUIKitIcon(
        item: TabBarItem,
        index: Int,
        isSelected: Bool,
        color: UIColor
    ) -> UIView {
        // Create container view for icon + badge/red dot
        let iconContainer = UIView()
        iconContainer.translatesAutoresizingMaskIntoConstraints = false

        let iconView = UIImageView()
        iconView.contentMode = .scaleAspectFit
        iconView.translatesAutoresizingMaskIntoConstraints = false

        let iconPath = isSelected && !item.selected_icon_path.toString().isEmpty
            ? item.selected_icon_path.toString()
            : item.icon_path.toString()

        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            iconView.image = UIImage(systemName: symbolName)
            iconView.tintColor = color
        } else {
            if let bundleImage = UIImage(named: iconPath) {
                iconView.image = bundleImage
                iconView.tintColor = color
            } else {
                iconView.image = UIImage(systemName: "circle.fill")
                iconView.tintColor = color
            }
        }

        iconContainer.addSubview(iconView)

        // Get badge and red dot data from Rust
        if let rustItem = getTabBarItem(appId, Int32(index)) {
            let badgeText = rustItem.badge.toString()
            let hasRedDot = rustItem.has_red_dot

            // Add badge if present
            if !badgeText.isEmpty {
                let badgeView = createBadgeView(text: badgeText)
                iconContainer.addSubview(badgeView)

                NSLayoutConstraint.activate([
                    badgeView.topAnchor.constraint(equalTo: iconContainer.topAnchor, constant: -6),
                    badgeView.trailingAnchor.constraint(equalTo: iconContainer.trailingAnchor, constant: 4)
                ])
            }
            // Add red dot if no badge and red dot is enabled
            else if hasRedDot {
                let redDotView = createRedDotView()
                iconContainer.addSubview(redDotView)

                NSLayoutConstraint.activate([
                    redDotView.topAnchor.constraint(equalTo: iconContainer.topAnchor, constant: -4),
                    redDotView.trailingAnchor.constraint(equalTo: iconContainer.trailingAnchor, constant: 4),
                    redDotView.widthAnchor.constraint(equalToConstant: 8),
                    redDotView.heightAnchor.constraint(equalToConstant: 8)
                ])
            }
        }

        NSLayoutConstraint.activate([
            iconContainer.widthAnchor.constraint(equalToConstant: 32),
            iconContainer.heightAnchor.constraint(equalToConstant: 32),

            iconView.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
            iconView.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 24),
            iconView.heightAnchor.constraint(equalToConstant: 24)
        ])

        return iconContainer
    }

    @objc private func uikitTabButtonTapped(_ sender: UIButton) {
        let index = sender.tag
        // Update local UI selection immediately, and notify listener (which routes to Rust)
        setSelectedIndex(index, notifyListener: true)
    }

    private func createBadgeView(text: String) -> UIView {
        let badgeView = UIView()
        badgeView.backgroundColor = UIColor(red: 0xFA / 255.0, green: 0x51 / 255.0, blue: 0x51 / 255.0, alpha: 1.0)
        badgeView.layer.cornerRadius = 8
        badgeView.translatesAutoresizingMaskIntoConstraints = false

        let badgeLabel = UILabel()
        badgeLabel.text = text
        badgeLabel.textColor = UIColor.white
        badgeLabel.font = UIFont.systemFont(ofSize: 10, weight: .medium)
        badgeLabel.textAlignment = .center
        badgeLabel.translatesAutoresizingMaskIntoConstraints = false

        badgeView.addSubview(badgeLabel)

        NSLayoutConstraint.activate([
            badgeLabel.centerXAnchor.constraint(equalTo: badgeView.centerXAnchor),
            badgeLabel.centerYAnchor.constraint(equalTo: badgeView.centerYAnchor),
            badgeLabel.leadingAnchor.constraint(greaterThanOrEqualTo: badgeView.leadingAnchor, constant: 4),
            badgeLabel.trailingAnchor.constraint(lessThanOrEqualTo: badgeView.trailingAnchor, constant: -4),

            badgeView.widthAnchor.constraint(greaterThanOrEqualToConstant: 16),
            badgeView.heightAnchor.constraint(equalToConstant: 16)
        ])

        return badgeView
    }
}

typealias LingXiaTabBar = iOSTabBarWrapper
#elseif os(macOS)
import AppKit
import SwiftUI

/// NSView wrapper for SwiftUI LxAppTabBar on macOS
@MainActor
class macOSTabBarWrapper: NSView, TabBarProtocol, ObservableObject {
    private var hostingController: NSHostingController<AnyView>?
    private var tabBarConfig: TabBar?
    var appId: String = ""
    @Published private var selectedIndex: Int = 0
    private var onTabSelectedCallback: ((Int, String) -> Void)?

    var config: TabBar? {
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

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let config = tabBarConfig,
              config.is_visible,
              !isHidden,
              alphaValue > 0.01,
              frame.contains(point) else {
            return nil
        }

        return super.hitTest(point) ?? self
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func mouseDown(with event: NSEvent) {}

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        // Rebuild the SwiftUI root so semantic default colors resolve against
        // a Runner-pinned appearance immediately, without a page-side JS API.
        guard tabBarConfig != nil else { return }
        refreshLayout()
    }

    private func setupView() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
    }

    func setOnTabSelectedListener(_ listener: @escaping (Int, String) -> Void) {
        self.onTabSelectedCallback = listener
    }

    /// Initialize TabBar with config and appId
    func initialize(config: TabBar, appId: String) {
        self.tabBarConfig = config
        self.appId = appId

        // Initialize local selection from Rust state so UI reflects correct tab on first render
        self.selectedIndex = Int(config.selected_index)
        refreshLayout()
    }

    func setSelectedIndex(_ index: Int, notifyListener: Bool) {
        let previousIndex = self.selectedIndex
        // Update local selectedIndex to reflect Rust state
        selectedIndex = index

        if previousIndex != index {
            refreshLayout()
        }

        if notifyListener, let callback = onTabSelectedCallback, let config = tabBarConfig {
            let items = config.getItems(appId: appId)
            if index < items.count {
                callback(index, items[index].page_path.toString())
            }
        }
    }

    func refreshLayout() {
        // Get fresh config from Rust instead of using cached tabBarConfig
        guard let freshConfig = getTabBar(appId) else {
            // If no config exists, hide the view.
            self.isHidden = true
            return
        }

        // Update cached config with fresh data
        self.tabBarConfig = freshConfig

        // Update selected index from fresh config
        self.selectedIndex = Int(freshConfig.selected_index)

        // Always recreate layout to ensure fresh badge/red dot data
        updateSwiftUIView()

        // Apply visibility state
        self.isHidden = !freshConfig.is_visible
        self.alphaValue = freshConfig.is_visible ? 1.0 : 0.0
    }

    private func updateSwiftUIView() {
        guard let config = tabBarConfig else { return }

        let wrapperView = TabBarWrapperView(
            wrapper: self,
            appId: appId,
            config: config
        )

        if let existingController = hostingController {
            // Update existing controller's root view instead of recreating
            existingController.rootView = AnyView(wrapperView)
            return
        }

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
        let config: TabBar

        var body: some View {
            MacOSLxAppTabBar(
                appId: appId,
                config: config,
                selectedIndex: $wrapper.selectedIndex
            ) { index, path in
                wrapper.setSelectedIndex(index, notifyListener: true)
            }
        }
    }
}

typealias LingXiaTabBar = macOSTabBarWrapper
#endif
