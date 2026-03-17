#if os(macOS)
import AppKit
import os.log

public enum PanelPosition: String {
    case left
    case right
    case bottom
}

public struct PanelConfig {
    public let id: String
    public let position: PanelPosition
    public let defaultSize: CGFloat

    public init(id: String, position: PanelPosition, defaultSize: CGFloat = 320) {
        self.id = id
        self.position = position
        self.defaultSize = defaultSize
    }
}

/// One panel card — a floating sibling of the WebView card in the window background.
@MainActor
private class PanelSlot {
    let config: PanelConfig

    /// Shadow-casting wrapper (masksToBounds = false).
    let shadowWrapper: NSView
    /// Visual surface: NSVisualEffectView for blur + automatic dark/light handling.
    let blurView: NSVisualEffectView
    /// Content container where the panel's WebView is attached.
    let containerView: NSView

    var isVisible: Bool = false
    var currentSize: CGFloat

    init(config: PanelConfig, cornerRadius: CGFloat) {
        self.config = config
        self.currentSize = config.defaultSize

        shadowWrapper = NSView()
        shadowWrapper.wantsLayer = true
        shadowWrapper.layer?.masksToBounds = false

        blurView = NSVisualEffectView()
        blurView.material = .contentBackground
        blurView.blendingMode = .behindWindow
        blurView.state = .active
        blurView.wantsLayer = true
        blurView.layer?.cornerRadius = cornerRadius
        blurView.layer?.masksToBounds = true
        blurView.translatesAutoresizingMaskIntoConstraints = false

        containerView = NSView()
        containerView.wantsLayer = true
        containerView.translatesAutoresizingMaskIntoConstraints = false

        shadowWrapper.addSubview(blurView)
        blurView.addSubview(containerView)

        NSLayoutConstraint.activate([
            blurView.topAnchor.constraint(equalTo: shadowWrapper.topAnchor),
            blurView.leadingAnchor.constraint(equalTo: shadowWrapper.leadingAnchor),
            blurView.trailingAnchor.constraint(equalTo: shadowWrapper.trailingAnchor),
            blurView.bottomAnchor.constraint(equalTo: shadowWrapper.bottomAnchor),
            containerView.topAnchor.constraint(equalTo: blurView.topAnchor),
            containerView.leadingAnchor.constraint(equalTo: blurView.leadingAnchor),
            containerView.trailingAnchor.constraint(equalTo: blurView.trailingAnchor),
            containerView.bottomAnchor.constraint(equalTo: blurView.bottomAnchor),
        ])
    }

    func applyShadow(for position: PanelPosition) {
        shadowWrapper.layer?.shadowColor = NSColor.black.cgColor
        shadowWrapper.layer?.shadowOpacity = 0.15
        shadowWrapper.layer?.shadowRadius = 8
        switch position {
        case .right:  shadowWrapper.layer?.shadowOffset = CGSize(width: -3, height: 0)
        case .left:   shadowWrapper.layer?.shadowOffset = CGSize(width:  3, height: 0)
        case .bottom: shadowWrapper.layer?.shadowOffset = CGSize(width:  0, height: 3)
        }
    }
}

/// Manages the content area (WebView card interior) and floating panel cards.
///
/// ## Layout model
///
/// ```
/// contentView (window background)
///   ├── base            — dark/light fill
///   ├── sidebar         — icon strip
///   ├── shadowWrapper   — WebView card (shrinks when panels open)
///   │     └── right     — corner-radius clip, contains rootView
///   ├── panel cards     — sibling cards at same depth as WebView card  ← this file
///   └── sidebarRevealButton
/// ```
///
/// When a panel opens the WebView card shrinks to make room; both the panel slide-in
/// and the card resize animate together. Neither card ever overlaps the other.
///
/// ## Panel position constraints in contentView
///
/// | Position | Card anchored to           |
/// |----------|----------------------------|
/// | right    | contentView trailing edge  |
/// | left     | sidebar trailing edge      |
/// | bottom   | contentView bottom edge    |
///
/// ## Multi-panel
/// One active panel per position (mutual exclusion). Opening a second panel at the same
/// position closes the first with a simultaneous animation.
@MainActor
public class WorkspaceManager: NSObject {

    private static let log = OSLog(subsystem: "LingXia", category: "Workspace")
    private static let animationDuration: TimeInterval = 0.22
    private static let cornerRadius: CGFloat = 10

