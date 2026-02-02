#if os(macOS)
import SwiftUI
import Foundation
import os.log
import CLingXiaRustAPI

/// NSWindow class for LxApp Tab mode
public class LxAppWindow: NSWindow {

    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)
    }

    func configureForTabStyle() {
        // Tab-style with native window controls and custom tab bar
        styleMask.insert(.fullSizeContentView)
        titlebarAppearsTransparent = true
        titleVisibility = .hidden
        isMovableByWindowBackground = false // Tabs handle dragging
        backgroundColor = NSColor.windowBackgroundColor
    }

    public override var canBecomeKey: Bool {
        return true
    }

    public override var canBecomeMain: Bool {
        return true
    }
}

/// SwiftUI tab bar component for Tab mode
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
        return tabManager.tabs.filter { LxAppCore.isHomeLxApp($0.appId) }
    }

    private var regularTabsList: [LxAppTab] {
        return tabManager.tabs.filter { !LxAppCore.isHomeLxApp($0.appId) }
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

#endif
