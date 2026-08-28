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
}

/// Extensions for TabBarItem
extension TabBarItem {
    var cachedPagePath: String { page_path.toString() }
    var cachedText: String { text.toString() }
    var cachedIconPath: String { icon_path.toString() }
    var cachedSelectedIconPath: String { selected_icon_path.toString() }
}

/// TabBar styling helpers
/// Circle drawn behind the icon of a selected item that ships only one icon,
/// standing in for the selected artwork it does not have.
enum TabBarMetrics {
    static let activeIndicatorSize: CGFloat = 36
    static let activeIndicatorOpacity: CGFloat = 0.2
}

struct TabBarHelper {
    static func isTransparent(_ colorValue: UInt32) -> Bool {
        return (colorValue >> 24) & 0xFF == 0
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

        let forceColor = isSelected ?
            Color(PlatformColor(argb: config.selected_color)) :
            Color(PlatformColor(argb: config.color))

        Button(action: {
            // Always trigger callback - let parent decide if action is needed
            onTabSelected(index, item.cachedPagePath)
        }) {
            VStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                // Tab icon with badge and red dot overlay
                ZStack {
                    // A single-icon item has no swap to signal selection, so the
                    // strip draws an active indicator behind it instead.
                    if isSelected, let rustItem, !rustItem.has_selected_icon {
                        Circle()
                            .fill(Color(PlatformColor(argb: config.selected_color))
                                .opacity(TabBarMetrics.activeIndicatorOpacity))
                            .frame(
                                width: TabBarMetrics.activeIndicatorSize,
                                height: TabBarMetrics.activeIndicatorSize
                            )
                    }

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
                    .renderingMode(item.has_selected_icon ? .original : .template)
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            }
        } else {
            if let bundleImage = loadBundleImage(named: iconPath) {
                bundleImage
                    .renderingMode(item.has_selected_icon ? .original : .template)
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
        let platformColor = PlatformColor(argb: config.background_color)
        return Color(platformColor)
    }
}

/// macOS TabBar that accepts external state manager
struct MacOSLxAppTabBar: View {
    private enum Overflow {
        static let columns = 5
        static let cellWidth: CGFloat = 64
    }

    let appId: String
    let config: TabBar
    @Binding var selectedIndex: Int
    let onTabSelected: (Int, String) -> Void
    /// A phone-shaped host, where the strip has room for five slots. Roomier
    /// hosts (desktop, tablet) lay every item out and never fold.
    let compact: Bool
    /// Ask the host to present the folded items. The panel belongs to the host
    /// view so it can sit above the strip inside the simulated screen, rather
    /// than floating out of the window as a desktop popover would.
    let onMoreRequested: () -> Void
    /// When set, render these item indices as the overflow grid instead of the
    /// strip. The host uses this for the panel above the bar.
    let overflowGrid: [Int]?

    init(
        appId: String,
        config: TabBar,
        selectedIndex: Binding<Int>,
        compact: Bool = true,
        overflowGrid: [Int]? = nil,
        onMoreRequested: @escaping () -> Void = {},
        onTabSelected: @escaping (Int, String) -> Void
    ) {
        self.appId = appId
        self.config = config
        self._selectedIndex = selectedIndex
        self.compact = compact
        self.overflowGrid = overflowGrid
        self.onMoreRequested = onMoreRequested
        self.onTabSelected = onTabSelected
    }

    var body: some View {
        let items = config.getItems(appId: appId)

        Group {
            if let overflowGrid {
                buildOverflowGrid(items: items, indices: overflowGrid)
            } else {
                switch config.positionEnum {
                case .bottom:
                    buildHorizontalTabBar(items: items)
                        .frame(height: config.dimensionPoints)

                case .left, .right:
                    buildVerticalTabBar(items: items)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .background(getTabBarBackgroundColor())
    }

    @ViewBuilder
    private func buildTabItem(item: TabBarItem, index: Int) -> some View {
        let isSelected = (index == selectedIndex)
        // Get state directly from Rust
        let rustItem = getTabBarItem(appId, Int32(index))

        let forceColor = isSelected ?
            Color(PlatformColor(argb: config.selected_color)) :
            Color(PlatformColor(argb: config.color))

        Button(action: {
            // Always trigger callback - let parent decide if action is needed
            onTabSelected(index, item.cachedPagePath)
        }) {
            VStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                // Tab icon with badge and red dot overlay
                ZStack {
                    // A single-icon item has no swap to signal selection, so the
                    // strip draws an active indicator behind it instead.
                    if isSelected, let rustItem, !rustItem.has_selected_icon {
                        Circle()
                            .fill(Color(PlatformColor(argb: config.selected_color))
                                .opacity(TabBarMetrics.activeIndicatorOpacity))
                            .frame(
                                width: TabBarMetrics.activeIndicatorSize,
                                height: TabBarMetrics.activeIndicatorSize
                            )
                    }

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

    /// First folded item index, or -1 when every item has its own slot.
    private func overflowStart(itemCount: Int) -> Int {
        guard compact else { return -1 }
        let start = Int(config.overflow_start_index)
        return (start >= 0 && start < itemCount) ? start : -1
    }

    @ViewBuilder
    private func buildHorizontalTabBar(items: [TabBarItem]) -> some View {
        // Rust caps how many items a compact strip shows; past that the last
        // slot becomes the overflow affordance and stands in for the rest.
        let start = overflowStart(itemCount: items.count)
        let stripCount = start >= 0 ? start : items.count

        HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
            ForEach(0..<stripCount, id: \.self) { index in
                buildTabItem(item: items[index], index: index)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            if start >= 0 {
                buildMoreItem(items: items, overflowStart: start)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(.horizontal, LxAppTheme.Metrics.largeSpacing)
        .contentShape(Rectangle())
    }

    /// The overflow slot stands in for the folded items, selection included.
    @ViewBuilder
    private func buildMoreItem(items: [TabBarItem], overflowStart: Int) -> some View {
        let isSelected = selectedIndex >= overflowStart
        let forceColor = isSelected ?
            Color(PlatformColor(argb: config.selected_color)) :
            Color(PlatformColor(argb: config.color))

        Button(action: onMoreRequested) {
            VStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                ZStack {
                    Image(systemName: "ellipsis")
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .frame(width: 24, height: 24)
                        .foregroundColor(forceColor)

                    // Folded badges still have to surface, so "more" aggregates
                    // them to a dot.
                    if overflowHasNotification(from: overflowStart, itemCount: items.count) {
                        TabBarHelpers.buildRedDot().offset(x: 16, y: -4)
                    }
                }
                Text(L10n.string("lx_tabbar_more"))
                    .font(LxAppTheme.Typography.tabTitle)
                    .foregroundColor(forceColor)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.vertical, LxAppTheme.Metrics.smallSpacing)
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
    }

    private func overflowHasNotification(from start: Int, itemCount: Int) -> Bool {
        for index in start..<itemCount {
            guard let rustItem = getTabBarItem(appId, Int32(index)) else { continue }
            if rustItem.has_red_dot || !rustItem.badge.toString().isEmpty {
                return true
            }
        }
        return false
    }

    /// The folded items as a grid, built from the strip's own cells so the
    /// panel and the bar cannot drift apart. A short final row keeps the full
    /// column count, leaving the cells aligned.
    @ViewBuilder
    private func buildOverflowGrid(items: [TabBarItem], indices: [Int]) -> some View {
        let rows = stride(from: 0, to: indices.count, by: Overflow.columns).map { start in
            Array(indices[start..<min(start + Overflow.columns, indices.count)])
        }

        VStack(spacing: 0) {
            ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                HStack(spacing: 0) {
                    ForEach(row, id: \.self) { index in
                        buildTabItem(item: items[index], index: index)
                            .frame(width: Overflow.cellWidth, height: config.dimensionPoints)
                    }
                    ForEach(row.count..<Overflow.columns, id: \.self) { _ in
                        Color.clear.frame(width: Overflow.cellWidth, height: config.dimensionPoints)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity)
        .padding(LxAppTheme.Metrics.smallSpacing)
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
                    .renderingMode(item.has_selected_icon ? .original : .template)
                    .resizable()
                    .frame(width: LxAppTheme.Metrics.tabIconSize, height: LxAppTheme.Metrics.tabIconSize)
                    .foregroundColor(iconColor)
            }
        } else {
            if let bundleImage = loadBundleImage(named: iconPath) {
                bundleImage
                    .renderingMode(item.has_selected_icon ? .original : .template)
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
        let platformColor = PlatformColor(argb: config.background_color)
        return Color(platformColor)
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
    private weak var overflowPanel: LxAppTabBarOverflowPanel?

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
        let bgColor = PlatformColor(argb: freshConfig.background_color)
        self.backgroundColor = bgColor
        self.layer.backgroundColor = bgColor.cgColor
        self.isOpaque = ((freshConfig.background_color >> 24) & 0xFF) == 0xFF

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
    }

    private func setupVerticalLayout(items: [TabBarItem], config: TabBar, containerView: UIView) {
        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.distribution = .fillEqually
        stackView.alignment = .center
        stackView.spacing = 8
        stackView.translatesAutoresizingMaskIntoConstraints = false
        containerView.addSubview(stackView)

        // Rust caps how many items a compact strip shows; past that the last
        // slot becomes the overflow affordance and stands in for the rest.
        let overflowStart = overflowStart(itemCount: items.count, config: config)
        let stripCount = overflowStart >= 0 ? overflowStart : items.count
        for index in 0..<stripCount {
            let tabView = createUIKitTabItem(item: items[index], index: index, config: config)
            stackView.addArrangedSubview(tabView)
        }
        if overflowStart >= 0 {
            stackView.addArrangedSubview(createUIKitMoreItem(config: config, overflowStart: overflowStart))
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

        if !item.icon_path.toString().isEmpty {
            let iconView = createUIKitIcon(item: item, index: index, isSelected: isSelected)
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

    private func createUIKitIcon(item: TabBarItem, index: Int, isSelected: Bool) -> UIView {
        // Create container view for icon + badge/red dot
        let iconContainer = UIView()
        iconContainer.translatesAutoresizingMaskIntoConstraints = false

        let iconView = UIImageView()
        iconView.contentMode = .scaleAspectFit
        iconView.translatesAutoresizingMaskIntoConstraints = false

        let iconPath = isSelected && !item.selected_icon_path.toString().isEmpty
            ? item.selected_icon_path.toString()
            : item.icon_path.toString()

        // An item shipping an icon pair owns its artwork in both states; a
        // single icon is a template glyph the strip tints instead.
        let template = !item.has_selected_icon
        let iconColor = if template {
            isSelected
                ? PlatformColor(argb: tabBarConfig?.selected_color ?? 0)
                : PlatformColor(argb: tabBarConfig?.color ?? 0)
        } else {
            isSelected ? UIColor.systemBlue : UIColor.secondaryLabel
        }

        let render: (UIImage?) -> UIImage? = { image in
            template ? image?.withRenderingMode(.alwaysTemplate) : image
        }
        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            iconView.image = UIImage(systemName: symbolName)
            iconView.tintColor = iconColor
        } else {
            if let bundleImage = UIImage(named: iconPath) {
                iconView.image = render(bundleImage)
                iconView.tintColor = iconColor
            } else {
                iconView.image = UIImage(systemName: "circle.fill")
                iconView.tintColor = iconColor
            }
        }

        // A single-icon item has no swap to signal selection, so the strip
        // draws a Material-style active indicator behind it instead.
        if isSelected, !item.has_selected_icon {
            let indicator = UIView()
            indicator.backgroundColor = PlatformColor(argb: tabBarConfig?.selected_color ?? 0)
                .withAlphaComponent(TabBarMetrics.activeIndicatorOpacity)
            indicator.layer.cornerRadius = TabBarMetrics.activeIndicatorSize / 2
            indicator.translatesAutoresizingMaskIntoConstraints = false
            iconContainer.addSubview(indicator)
            NSLayoutConstraint.activate([
                indicator.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
                indicator.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
                indicator.widthAnchor.constraint(equalToConstant: TabBarMetrics.activeIndicatorSize),
                indicator.heightAnchor.constraint(equalToConstant: TabBarMetrics.activeIndicatorSize)
            ])
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

    /// First folded item index, or -1 when every item has its own slot.
    private func overflowStart(itemCount: Int, config: TabBar) -> Int {
        let start = Int(config.overflow_start_index)
        return (start >= 0 && start < itemCount) ? start : -1
    }

    /// The overflow slot stands in for the folded items, selection included.
    private func createUIKitMoreItem(config: TabBar, overflowStart: Int) -> UIView {
        let containerView = UIView()
        containerView.translatesAutoresizingMaskIntoConstraints = false

        let button = UIButton(type: .custom)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.addTarget(self, action: #selector(moreButtonTapped), for: .touchUpInside)

        let isSelected = selectedIndex >= overflowStart
        let tint = isSelected
            ? PlatformColor(argb: config.selected_color)
            : PlatformColor(argb: config.color)

        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.alignment = .center
        stackView.spacing = 4
        stackView.translatesAutoresizingMaskIntoConstraints = false
        stackView.isUserInteractionEnabled = false

        let iconContainer = UIView()
        iconContainer.translatesAutoresizingMaskIntoConstraints = false
        let iconView = UIImageView(image: UIImage(systemName: "ellipsis"))
        iconView.contentMode = .scaleAspectFit
        iconView.tintColor = tint
        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconContainer.addSubview(iconView)
        stackView.addArrangedSubview(iconContainer)

        // Folded badges still have to surface, so "more" aggregates them to a dot.
        if overflowHasNotification(from: overflowStart, config: config) {
            let dot = createRedDotView()
            iconContainer.addSubview(dot)
            NSLayoutConstraint.activate([
                dot.topAnchor.constraint(equalTo: iconContainer.topAnchor, constant: -4),
                dot.trailingAnchor.constraint(equalTo: iconContainer.trailingAnchor, constant: 4),
                dot.widthAnchor.constraint(equalToConstant: 8),
                dot.heightAnchor.constraint(equalToConstant: 8)
            ])
        }

        let textLabel = UILabel()
        textLabel.text = L10n.string("lx_tabbar_more")
        textLabel.font = UIFont.systemFont(ofSize: 10, weight: .medium)
        textLabel.textColor = tint
        textLabel.textAlignment = .center
        textLabel.translatesAutoresizingMaskIntoConstraints = false
        stackView.addArrangedSubview(textLabel)

        button.addSubview(stackView)
        containerView.addSubview(button)

        NSLayoutConstraint.activate([
            stackView.centerXAnchor.constraint(equalTo: button.centerXAnchor),
            stackView.centerYAnchor.constraint(equalTo: button.centerYAnchor),
            stackView.leadingAnchor.constraint(greaterThanOrEqualTo: button.leadingAnchor, constant: 8),
            stackView.trailingAnchor.constraint(lessThanOrEqualTo: button.trailingAnchor, constant: -8),

            iconContainer.widthAnchor.constraint(equalToConstant: 32),
            iconContainer.heightAnchor.constraint(equalToConstant: 32),
            iconView.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
            iconView.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 24),
            iconView.heightAnchor.constraint(equalToConstant: 24),

            button.topAnchor.constraint(equalTo: containerView.topAnchor),
            button.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            button.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            button.bottomAnchor.constraint(equalTo: containerView.bottomAnchor),
            button.heightAnchor.constraint(equalToConstant: 60),
            button.widthAnchor.constraint(equalToConstant: 60)
        ])

        return containerView
    }

    private func overflowHasNotification(from start: Int, config: TabBar) -> Bool {
        for index in start..<Int(config.items_count) {
            guard let item = getTabBarItem(appId, Int32(index)) else { continue }
            if item.has_red_dot || !item.badge.toString().isEmpty {
                return true
            }
        }
        return false
    }

    @objc private func moreButtonTapped() {
        guard let config = tabBarConfig, let host = superview else { return }
        let items = config.getItems(appId: appId)
        let start = overflowStart(itemCount: items.count, config: config)
        guard start >= 0 else { return }

        overflowPanel?.removeFromSuperview()
        let panel = LxAppTabBarOverflowPanel(
            items: items,
            indices: Array(start..<items.count),
            config: config,
            selectedIndex: selectedIndex,
            appId: appId
        ) { [weak self] index in
            self?.setSelectedIndex(index, notifyListener: true)
        }
        panel.present(in: host, above: self)
        overflowPanel = panel
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
    /// Phone-shaped host; see `MacOSLxAppTabBar.compact`. Republished so a
    /// simulated device change re-lays the strip.
    @Published private var compact: Bool = true
    private var onTabSelectedCallback: ((Int, String) -> Void)?
    /// Panel above the strip listing the folded items, while it is open.
    private weak var overflowPanel: NSView?

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
        // A pick from the overflow panel is a tab switch; the panel has done
        // its job either way.
        dismissOverflowPanel()
        if notifyListener, let callback = onTabSelectedCallback, let config = tabBarConfig {
            let items = config.getItems(appId: appId)
            guard items.indices.contains(index) else { return }
            selectedIndex = index
            callback(index, items[index].page_path.toString())
            return
        }

        // The binding redraws selection immediately. A Rust state-change
        // notification refreshes config after a user click; rebuilding the
        // hosting controller here would replace its root view inside the
        // SwiftUI Button action and briefly restore the stale Rust selection.
        selectedIndex = index
    }

    /// The folded items, shown flush above the strip. A desktop popover would
    /// float outside the simulated screen, so the panel is an ordinary subview
    /// of the same window instead — the phone hosts do the same.
    fileprivate func toggleOverflowPanel() {
        if overflowPanel != nil {
            dismissOverflowPanel()
            return
        }
        guard let host = window?.contentView, let config = tabBarConfig else { return }
        let start = Int(config.overflow_start_index)
        let count = Int(config.items_count)
        guard start >= 0, start < count else { return }

        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false

        let scrim = NSView()
        scrim.translatesAutoresizingMaskIntoConstraints = false
        scrim.wantsLayer = true
        scrim.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.4).cgColor
        scrim.addGestureRecognizer(
            NSClickGestureRecognizer(target: self, action: #selector(overflowScrimClicked))
        )
        container.addSubview(scrim)

        let grid = MacOSLxAppTabBar(
            appId: appId,
            config: config,
            selectedIndex: Binding(
                get: { [weak self] in self?.selectedIndex ?? 0 },
                set: { [weak self] value in self?.setSelectedIndex(value, notifyListener: true) }
            ),
            compact: compact,
            overflowGrid: Array(start..<count)
        ) { [weak self] index, _ in
            self?.setSelectedIndex(index, notifyListener: true)
        }
        let panel = NSHostingView(rootView: grid)
        panel.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(panel)

        host.addSubview(container, positioned: .above, relativeTo: nil)
        NSLayoutConstraint.activate([
            container.topAnchor.constraint(equalTo: host.topAnchor),
            container.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            container.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            container.bottomAnchor.constraint(equalTo: host.bottomAnchor),
            scrim.topAnchor.constraint(equalTo: container.topAnchor),
            scrim.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            scrim.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            scrim.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            // Flush on top of the strip, and no wider than it, so the panel
            // stays inside the simulated screen rather than the whole window.
            panel.bottomAnchor.constraint(equalTo: topAnchor),
            panel.leadingAnchor.constraint(equalTo: leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: trailingAnchor)
        ])
        overflowPanel = container
    }

    @objc private func overflowScrimClicked() {
        dismissOverflowPanel()
    }

    private func dismissOverflowPanel() {
        overflowPanel?.removeFromSuperview()
        overflowPanel = nil
    }

    /// Follow the host's size class: a phone-shaped runner folds extra items
    /// behind "more", a desktop or tablet one lays them all out.
    func setCompact(_ value: Bool) {
        guard compact != value else { return }
        compact = value
        updateSwiftUIView()
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
                selectedIndex: $wrapper.selectedIndex,
                compact: wrapper.compact,
                onMoreRequested: { wrapper.toggleOverflowPanel() }
            ) { index, path in
                wrapper.setSelectedIndex(index, notifyListener: true)
            }
        }
    }
}

typealias LingXiaTabBar = macOSTabBarWrapper
#endif
