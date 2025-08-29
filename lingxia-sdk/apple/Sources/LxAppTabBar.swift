import SwiftUI
import Foundation
import CLingXiaFFI
import os.log

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

extension Notification.Name {
    static let tabBarStateChanged = Notification.Name("TabBarDataChanged")
}

/// Extensions for TabBar
extension TabBar {
    public var positionEnum: TabBarPosition {
        switch position {
        case 1: return .left
        case 2: return .right
        default: return .bottom
        }
    }

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

    public static func isTransparent(_ colorValue: UInt32) -> Bool {
        return (colorValue >> 24) & 0xFF == 0
    }

    public func getResolvedBackgroundColor(isVertical: Bool) -> PlatformColor {
        return TabBarHelper.resolvedBackgroundColor(background_color, isVertical: isVertical)
    }
}

public enum TabBarPosition {
    case bottom, left, right
}

public struct TabBarConstants {
    public static let DEFAULT_SPACING: CGFloat = 8
    public static let CENTER_SPACING: CGFloat = 8
    public static let MINIMAL_SPACER_SIZE: CGFloat = 4
}

/// Extensions for TabBarItem
extension TabBarItem {
    public var visible: Bool { true }
    public var cachedPagePath: String { page_path.toString() }
    public var cachedText: String { text.toString() }
    public var cachedIconPath: String { icon_path.toString() }
    public var cachedSelectedIconPath: String { selected_icon_path.toString() }
}

/// TabBar styling helpers
public struct TabBarHelper {
    public static func resolvedBackgroundColor(_ colorValue: UInt32, isVertical: Bool) -> PlatformColor {
        if (colorValue >> 24) & 0xFF == 0 {
            return isVertical ?
                PlatformColor(red: 0.95, green: 0.95, blue: 0.95, alpha: 1.0) :
                PlatformColor(red: 0.98, green: 0.98, blue: 0.98, alpha: 1.0)
        }
        return PlatformColor(argb: colorValue)
    }
}

/// Unified SwiftUI TabBar for iOS and macOS
public struct LxAppTabBar: View {
    let appId: String
    let config: TabBar
    @Binding var selectedIndex: Int
    let onTabSelected: (Int, String) -> Void
    // Simple refresh trigger for UI updates
    @State private var refreshTrigger = false

    public init(
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

    public var body: some View {
        // Get fresh data from Rust every time body is called
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
        .id("tabbar-\(selectedIndex)-\(refreshTrigger)")
        .onReceive(NotificationCenter.default.publisher(for: .tabBarStateChanged)) { notification in
            // Trigger UI refresh when updateTabBarUI() is called for this app
            if let notificationAppId = notification.object as? String, notificationAppId == appId {
                refreshTrigger.toggle()
            }
        }
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
            let startItems = getStartItems(items: items)
            let centerItems = getCenterItems(items: items)
            let endItems = getEndItems(items: items)

            // Start items (group 1)
            if !startItems.isEmpty {
                HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(startItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildCompactTabItem(item: item, index: index)
                    }
                }
                .padding(.leading, 6) // Slightly more padding from edge
            }

            // Flexible spacer
            Spacer()

            // Center items (group 0)
            if !centerItems.isEmpty {
                HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(centerItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildCompactTabItem(item: item, index: index)
                    }
                }
            }

            // Flexible spacer
            Spacer()

