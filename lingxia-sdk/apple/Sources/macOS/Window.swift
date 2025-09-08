#if os(macOS)
import SwiftUI
import Foundation
import os.log
import CLingXiaRustAPI

/// SwiftUI-based window for LxApp 
@available(macOS 13.0, *)
@MainActor
public struct LxAppSwiftUIWindow: Scene {
    @StateObject private var windowManager = LxAppWindowManager.shared

    public init() {}

    public var body: some Scene {
        WindowGroup {
            LxAppWindowContentView()
                .environmentObject(windowManager)
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .defaultSize(width: 1200, height: 800)
    }
}

/// Legacy NSWindow class for backward compatibility
public class LxAppWindow: NSWindow {
    private var windowStyle: LxAppWindowStyle

    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        // Initialize with a default value, will be set properly by configureForStyle()
        self.windowStyle = .tabStyle
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)
    }

    func configureForStyle(_ style: LxAppWindowStyle) {
        self.windowStyle = style
        LxAppWindowManager.shared.setWindowStyle(style)
        configureAppKitWindow(style)
    }

    private func configureAppKitWindow(_ style: LxAppWindowStyle) {
        switch style {
        case .capsuleStyle:
            // Custom capsule style with full-size content view
            styleMask.insert(.fullSizeContentView)
            titlebarAppearsTransparent = true
            titleVisibility = .hidden
            isMovableByWindowBackground = true
        case .tabStyle:
            // Tab-style with native window controls and custom tab bar
            styleMask.insert(.fullSizeContentView)
            titlebarAppearsTransparent = true
            titleVisibility = .hidden
            isMovableByWindowBackground = false // Tabs handle dragging
            backgroundColor = NSColor.windowBackgroundColor
            // Keep native window controls visible
        }
    }

    public override var canBecomeKey: Bool {
        return true
    }

    public override var canBecomeMain: Bool {
        return true
    }
}

/// Main content view for SwiftUI window
public struct LxAppWindowContentView: View {
    @EnvironmentObject private var windowManager: LxAppWindowManager
    @StateObject private var tabManager = LxAppTabManager.shared

    public var body: some View {
        VStack(spacing: 0) {
            // Custom title bar with window controls
            LxAppSwiftUITitleBar(
                style: windowManager.windowStyle,
                onMore: { windowManager.handleMoreAction() },
                onMinimize: { windowManager.handleMinimizeAction() },
                onClose: { windowManager.handleCloseAction() }
            )
            .frame(height: LxAppWindowLayout.titleBarHeight)

            // Tab bar (only for tab style)
            if windowManager.windowStyle == .tabStyle {
                LxAppSwiftUITabBar(tabManager: tabManager)
                    .frame(height: LxAppWindowLayout.macOSTabViewHeight)
            }

            // Main content area
            LxAppMainContentView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(Color(NSColor.windowBackgroundColor))
        .lxAppWindowStyle(windowManager.windowStyle)
    }
}

/// SwiftUI window manager - replaces macOSWindowSupport
@MainActor
public class LxAppWindowManager: ObservableObject {
    public static let shared = LxAppWindowManager()

    @Published public var windowStyle: LxAppWindowStyle = .tabStyle
    @Published public var isMinimized: Bool = false
    @Published public var windowTitle: String = "LingXia"

    private init() {}

    /// Handle more button action
    public func handleMoreAction() {
        if let appId = NavigationBarStateManager.shared.currentAppId {
            let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionMore)
        }
    }

    /// Handle minimize button action
    public func handleMinimizeAction() {
        isMinimized = true
        // SwiftUI will handle the actual minimization
        if let window = NSApp.keyWindow {
            window.miniaturize(nil)
        }
    }

    /// Handle close button action
    public func handleCloseAction() {
        if let appId = NavigationBarStateManager.shared.currentAppId {
            let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionClose)
        } else {
            NSApplication.shared.terminate(nil)
        }
    }

    /// Set window style
    public func setWindowStyle(_ style: LxAppWindowStyle) {
        windowStyle = style
    }

    /// Get top margin for content based on window style
    public func getTopMarginForStyle() -> CGFloat {
        switch windowStyle {
        case .capsuleStyle:
            return LxAppWindowLayout.titleBarHeight  // Custom capsule style needs space for title bar
        case .tabStyle:
            return LxAppWindowLayout.titleBarHeight + LxAppWindowLayout.macOSTabViewHeight  // Title bar + macOS tab view
        }
    }
}

