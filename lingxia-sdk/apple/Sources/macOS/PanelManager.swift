#if os(macOS)
import AppKit
import os.log

/// Position of a panel relative to the main content
public enum PanelPosition: String {
    case left
    case right
    case bottom
}

/// Scope determines panel lifecycle relative to tabs
public enum PanelScope {
    case shared   // persists across tab switches
    case perTab   // each tab gets its own panel state
}

/// Configuration for a panel slot
public struct PanelConfig {
    public let id: String
    public let position: PanelPosition
    public let scope: PanelScope
    public let minSize: CGFloat
    public let maxSize: CGFloat
    public let defaultSize: CGFloat

    public init(id: String, position: PanelPosition, scope: PanelScope = .shared,
                minSize: CGFloat = 150, maxSize: CGFloat = 500, defaultSize: CGFloat = 250) {
        self.id = id
        self.position = position
        self.scope = scope
        self.minSize = minSize
        self.maxSize = maxSize
        self.defaultSize = defaultSize
    }
}

/// NSSplitView subclass with customizable divider color
class PanelSplitView: NSSplitView {
    var customDividerColor: NSColor = .separatorColor

    override var dividerColor: NSColor {
        return customDividerColor
    }
}

/// A single panel slot — pure layout container
class PanelSlot {
    let config: PanelConfig
    let containerView: NSView
    var isVisible: Bool = false
    var currentSize: CGFloat

    init(config: PanelConfig) {
        self.config = config
        self.containerView = NSView()
        self.containerView.wantsLayer = true
        self.currentSize = config.defaultSize
    }
}

/// Manages panel layout using NSSplitView.
///
/// Layout structure (when all panels visible):
/// ```
/// horizontalSplitView (isVertical = true → side-by-side panes)
///   ├── leftPanelHolder
///   ├── verticalSplitView (isVertical = false → stacked panes)
///   │     ├── contentContainer
///   │     └── bottomPanelHolder
///   └── rightPanelHolder
/// ```
///
/// Panel holders are added/removed from split views dynamically.
/// Initially only contentContainer and verticalSplitView are present.
///
/// Panels are pure layout containers. Content (WebViews) is attached via
/// `WebViewManager.attachWebViewToContainer(webView, container: panelContainer)`.
@MainActor
public class PanelLayoutManager: NSObject, NSSplitViewDelegate {

    private static let log = OSLog(subsystem: "LingXia", category: "PanelLayout")

    // MARK: - Split views

    /// Horizontal split: [left panel] | [center (vertical split)] | [right panel]
    private let horizontalSplitView = PanelSplitView()

    /// Vertical split: [content] | [bottom panel]  (nested inside horizontal center)
    private let verticalSplitView = PanelSplitView()

    // MARK: - Layout holders (kept alive but only added to split view when visible)

    private let leftPanelHolder = NSView()
    private let rightPanelHolder = NSView()
    private let bottomPanelHolder = NSView()

    /// The main content container where the active VC's view is placed
    public let contentContainer = NSView()

    // MARK: - Panel registry

    private var panels: [String: PanelSlot] = [:]
    private var positionMap: [PanelPosition: String] = [:]

    /// The root view to embed in the window
    public var rootView: NSView { horizontalSplitView }

    /// Divider color for all split views (default: system separator color)
    public var dividerColor: NSColor {
        get { horizontalSplitView.customDividerColor }
        set {
            horizontalSplitView.customDividerColor = newValue
            verticalSplitView.customDividerColor = newValue
            horizontalSplitView.needsDisplay = true
            verticalSplitView.needsDisplay = true
        }
    }

    // MARK: - Init

    override init() {
        super.init()
        setupSplitViews()
    }

    private func setupSplitViews() {
        // Horizontal split: side-by-side panes with vertical dividers
        horizontalSplitView.isVertical = true
        horizontalSplitView.dividerStyle = .thin
        horizontalSplitView.delegate = self
        horizontalSplitView.translatesAutoresizingMaskIntoConstraints = false

        // Vertical split: stacked panes with horizontal divider
        verticalSplitView.isVertical = false
        verticalSplitView.dividerStyle = .thin
        verticalSplitView.delegate = self
        verticalSplitView.translatesAutoresizingMaskIntoConstraints = false

        contentContainer.wantsLayer = true

        for holder in [leftPanelHolder, rightPanelHolder, bottomPanelHolder] {
            holder.wantsLayer = true
        }

        // Initial layout: only content, no panels
        verticalSplitView.addSubview(contentContainer)
        horizontalSplitView.addSubview(verticalSplitView)
    }

