#if os(macOS)
import SwiftUI
import Foundation

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
            .frame(height: 32)

            // Tab bar (only for tab style)
            if windowManager.windowStyle == .tabStyle {
                LxAppSwiftUITabBar(tabManager: tabManager)
                    .frame(height: 32)
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
        print("More button tapped")
        // Implement more menu functionality
    }

    /// Handle minimize button action
    public func handleMinimizeAction() {
        print("Minimize button tapped")
        isMinimized = true
        // SwiftUI will handle the actual minimization
    }

    /// Handle close button action
    public func handleCloseAction() {
        print("Close button tapped")
        // SwiftUI will handle the actual closing
        NSApplication.shared.terminate(nil)
    }

    /// Set window style
    public func setWindowStyle(_ style: LxAppWindowStyle) {
        windowStyle = style
    }

    /// Get top margin for content based on window style
    public func getTopMarginForStyle() -> CGFloat {
        switch windowStyle {
        case .capsuleStyle:
            return 32  // Custom capsule style needs space for title bar
        case .tabStyle:
            return 64  // Title bar + tab bar
        }
    }
}

/// SwiftUI title bar - replaces NSWindow titlebar
public struct LxAppSwiftUITitleBar: View {
    let style: LxAppWindowStyle
    let onMore: () -> Void
    let onMinimize: () -> Void
    let onClose: () -> Void

    public var body: some View {
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
                    onMore: onMore,
                    onMinimize: onMinimize,
                    onClose: onClose
                )
                .padding(.trailing, 7)
            }
        }
        .frame(height: 32)
        .background(titleBarBackground)
        .overlay(
            // Drag area for capsule style
            style == .capsuleStyle ?
            Color.clear.contentShape(Rectangle()) : nil
        )
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

/// SwiftUI capsule buttons - replaces NSButton controls
public struct LxAppSwiftUICapsuleButtons: View {
    let onMore: () -> Void
    let onMinimize: () -> Void
    let onClose: () -> Void

    private let buttonWidth: CGFloat = 87 / 3
    private let buttonHeight: CGFloat = 28

    public var body: some View {
        HStack(spacing: 0) {
            // More button with custom drawn three dots
            Button(action: onMore) {
                MacOSThreeDotsView()
                    .frame(width: buttonWidth, height: buttonHeight)
                    .contentShape(Rectangle())
            }
            .buttonStyle(PlainButtonStyle())
            .help("More options")

            // Separator
            Rectangle()
                .fill(Color.gray.opacity(0.15))
                .frame(width: 0.5, height: buttonHeight - 12)

            // Minimize button with custom drawn minimize icon
            Button(action: onMinimize) {
                MacOSMinimizeView()
                    .frame(width: buttonWidth, height: buttonHeight)
                    .contentShape(Rectangle())
            }
            .buttonStyle(PlainButtonStyle())
            .help("Minimize")

            // Separator
            Rectangle()
                .fill(Color.gray.opacity(0.15))
                .frame(width: 0.5, height: buttonHeight - 12)

            // Close button with custom drawn close icon
            Button(action: onClose) {
                MacOSCloseView()
                    .frame(width: buttonWidth, height: buttonHeight)
                    .contentShape(Rectangle())
            }
            .buttonStyle(PlainButtonStyle())
            .help("Close")
        }
        .background(
            Color.white.opacity(0.9)
                .background(.ultraThinMaterial)
        )
        .clipShape(Capsule())
        .overlay(
            Capsule()
                .stroke(Color.gray.opacity(0.3), lineWidth: 0.5)
        )
        .frame(width: 87, height: buttonHeight)
        .shadow(color: .black.opacity(0.1), radius: 2, x: 0, y: 1)
    }
}

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
        .frame(height: 32)
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
                    // Handle tab close
                    print("Close tab: \(tab.appId)")
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

/// Custom three dots view for macOS capsule buttons
private struct MacOSThreeDotsView: View {
    var body: some View {
        Canvas { context, size in
            let centerY = size.height / 2
            let centerX = size.width / 2
            let centerDotRadius = size.height / 7
            let sideDotRadius = size.height / 10
            let spacing = centerDotRadius * 2.8

            let dotsPath = Path { path in
                // Left dot
                path.addEllipse(in: CGRect(
                    x: centerX - spacing - sideDotRadius,
                    y: centerY - sideDotRadius,
                    width: sideDotRadius * 2,
                    height: sideDotRadius * 2
                ))

                // Center dot
                path.addEllipse(in: CGRect(
                    x: centerX - centerDotRadius,
                    y: centerY - centerDotRadius,
                    width: centerDotRadius * 2,
                    height: centerDotRadius * 2
                ))

                // Right dot
                path.addEllipse(in: CGRect(
                    x: centerX + spacing - sideDotRadius,
                    y: centerY - sideDotRadius,
                    width: sideDotRadius * 2,
                    height: sideDotRadius * 2
                ))
            }

            context.fill(dotsPath, with: .color(.primary))
        }
    }
}

/// Custom minimize view for macOS capsule buttons
private struct MacOSMinimizeView: View {
    var body: some View {
        Canvas { context, size in
            let centerX = size.width / 2
            let centerY = size.height / 2
            let lineLength: CGFloat = size.width * 0.4

            let linePath = Path { path in
                path.move(to: CGPoint(x: centerX - lineLength/2, y: centerY))
                path.addLine(to: CGPoint(x: centerX + lineLength/2, y: centerY))
            }

            context.stroke(linePath, with: .color(.primary), style: StrokeStyle(lineWidth: 2.2, lineCap: .round))
        }
    }
}

/// Custom close view for macOS capsule buttons
private struct MacOSCloseView: View {
    var body: some View {
        Canvas { context, size in
            let centerX = size.width / 2
            let centerY = size.height / 2
            let outerRadius = size.width * 0.35
            let innerRadius: CGFloat = 2.5

            // Outer circle (stroke)
            let outerCirclePath = Path { path in
                path.addEllipse(in: CGRect(
                    x: centerX - outerRadius,
                    y: centerY - outerRadius,
                    width: outerRadius * 2,
                    height: outerRadius * 2
                ))
            }

            context.stroke(outerCirclePath, with: .color(.primary), style: StrokeStyle(lineWidth: 2.2, lineCap: .round))

            // Inner circle (fill)
            let innerCirclePath = Path { path in
                path.addEllipse(in: CGRect(
                    x: centerX - innerRadius,
                    y: centerY - innerRadius,
                    width: innerRadius * 2,
                    height: innerRadius * 2
                ))
            }

            context.fill(innerCirclePath, with: .color(.primary))
        }
    }
}

#endif