/// SwiftUI title bar - replaces NSWindow titlebar
public struct LxAppSwiftUITitleBar: View {
    let style: LxAppWindowStyle
    let onMore: () -> Void
    let onMinimize: () -> Void
    let onClose: () -> Void
    @StateObject private var stateManager = NavigationBarStateManager.shared

    public var body: some View {
        ZStack {
            HStack(spacing: 0) {
                // Left side - window controls for tab style
                if style == .tabStyle {
                    // Standard macOS window controls area (70pt)
                    Rectangle()
                        .fill(Color.clear)
                        .frame(width: 70)
                }

                // Center - title or content
                Spacer()

                if style == .capsuleStyle {
                    Text("LingXia")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(.primary)
                }

                Spacer()

                // Right side - custom controls for capsule style
                if style == .capsuleStyle {
                    LxAppSwiftUICapsuleButtons(
                        onMoreTapped: onMore,
                        onMinimizeTapped: onMinimize,
                        onCloseTapped: onClose
                    )
                    .padding(.trailing, 7)
                }
            }
            .frame(height: LxAppWindowLayout.titleBarHeight)
            .background(titleBarBackground)
            .overlay(
                // Drag area for capsule style
                style == .capsuleStyle ?
                Color.clear.contentShape(Rectangle()) : nil
            )
            
            // Floating navbar buttons for capsule style - shown regardless of navbar visibility
            if style == .capsuleStyle {
                VStack(spacing: 0) {
                    Spacer()
                        .frame(height: 6) // Small margin from top

                    HStack {
                        floatingNavbarButton
                            .padding(.leading, 10)
                        Spacer()
                    }
                    .frame(height: 44)
                }
            }
        }
        .frame(height: max(32, style == .capsuleStyle ? 50 : 32)) // Allow more height for floating buttons
    }
    
    @ViewBuilder
    private var floatingNavbarButton: some View {
        if let state = stateManager.currentState {
            if state.show_back_button {
                NavigationButton(isBackButton: true, action: {
                    if let appId = stateManager.currentAppId {
                        let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionBack)
                    }
                })
            } else if state.show_home_button {
                NavigationButton(isBackButton: false, action: {
                    if let appId = stateManager.currentAppId {
                        let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionHome)
                    }
                })
            }
        }
    }

    private var titleBarBackground: some View {
        Group {
            if style == .capsuleStyle {
                Color(NSColor.windowBackgroundColor)
            } else {
                Color.clear
            }
        }
    }
}

/// SwiftUI capsule buttons - reuse unified implementation
public typealias LxAppSwiftUICapsuleButtons = LxAppUnifiedCapsuleViewMacOS

/// SwiftUI tab bar component
public struct LxAppSwiftUITabBar: View {
    @ObservedObject var tabManager: LxAppTabManager

    public var body: some View {
        HStack(spacing: 0) {
            // Window controls area (70pt)
            Rectangle()
                .fill(Color.clear)
                .frame(width: 70)

            // Home tabs
            ForEach(homeTabsList, id: \.appId) { tab in
                homeTabView(for: tab)
            }

            // Separator if needed
            if !homeTabsList.isEmpty && !regularTabsList.isEmpty {
                Rectangle()
                    .fill(Color(NSColor.separatorColor))
                    .frame(width: 1, height: 24)
                    .padding(.horizontal, 4)
            }

            // Regular tabs
            ForEach(regularTabsList, id: \.appId) { tab in
                regularTabView(for: tab)
            }

            Spacer()
        }
        .frame(height: LxAppWindowLayout.macOSTabViewHeight)
        .background(Color.clear)
    }

    private var homeTabsList: [LxAppTab] {
        let homeLxAppId = LxAppCore.getHomeLxAppId()
        return tabManager.tabs.filter { $0.appId == homeLxAppId }
    }