    // MARK: - Panel Operations

    /// Whether a panel with the given ID has been registered
    public func isPanelRegistered(id: String) -> Bool {
        return panels[id] != nil
    }

    /// Register a panel and return its container view for attaching content.
    /// The holder is NOT added to the split view yet — call `showPanel` for that.
    @discardableResult
    public func registerPanel(_ config: PanelConfig) -> NSView {
        if let existing = panels[config.id] {
            os_log("Panel already registered: %@, returning existing container", log: Self.log, type: .info, config.id)
            return existing.containerView
        }

        let slot = PanelSlot(config: config)
        panels[config.id] = slot
        positionMap[config.position] = config.id

        // Add the slot's container into the holder (holder is not in split view yet)
        let holder = holderView(for: config.position)
        slot.containerView.translatesAutoresizingMaskIntoConstraints = false
        holder.addSubview(slot.containerView)

        NSLayoutConstraint.activate([
            slot.containerView.topAnchor.constraint(equalTo: holder.topAnchor),
            slot.containerView.leadingAnchor.constraint(equalTo: holder.leadingAnchor),
            slot.containerView.trailingAnchor.constraint(equalTo: holder.trailingAnchor),
            slot.containerView.bottomAnchor.constraint(equalTo: holder.bottomAnchor),
        ])

        os_log("Panel registered: %@ at %@", log: Self.log, type: .info, config.id, config.position.rawValue)
        return slot.containerView
    }

    /// Show a panel by inserting its holder into the split view
    public func showPanel(id: String) {
        guard let slot = panels[id] else {
            os_log("showPanel: unknown panel %@", log: Self.log, type: .error, id)
            return
        }
        guard !slot.isVisible else { return }

        slot.isVisible = true

        let holder = holderView(for: slot.config.position)
        let sv = splitView(for: slot.config.position)

        // Insert holder at the correct position in the split view
        switch slot.config.position {
        case .left:
            // Before the center (verticalSplitView)
            sv.addSubview(holder, positioned: .below, relativeTo: verticalSplitView)
        case .right:
            // After the center (verticalSplitView)
            sv.addSubview(holder, positioned: .above, relativeTo: verticalSplitView)
        case .bottom:
            // After the content container
            sv.addSubview(holder, positioned: .above, relativeTo: contentContainer)
        }

        // Let the split view lay out, then set the divider to the desired position
        sv.adjustSubviews()
        applyPanelSize(slot)

        os_log("Panel shown: %@", log: Self.log, type: .info, id)
    }

    /// Hide a panel by removing its holder from the split view
    public func hidePanel(id: String) {
        guard let slot = panels[id] else {
            os_log("hidePanel: unknown panel %@", log: Self.log, type: .error, id)
            return
        }
        guard slot.isVisible else { return }

        slot.isVisible = false

        let holder = holderView(for: slot.config.position)
        let sv = splitView(for: slot.config.position)

        holder.removeFromSuperview()
        sv.adjustSubviews()

        os_log("Panel hidden: %@", log: Self.log, type: .info, id)
    }

    /// Toggle panel visibility
    public func togglePanel(id: String) {
        guard let slot = panels[id] else {
            os_log("togglePanel: unknown panel %@", log: Self.log, type: .error, id)
            return
        }
        if slot.isVisible {
            hidePanel(id: id)
        } else {
            showPanel(id: id)
        }
    }

    /// Check if panel is visible
    public func isPanelVisible(id: String) -> Bool {
        return panels[id]?.isVisible ?? false
    }

    /// Get the container view for a panel (for attaching WebViews)
    public func panelContainer(id: String) -> NSView? {
        return panels[id]?.containerView
    }

    // MARK: - Private Helpers

    private func holderView(for position: PanelPosition) -> NSView {
        switch position {
        case .left: return leftPanelHolder
        case .right: return rightPanelHolder
        case .bottom: return bottomPanelHolder
        }
    }

    private func splitView(for position: PanelPosition) -> NSSplitView {
        switch position {
        case .left, .right: return horizontalSplitView
        case .bottom: return verticalSplitView
        }
    }