            // End items (group 2)
            if !endItems.isEmpty {
                HStack(spacing: 6) { // Comfortable spacing between end items
                    ForEach(Array(endItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildCompactTabItem(item: item, index: index)
                    }
                }
                .padding(.trailing, 6) // Slightly more padding from edge
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
        // Get state directly from Rust
        let rustItem = getTabBarItem(appId, Int32(index))

        let forceColor = isSelected ?
            Color(PlatformColor(argb: config.selected_color)) :
            Color(PlatformColor(argb: config.color))

        Button(action: {
            // Update selection and trigger re-render
            selectedIndex = index
            refreshTrigger.toggle()
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
                        buildBadge(text: rustItem.badge.toString())
                            .offset(x: 12, y: -8)
                    }
                    // Red dot overlay (only show if no badge)
                    else if let rustItem = rustItem, rustItem.has_red_dot {
                        buildRedDot()
                            .offset(x: 12, y: -8)
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

    // Compact tab item for group layouts - uses natural content size instead of maxWidth: .infinity
    @ViewBuilder
    private func buildCompactTabItem(item: TabBarItem, index: Int) -> some View {
        let isSelected = (index == selectedIndex)
        // Get state directly from Rust
        let rustItem = getTabBarItem(appId, Int32(index))

        let forceColor = isSelected ?
            Color(PlatformColor(argb: config.selected_color)) :
            Color(PlatformColor(argb: config.color))

        Button(action: {
            // Update selection and trigger re-render
            selectedIndex = index
            refreshTrigger.toggle()
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
                        buildBadge(text: rustItem.badge.toString())
                            .offset(x: 12, y: -8)
                    }
                    // Red dot overlay (only show if no badge)
                    else if let rustItem = rustItem, rustItem.has_red_dot {
                        buildRedDot()
                            .offset(x: 12, y: -8)
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
            // Natural content size - no maxWidth expansion for group layouts
            .padding(.vertical, LxAppTheme.Metrics.smallSpacing)
        }
        .buttonStyle(PlainButtonStyle())
    }

    private func findItemIndex(for item: TabBarItem, in items: [TabBarItem]) -> Int {
        return items.firstIndex(where: { $0.cachedPagePath == item.cachedPagePath }) ?? 0
    }

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

    @ViewBuilder
    private func buildBadge(text: String) -> some View {
        Text(text)
            .font(.system(size: 10, weight: .medium))
            .foregroundColor(.white)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Capsule().fill(Color.red))
            .zIndex(1)
    }

    @ViewBuilder
    private func buildRedDot() -> some View {
        Circle()
            .fill(Color.red)
            .frame(width: 8, height: 8)
            .zIndex(1)
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
public struct MacOSLxAppTabBar: View {
    let appId: String
    let config: TabBar
    @Binding var selectedIndex: Int
    let onTabSelected: (Int, String) -> Void

    public init(
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

    // Copy helper methods from LxAppTabBar
    @ViewBuilder
    private func buildTabItem(item: TabBarItem, index: Int) -> some View {
        let isSelected = (index == selectedIndex)
        // Get state directly from Rust
        let rustItem = getTabBarItem(appId, Int32(index))

        Button(action: {
            selectedIndex = index
            onTabSelected(index, item.cachedPagePath)
        }) {
            VStack(spacing: LxAppTheme.Metrics.smallSpacing) {
                // Tab icon with badge and red dot overlay
                ZStack {
                    if !item.cachedIconPath.isEmpty {
                        buildTabIcon(item: item, isSelected: isSelected)
                    }

                    // Badge overlay (from Rust state)
                    if let rustItem = rustItem, !rustItem.badge.toString().isEmpty {
                        buildBadge(text: rustItem.badge.toString())
                            .offset(x: 12, y: -8)
                    }
                    // Red dot overlay (only show if no badge)
                    else if let rustItem = rustItem, rustItem.has_red_dot {
                        buildRedDot()
                            .offset(x: 12, y: -8)
                    }
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

    // Copy helper methods from LxAppTabBar
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
            let startItems = getStartItems(items: items)
            let centerItems = getCenterItems(items: items)
            let endItems = getEndItems(items: items)

            // Start items (group 1) - close to left edge
            if !startItems.isEmpty {
                HStack(spacing: 4) {
                    ForEach(Array(startItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
                .padding(.leading, 4)
            }

            Spacer()

            // Center items (group 0)
            if !centerItems.isEmpty {
                HStack(spacing: LxAppTheme.Metrics.standardSpacing) {
                    ForEach(Array(centerItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
            }

            Spacer()

            // End items (group 2) - close together, close to right edge
            if !endItems.isEmpty {
                HStack(spacing: 4) {
                    ForEach(Array(endItems.enumerated()), id: \.offset) { _, item in
                        let index = findItemIndex(for: item, in: items)
                        buildTabItem(item: item, index: index)
                    }
                }
                .padding(.trailing, 4)
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

    // Helper methods copied from LxAppTabBar
    private func findItemIndex(for item: TabBarItem, in items: [TabBarItem]) -> Int {
        return items.firstIndex(where: { $0.cachedPagePath == item.cachedPagePath }) ?? 0
    }

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
            Color.black

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

    @ViewBuilder
    private func buildBadge(text: String) -> some View {
        Text(text)
            .font(.system(size: 10, weight: .medium))
            .foregroundColor(.white)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Capsule().fill(Color.red))
            .zIndex(1)
    }

    @ViewBuilder
    private func buildRedDot() -> some View {
        Circle()
            .fill(Color.red)
            .frame(width: 8, height: 8)
            .zIndex(1)
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
@MainActor
public protocol TabBarProtocol: AnyObject {
    var config: TabBar? { get }
    func setConfig(config: TabBar, appId: String)
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
    private var tabBarConfig: TabBar?
    private var appId: String = ""
    private var selectedIndex: Int = 0
    private var onTabSelectedCallback: ((Int, String) -> Void)?
    private var tabBarUpdateObserver: NSObjectProtocol?

    // Public accessor for tabBarConfig
    public var config: TabBar? {
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

    private func setupNotificationObserver() {
        // Remove existing observer if any
        if let observer = tabBarUpdateObserver {
            NotificationCenter.default.removeObserver(observer)
        }

        // Only setup observer if we have an appId
        guard !appId.isEmpty else { return }

        // Listen for TabBar update notifications
        tabBarUpdateObserver = NotificationCenter.default.addObserver(
            forName: .tabBarStateChanged,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            // Extract notification data outside of Task to avoid data races
            guard let notificationAppId = notification.object as? String else { return }

            Task { @MainActor in
                guard let self = self,
                      notificationAppId == self.appId else { return }

                // Force TabBar to refresh - it will automatically get fresh badge/red dot data from Rust
                self.updateLayout()
            }
        }
    }

    public func setConfig(config: TabBar, appId: String) {
        self.tabBarConfig = config
        self.appId = appId

        // Setup notification observer now that we have the appId
        setupNotificationObserver()
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

    public func setSelectedIndex(_ index: Int, notifyListener: Bool) {
        selectedIndex = index
        updateLayout()

        if notifyListener, let callback = onTabSelectedCallback, let config = tabBarConfig {
            let items = config.getItems(appId: appId)
            if index < items.count {
                callback(index, items[index].page_path.toString())
            }
        }
    }

    public func forceTransparencyMode() {
        backgroundColor = UIColor.clear
        layer.backgroundColor = UIColor.clear.cgColor
    }


    private func findButton(for index: Int) -> UIButton? {
        return findButtonRecursively(in: self, tag: index)
    }

    private func findButtonRecursively(in view: UIView, tag: Int) -> UIButton? {
        if let button = view as? UIButton, button.tag == tag {
            return button
        }

        for subview in view.subviews {
            if let foundButton = findButtonRecursively(in: subview, tag: tag) {
                return foundButton
            }
        }

        return nil
    }

    public func updateLayout() {
        guard let config = tabBarConfig else { return }

        let items = config.getItems(appId: appId)

        // Always recreate layout to ensure fresh badge/red dot data
        setupUIKitLayout(items: items, config: config)
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

    private func setupUIKitLayout(items: [TabBarItem], config: TabBar) {
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

    private func setupVerticalGroupedLayout(items: [TabBarItem], config: TabBar, containerView: UIView) {
        setupGroupedLayout(items: items, config: config, containerView: containerView, isVertical: true)
    }

    private func setupHorizontalGroupedLayout(items: [TabBarItem], config: TabBar, containerView: UIView) {
        setupGroupedLayout(items: items, config: config, containerView: containerView, isVertical: false)
    }

    private func setupGroupedLayout(items: [TabBarItem], config: TabBar, containerView: UIView, isVertical: Bool) {
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

    private func addGroupContainer(items: [TabBarItem], allItems: [TabBarItem], config: TabBar, to stackView: UIStackView, isVertical: Bool) {
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

    private func createUIKitIcon(item: TabBarItem, isSelected: Bool) -> UIView {
        // Create container view for icon + badge/red dot
        let iconContainer = UIView()
        iconContainer.translatesAutoresizingMaskIntoConstraints = false

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

        iconContainer.addSubview(iconView)

        // Get badge and red dot data from Rust
        let itemIndex = findItemIndex(item: item)
        if itemIndex >= 0, let rustItem = getTabBarItem(appId, Int32(itemIndex)) {
            let badgeText = rustItem.badge.toString()
            let hasRedDot = rustItem.has_red_dot

            // Add badge if present
            if !badgeText.isEmpty {
                let badgeView = createBadgeView(text: badgeText)
                iconContainer.addSubview(badgeView)

                NSLayoutConstraint.activate([
                    badgeView.topAnchor.constraint(equalTo: iconContainer.topAnchor, constant: -4),
                    badgeView.trailingAnchor.constraint(equalTo: iconContainer.trailingAnchor, constant: 4)
                ])
            }
            // Add red dot if no badge and red dot is enabled
            else if hasRedDot {
                let redDotView = createRedDotView()
                iconContainer.addSubview(redDotView)

                NSLayoutConstraint.activate([
                    redDotView.topAnchor.constraint(equalTo: iconContainer.topAnchor, constant: -2),
                    redDotView.trailingAnchor.constraint(equalTo: iconContainer.trailingAnchor, constant: 2)
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

    private func findItemIndex(item: TabBarItem) -> Int {
        guard let config = tabBarConfig else { return -1 }
        let items = config.getItems(appId: appId)

        for (index, configItem) in items.enumerated() {
            if configItem.page_path.toString() == item.page_path.toString() {
                return index
            }
        }
        return -1
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

    private func createBadgeView(text: String) -> UIView {
        let badgeView = UIView()
        badgeView.backgroundColor = UIColor.red
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

    private func createRedDotView() -> UIView {
        let redDotView = UIView()
        redDotView.backgroundColor = UIColor.red
        redDotView.layer.cornerRadius = 4
        redDotView.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            redDotView.widthAnchor.constraint(equalToConstant: 8),
            redDotView.heightAnchor.constraint(equalToConstant: 8)
        ])

        return redDotView
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
    private var tabBarConfig: TabBar?
    private var appId: String = ""
    @Published private var selectedIndex: Int = 0
    private var onTabSelectedCallback: ((Int, String) -> Void)?

    public var config: TabBar? {
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

    public func setConfig(config: TabBar, appId: String) {
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

public typealias LingXiaTabBar = macOSTabBarWrapper
public typealias PlatformTabBar = macOSTabBarWrapper
#endif