    private var regularTabsList: [LxAppTab] {
        let homeLxAppId = LxAppCore.getHomeLxAppId()
        return tabManager.tabs.filter { $0.appId != homeLxAppId }
    }

    private func homeTabView(for tab: LxAppTab) -> some View {
        Button(action: {
            tabManager.selectTab(appId: tab.appId)
        }) {
            Image(systemName: "house.fill")
                .font(.system(size: 16))
                .foregroundColor(Color(NSColor.labelColor).opacity(0.8))
        }
        .buttonStyle(PlainButtonStyle())
        .frame(width: 40, height: 32)
        .contentShape(Rectangle())
    }

    private func regularTabView(for tab: LxAppTab) -> some View {
        let isActive = tabManager.activeTab?.appId == tab.appId
        let tabWidth = calculateTabWidth()

        return HStack(spacing: 8) {
            // Tab title
            Text(truncatedTitle(for: tab))
                .font(isActive ? .system(size: 13, weight: .semibold) : .system(size: 13))
                .foregroundColor(isActive ? Color(NSColor.labelColor) : Color(NSColor.secondaryLabelColor))
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer()

            // Close button for closable tabs
            if tab.isClosable && isActive {
                Button(action: {
                    tabManager.closeTab(appId: tab.appId)
                }) {
                    closeButtonImage
                }
                .buttonStyle(PlainButtonStyle())
                .frame(width: 16, height: 16)
            }
        }
        .padding(.horizontal, 12)
        .frame(width: tabWidth, height: 32)
        .background(tabBackground(isActive: isActive))
        .cornerRadius(6)
        .contentShape(Rectangle())
        .onTapGesture {
            tabManager.selectTab(appId: tab.appId)
        }
        .help(tab.title)
    }

    private func calculateTabWidth() -> CGFloat {
        let regularTabsCount = regularTabsList.count
        guard regularTabsCount > 0 else { return 160 }

        let totalWidth: CGFloat = 1200
        let usedWidth: CGFloat = 70 + (!homeTabsList.isEmpty ? 40 : 0) + (!homeTabsList.isEmpty && !regularTabsList.isEmpty ? 9 : 0)
        let availableWidth = totalWidth - usedWidth

        return min(160, max(80, availableWidth / CGFloat(regularTabsCount)))
    }

    private func truncatedTitle(for tab: LxAppTab) -> String {
        let maxLength = 10
        return tab.title.count > maxLength ? String(tab.title.prefix(maxLength - 1)) + "…" : tab.title
    }

    private func tabBackground(isActive: Bool) -> some View {
        Group {
            if isActive {
                Color(NSColor.controlBackgroundColor)
                    .shadow(color: .black.opacity(0.2), radius: 4, x: 0, y: 2)
            } else {
                Color(NSColor.controlBackgroundColor).opacity(0.1)
            }
        }
    }

    private var closeButtonImage: some View {
        ZStack {
            Path { path in
                let margin: CGFloat = 4
                let size: CGFloat = 16
                path.move(to: CGPoint(x: margin, y: margin))
                path.addLine(to: CGPoint(x: size - margin, y: size - margin))
                path.move(to: CGPoint(x: size - margin, y: margin))
                path.addLine(to: CGPoint(x: margin, y: size - margin))
            }
            .stroke(Color(NSColor.labelColor).opacity(0.6), style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round))
        }
        .frame(width: 16, height: 16)
    }
}

/// Main content view placeholder
public struct LxAppMainContentView: View {
    public var body: some View {
        VStack {
            Text("LingXia Content Area")
                .font(.title2)
                .fontWeight(.semibold)

            Text("SwiftUI-based window content")
                .font(.body)
                .foregroundColor(.secondary)

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(NSColor.windowBackgroundColor))
    }
}

/// SwiftUI window style modifier
public extension View {
    func lxAppWindowStyle(_ style: LxAppWindowStyle) -> some View {
        self
            .background(
                Group {
                    if style == .capsuleStyle {
                        Color(NSColor.windowBackgroundColor)
                            .cornerRadius(12)
                            .shadow(radius: 10)
                    } else {
                        Color(NSColor.windowBackgroundColor)
                    }
                }
            )
    }
}



#endif