    /// Find the divider index for a panel position among current subviews
    private func dividerIndex(for position: PanelPosition) -> Int? {
        let sv = splitView(for: position)
        let holder = holderView(for: position)
        guard let idx = sv.subviews.firstIndex(of: holder) else { return nil }

        switch position {
        case .left:
            // Divider is to the right of the left panel holder
            return idx
        case .right, .bottom:
            // Divider is to the left/above the panel holder
            guard idx > 0 else { return nil }
            return idx - 1
        }
    }

    private func applyPanelSize(_ slot: PanelSlot) {
        guard let divIdx = dividerIndex(for: slot.config.position) else { return }
        let sv = splitView(for: slot.config.position)

        switch slot.config.position {
        case .left:
            sv.setPosition(slot.currentSize, ofDividerAt: divIdx)
        case .right:
            sv.setPosition(sv.frame.width - slot.currentSize, ofDividerAt: divIdx)
        case .bottom:
            sv.setPosition(sv.frame.height - slot.currentSize, ofDividerAt: divIdx)
        }
    }

    /// Identify which panel slot is adjacent to a given divider
    private func panelSlot(forDividerAt dividerIndex: Int, in splitView: NSSplitView) -> (slot: PanelSlot, side: PanelPosition)? {
        let subs = splitView.subviews
        guard dividerIndex >= 0, dividerIndex < subs.count - 1 else { return nil }

        let leftOfDivider = subs[dividerIndex]
        let rightOfDivider = subs[dividerIndex + 1]

        if splitView === horizontalSplitView {
            if leftOfDivider === leftPanelHolder,
               let id = positionMap[.left], let slot = panels[id], slot.isVisible {
                return (slot, .left)
            }
            if rightOfDivider === rightPanelHolder,
               let id = positionMap[.right], let slot = panels[id], slot.isVisible {
                return (slot, .right)
            }
        } else if splitView === verticalSplitView {
            if rightOfDivider === bottomPanelHolder,
               let id = positionMap[.bottom], let slot = panels[id], slot.isVisible {
                return (slot, .bottom)
            }
        }

        return nil
    }

    // MARK: - NSSplitViewDelegate

    public func splitView(_ splitView: NSSplitView,
                          constrainMinCoordinate proposedMinimumPosition: CGFloat,
                          ofSubviewAt dividerIndex: Int) -> CGFloat {
        guard let info = panelSlot(forDividerAt: dividerIndex, in: splitView) else {
            return proposedMinimumPosition
        }

        switch info.side {
        case .left:
            return info.slot.config.minSize
        case .right:
            return splitView.frame.width - info.slot.config.maxSize
        case .bottom:
            return splitView.frame.height - info.slot.config.maxSize
        }
    }

    public func splitView(_ splitView: NSSplitView,
                          constrainMaxCoordinate proposedMaximumPosition: CGFloat,
                          ofSubviewAt dividerIndex: Int) -> CGFloat {
        guard let info = panelSlot(forDividerAt: dividerIndex, in: splitView) else {
            return proposedMaximumPosition
        }

        switch info.side {
        case .left:
            return info.slot.config.maxSize
        case .right:
            return splitView.frame.width - info.slot.config.minSize
        case .bottom:
            return splitView.frame.height - info.slot.config.minSize
        }
    }

    public func splitView(_ splitView: NSSplitView, canCollapseSubview subview: NSView) -> Bool {
        // Panel holders can collapse; content containers cannot
        return subview === leftPanelHolder || subview === rightPanelHolder || subview === bottomPanelHolder
    }

    public func splitViewDidResizeSubviews(_ notification: Notification) {
        // Track current sizes when user drags dividers
        guard let splitView = notification.object as? NSSplitView else { return }

        if splitView === horizontalSplitView {
            if let id = positionMap[.left], let slot = panels[id], slot.isVisible {
                slot.currentSize = leftPanelHolder.frame.width
            }
            if let id = positionMap[.right], let slot = panels[id], slot.isVisible {
                slot.currentSize = rightPanelHolder.frame.width
            }
        } else if splitView === verticalSplitView {
            if let id = positionMap[.bottom], let slot = panels[id], slot.isVisible {
                slot.currentSize = bottomPanelHolder.frame.height
            }
        }
    }
}

#endif