    /// Main content view; active ViewController's view is placed here.
    public let contentContainer = NSView()

    /// Toolbar + contentContainer wrapper; placed inside the WebView card by WindowController.
    public let centerPanelView = NSView()

    public var rootView: NSView { centerPanelView }

    private weak var overlayParent: NSView?
    private weak var sidebarRef: NSView?
    private var padding: CGFloat = 6

    /// Called inside an animation block whenever panel state changes.
    /// WindowController updates its WebView card edge constraints in this callback.
    /// Parameters: (trailingInset, bottomInset)
    public var onCardEdgesChanged: ((_ trailing: CGFloat, _ bottom: CGFloat) -> Void)?

    private var panels: [String: PanelSlot] = [:]
    /// At most one active panel per position.
    private var activeByPosition: [PanelPosition: String] = [:]

    override init() {
        super.init()
        centerPanelView.wantsLayer = true
        contentContainer.wantsLayer = true
        contentContainer.translatesAutoresizingMaskIntoConstraints = false
        centerPanelView.addSubview(contentContainer)
    }

    /// Must be called once by WindowController after the sidebar is placed.
    ///
    /// - Parameters:
    ///   - overlayParent: The window's `contentView` — panel cards are added here.
    ///   - sidebar: Used as leading anchor for left-panel positioning.
    ///   - padding: Edge inset matching the WebView card's padding (keeps gaps consistent).
    ///   - onCardEdgesChanged: Called inside each animation block with the new trailing and
    ///     bottom insets. WindowController updates its own constraints here; WorkspaceManager
    ///     then calls `layoutSubtreeIfNeeded()` so both animate together.
    public func configure(
        overlayParent: NSView,
        sidebar: NSView,
        padding: CGFloat,
        onCardEdgesChanged: @escaping (_ trailing: CGFloat, _ bottom: CGFloat) -> Void
    ) {
        self.overlayParent = overlayParent
        self.sidebarRef = sidebar
        self.padding = padding
        self.onCardEdgesChanged = onCardEdgesChanged
    }

    /// Constrains `contentContainer` to fill `centerPanelView` below the toolbar.
    public func attachBelowToolbar(_ toolbarView: NSView) {
        NSLayoutConstraint.activate([
            contentContainer.topAnchor.constraint(equalTo: toolbarView.bottomAnchor),
            contentContainer.leadingAnchor.constraint(equalTo: centerPanelView.leadingAnchor),
            contentContainer.trailingAnchor.constraint(equalTo: centerPanelView.trailingAnchor),
            contentContainer.bottomAnchor.constraint(equalTo: centerPanelView.bottomAnchor),
        ])
    }


    public func isPanelRegistered(id: String) -> Bool { panels[id] != nil }

    /// Register a panel. Creates the card view and positions it off-screen.
    /// Returns the container view where WebViews should be attached.
    @discardableResult
    public func registerPanel(_ config: PanelConfig) -> NSView {
        if let existing = panels[config.id] { return existing.containerView }

        let slot = PanelSlot(config: config, cornerRadius: Self.cornerRadius)
        slot.applyShadow(for: config.position)
        panels[config.id] = slot

        guard let parent = overlayParent, let sidebar = sidebarRef else {
            os_log("registerPanel: configure() not called yet — panel %@ deferred", log: Self.log, type: .error, config.id)
            return slot.containerView
        }

        installCard(slot, in: parent, sidebar: sidebar)
        return slot.containerView
    }

    /// Show a panel, animating it in and shrinking the WebView card.
    public func showPanel(id: String) {
        guard let slot = panels[id], !slot.isVisible else { return }

        // Mutual exclusion: close the current panel at this position first (no separate animation —
        // it exits while the new one enters simultaneously).
        if let currentId = activeByPosition[slot.config.position], currentId != id {
            forceHide(id: currentId)
        }
        activeByPosition[slot.config.position] = id
        slot.isVisible = true

        let offset = exitOffset(for: slot)
        slot.shadowWrapper.isHidden = false
        slot.shadowWrapper.layer?.transform = CATransform3DMakeTranslation(offset.x, offset.y, 0)

        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = Self.animationDuration
            ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
            ctx.allowsImplicitAnimation = true
            slot.shadowWrapper.layer?.transform = CATransform3DIdentity
            onCardEdgesChanged?(cardTrailingInset(), cardBottomInset())
            overlayParent?.layoutSubtreeIfNeeded()
        }
        os_log("Panel shown: %@", log: Self.log, type: .info, id)
    }

    /// Hide a panel, animating it out and restoring the WebView card's space.
    public func hidePanel(id: String) {
        guard let slot = panels[id], slot.isVisible else { return }
        hidePanelInternal(id: id, duration: Self.animationDuration, updateCardEdges: true)
        os_log("Panel hidden: %@", log: Self.log, type: .info, id)
    }

    public func togglePanel(id: String) {
        guard let slot = panels[id] else { return }
        slot.isVisible ? hidePanel(id: id) : showPanel(id: id)
    }

    public func isPanelVisible(id: String) -> Bool { panels[id]?.isVisible ?? false }

    public func panelContainer(id: String) -> NSView? { panels[id]?.containerView }

    /// Place the panel card in `overlayParent` at its final AutoLayout position.
    /// It starts hidden; `showPanel` reveals it with a transform animation.
    private func installCard(_ slot: PanelSlot, in parent: NSView, sidebar: NSView) {
        let wrapper = slot.shadowWrapper
        wrapper.translatesAutoresizingMaskIntoConstraints = false
        wrapper.isHidden = true

        // Insert above the WebView card shadow wrapper but below sidebarRevealButton
        parent.addSubview(wrapper)

        let p = padding
        let size = slot.currentSize

        switch slot.config.position {
        case .right:
            NSLayoutConstraint.activate([
                wrapper.topAnchor.constraint(equalTo: parent.topAnchor, constant: p),
                wrapper.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                wrapper.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -p),
                wrapper.widthAnchor.constraint(equalToConstant: size),
            ])

        case .left:
            NSLayoutConstraint.activate([
                wrapper.topAnchor.constraint(equalTo: parent.topAnchor, constant: p),
                wrapper.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                wrapper.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: p),
                wrapper.widthAnchor.constraint(equalToConstant: size),
            ])

        case .bottom:
            NSLayoutConstraint.activate([
                wrapper.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: p),
                wrapper.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -p),
                wrapper.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                wrapper.heightAnchor.constraint(equalToConstant: size),
            ])
        }

        parent.layoutSubtreeIfNeeded()
    }

    /// Returns the inset the WebView card should apply for a given panel position.
    private func cardInset(for position: PanelPosition) -> CGFloat {
        let p = padding
        guard let id = activeByPosition[position], let slot = panels[id], slot.isVisible else { return p }
        return p + slot.currentSize + p
    }

    private func cardTrailingInset() -> CGFloat { cardInset(for: .right) }
    private func cardBottomInset() -> CGFloat { cardInset(for: .bottom) }

    /// The translation that places a panel fully off-screen (used as start/end of animation).
    private func exitOffset(for slot: PanelSlot) -> (x: CGFloat, y: CGFloat) {
        let size = slot.currentSize + padding
        switch slot.config.position {
        case .right:  return (x: size, y: 0)
        case .left:   return (x: -size, y: 0)
        case .bottom: return (x: 0, y: -size)
        }
    }

    /// Immediately exit without updating card edges — used when replacing a panel at the same position.
    private func forceHide(id: String) {
        hidePanelInternal(id: id, duration: Self.animationDuration * 0.5, updateCardEdges: false)
    }

    /// Shared exit animation for hidePanel and forceHide.
    private func hidePanelInternal(id: String, duration: TimeInterval, updateCardEdges: Bool) {
        guard let slot = panels[id] else { return }
        slot.isVisible = false
        if activeByPosition[slot.config.position] == id {
            activeByPosition.removeValue(forKey: slot.config.position)
        }
        let offset = exitOffset(for: slot)
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = duration
            ctx.timingFunction = CAMediaTimingFunction(name: .easeIn)
            ctx.allowsImplicitAnimation = true
            slot.shadowWrapper.layer?.transform = CATransform3DMakeTranslation(offset.x, offset.y, 0)
            if updateCardEdges {
                onCardEdgesChanged?(cardTrailingInset(), cardBottomInset())
                overlayParent?.layoutSubtreeIfNeeded()
            }
        } completionHandler: { [weak slot] in
            Task { @MainActor in
                guard let slot, !slot.isVisible else { return }
                slot.shadowWrapper.isHidden = true
                slot.shadowWrapper.layer?.transform = CATransform3DIdentity
            }
        }
    }
}

#endif
