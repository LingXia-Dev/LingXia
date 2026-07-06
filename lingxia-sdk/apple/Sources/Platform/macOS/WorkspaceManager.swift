#if os(macOS)
import AppKit
import CLingXiaRustAPI
import os.log

private let lxWorkspaceTerminalOSLog = OSLog(subsystem: "LingXia", category: "TerminalWorkspace")

private func lxWorkspaceStdoutLog(_ message: String) {
    if ProcessInfo.processInfo.environment["LX_TERMINAL_DEBUG_LOGS"] == "1" {
        os_log("%{public}@", log: lxWorkspaceTerminalOSLog, type: .info, message)
    }
    guard ProcessInfo.processInfo.environment["LX_TERMINAL_STDOUT_LOGS"] == "1" else {
        return
    }
    let line = "[LingXia][Workspace] \(message)\n"
    FileHandle.standardOutput.write(Data(line.utf8))
    NSLog("%@", line.trimmingCharacters(in: .newlines))
}

private func lxWorkspaceFormatRect(_ rect: NSRect) -> String {
    String(
        format: "%.0f,%.0f %.0fx%.0f",
        rect.minX,
        rect.minY,
        rect.width,
        rect.height
    )
}

enum PanelPosition: String {
    case left
    case right
    case top
    case bottom
}

struct PanelConfig {
    let id: String
    let position: PanelPosition
    let defaultSize: CGFloat

    init(id: String, position: PanelPosition, defaultSize: CGFloat = 320) {
        self.id = id
        self.position = position
        self.defaultSize = defaultSize
    }
}

// MARK: - PanelResizeHandle

/// Draggable edge handle placed as a sibling of the panel card in contentView.
/// Positioned in the gap between the WebView card and the panel card.
@MainActor
private class PanelResizeHandle: NSView {
    private let position: PanelPosition
    var onDrag: ((CGFloat) -> Void)?
    var onDragEnd: ((CGFloat) -> Void)?
    var currentSizeProvider: (() -> CGFloat)?

    private var initialPoint: CGPoint = .zero
    private var initialSize: CGFloat = 0

    init(position: PanelPosition) {
        self.position = position
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) { fatalError() }

    override var mouseDownCanMoveWindow: Bool { false }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: (position == .bottom || position == .top) ? .resizeUpDown : .resizeLeftRight)
    }

    override func mouseDown(with event: NSEvent) {
        initialPoint = event.locationInWindow
        initialSize = currentSizeProvider?() ?? 0
    }

    override func mouseDragged(with event: NSEvent) {
        onDrag?(newSize(for: event))
    }

    override func mouseUp(with event: NSEvent) {
        onDragEnd?(newSize(for: event))
    }

    private func newSize(for event: NSEvent) -> CGFloat {
        let loc = event.locationInWindow
        let delta: CGFloat
        switch position {
        case .right:  delta = initialPoint.x - loc.x   // drag left  → expand
        case .left:   delta = loc.x - initialPoint.x   // drag right → expand
        case .bottom: delta = loc.y - initialPoint.y   // drag up    → expand
        case .top:    delta = initialPoint.y - loc.y   // drag down  → expand
        }
        return initialSize + delta
    }
}

// MARK: - PanelSlot

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
    /// Drag handle placed as a sibling of shadowWrapper in overlayParent.
    let resizeHandle: PanelResizeHandle
    /// Width (left/right panels) or height (bottom panel) constraint — updated on resize drag.
    var sizeConstraint: NSLayoutConstraint?
    /// Side panels: the anchor to the window edge (right: trailing, left:
    /// leading). Its constant shifts inward when siblings share the edge so
    /// panels sit side by side.
    var edgeConstraint: NSLayoutConstraint?
    /// Side panels: the docked top/bottom anchors, deactivated while
    /// fullscreen so the panel can cover the content pane exactly.
    var sideDockConstraints: [NSLayoutConstraint] = []
    /// Side panels: covers the content pane (flush against the sidebar, its
    /// vertical extent, out to the window edge) while fullscreen.
    var sideFullscreenConstraints: [NSLayoutConstraint] = []

    var isVisible: Bool = false
    var currentSize: CGFloat
    var isFullscreen: Bool = false

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

        resizeHandle = PanelResizeHandle(position: config.position)
        resizeHandle.translatesAutoresizingMaskIntoConstraints = false

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
        case .top:    shadowWrapper.layer?.shadowOffset = CGSize(width:  0, height: -3)
        }
    }
}

// MARK: - WorkspaceManager

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
///   ├── panel handles   — sibling resize handles, one per panel, in the gap
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
class WorkspaceManager: NSObject {

    private static let log = OSLog(subsystem: "LingXia", category: "Workspace")
    private static let animationDuration: TimeInterval = 0.22
    private static let cornerRadius: CGFloat = 10
    private static let panelMinSize: CGFloat = 160
    /// Fallback cap used only before the window geometry is known; once laid
    /// out, a panel may grow until the main region hits its minimum.
    private static let panelFallbackMaxSize: CGFloat = 700
    private static let handleSize: CGFloat = 5
    private static let minMainRegionWidth: CGFloat = 320
    private static let minMainRegionHeight: CGFloat = 240

    /// Main content view; active ViewController's view is placed here.
    let contentContainer = NSView()

    /// Toolbar + contentContainer wrapper; placed inside the WebView card by WindowController.
    let workspaceView = NSView()

    var rootView: NSView { workspaceView }

    private weak var overlayParent: NSView?
    private weak var sidebarRef: NSView?
    private var padding: CGFloat = 6

    /// Called inside an animation block whenever panel state changes.
    /// WindowController updates its WebView card edge constraints in this callback.
    /// Parameters: (trailingInset, bottomInset, topInset, leadingInset)
    var onCardEdgesChanged: ((_ trailing: CGFloat, _ bottom: CGFloat, _ top: CGFloat, _ leading: CGFloat) -> Void)?

    private var panels: [String: PanelSlot] = [:]
    /// At most one active panel per position.
    /// Visible panels per edge, ordered outermost → innermost. Side edges
    /// (left/right) support several panels side by side; the frame-driven
    /// bottom/top edges keep a single visible panel.
    private var visibleOrder: [PanelPosition: [String]] = [:]
    /// The main content card's toolbar, used only to place the lxapp content
    /// below native navigation chrome inside the main card.
    private weak var toolbarRef: NSView?

    override init() {
        super.init()
        workspaceView.wantsLayer = true
        contentContainer.wantsLayer = true
        contentContainer.translatesAutoresizingMaskIntoConstraints = false
        workspaceView.addSubview(contentContainer)
    }

    /// Must be called once by WindowController after the sidebar is placed.
    func configure(
        overlayParent: NSView,
        sidebar: NSView,
        padding: CGFloat,
        onCardEdgesChanged: @escaping (_ trailing: CGFloat, _ bottom: CGFloat, _ top: CGFloat, _ leading: CGFloat) -> Void
    ) {
        self.overlayParent = overlayParent
        self.sidebarRef = sidebar
        self.padding = padding
        self.onCardEdgesChanged = onCardEdgesChanged
        lxWorkspaceStdoutLog(
            "configure parentBounds=\(lxWorkspaceFormatRect(overlayParent.bounds)) sidebarFrame=\(lxWorkspaceFormatRect(sidebar.frame)) padding=\(String(format: "%.1f", padding))"
        )
    }

    /// Constrains `contentContainer` to fill `workspaceView` below the toolbar.
    func attachBelowToolbar(_ toolbarView: NSView) {
        toolbarRef = toolbarView
        NSLayoutConstraint.activate([
            contentContainer.topAnchor.constraint(equalTo: toolbarView.bottomAnchor),
            contentContainer.leadingAnchor.constraint(equalTo: workspaceView.leadingAnchor),
            contentContainer.trailingAnchor.constraint(equalTo: workspaceView.trailingAnchor),
            contentContainer.bottomAnchor.constraint(equalTo: workspaceView.bottomAnchor),
        ])
    }

    func isPanelRegistered(id: String) -> Bool { panels[id] != nil }

    /// Register a panel. Creates the card view and positions it off-screen.
    /// Returns the container view where WebViews should be attached.
    @discardableResult
    func registerPanel(_ config: PanelConfig) -> NSView {
        if let existing = panels[config.id] {
            lxWorkspaceStdoutLog(
                "registerPanel existing id=\(config.id) position=\(config.position.rawValue) visible=\(existing.isVisible) currentSize=\(String(format: "%.1f", existing.currentSize)) wrapperFrame=\(lxWorkspaceFormatRect(existing.shadowWrapper.frame))"
            )
            return existing.containerView
        }

        let slot = PanelSlot(config: config, cornerRadius: Self.cornerRadius)
        slot.applyShadow(for: config.position)
        panels[config.id] = slot
        lxWorkspaceStdoutLog(
            "registerPanel new id=\(config.id) position=\(config.position.rawValue) defaultSize=\(String(format: "%.1f", config.defaultSize)) currentSize=\(String(format: "%.1f", slot.currentSize))"
        )

        let slotId = config.id
        slot.resizeHandle.currentSizeProvider = { [weak slot] in slot?.currentSize ?? config.defaultSize }
        let resizeHandler: (CGFloat) -> Void = { [weak self] newSize in self?.updatePanelSize(id: slotId, newSize: newSize) }
        slot.resizeHandle.onDrag = resizeHandler
        slot.resizeHandle.onDragEnd = resizeHandler

        guard let parent = overlayParent, let sidebar = sidebarRef else {
            LXLog.error("registerPanel: configure() not called yet — panel \(config.id) deferred", category: "Workspace")
            lxWorkspaceStdoutLog("registerPanel deferred id=\(config.id) missing parent/sidebar")
            return slot.containerView
        }

        lxWorkspaceStdoutLog(
            "registerPanel install id=\(config.id) parentBounds=\(lxWorkspaceFormatRect(parent.bounds)) sidebarFrame=\(lxWorkspaceFormatRect(sidebar.frame))"
        )
        installCard(slot, in: parent, sidebar: sidebar)
        lxWorkspaceStdoutLog(
            "registerPanel installed id=\(config.id) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame)) handleFrame=\(lxWorkspaceFormatRect(slot.resizeHandle.frame)) containerBounds=\(lxWorkspaceFormatRect(slot.containerView.bounds))"
        )
        return slot.containerView
    }

    /// Show a panel, animating it in and shrinking the WebView card.
    func showPanel(id: String) {
        guard let slot = panels[id] else {
            lxWorkspaceStdoutLog("showPanel missing id=\(id)")
            return
        }
        guard !slot.isVisible else {
            lxWorkspaceStdoutLog(
                "showPanel already-visible id=\(id) position=\(slot.config.position.rawValue) currentSize=\(String(format: "%.1f", slot.currentSize)) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame))"
            )
            return
        }
        lxWorkspaceStdoutLog(
            "showPanel start id=\(id) position=\(slot.config.position.rawValue) currentSize=\(String(format: "%.1f", slot.currentSize)) parentBounds=\(lxWorkspaceFormatRect(overlayParent?.bounds ?? .zero)) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame))"
        )

        // Clamp against current window size so panel cannot consume the whole main region.
        let normalizedSize = clampedPanelSize(slot.currentSize, for: slot.config.position, excluding: id)
        if normalizedSize != slot.currentSize {
            lxWorkspaceStdoutLog(
                "showPanel normalized id=\(id) from=\(String(format: "%.1f", slot.currentSize)) to=\(String(format: "%.1f", normalizedSize))"
            )
            slot.currentSize = normalizedSize
            slot.sizeConstraint?.constant = normalizedSize
        }
        let position = slot.config.position
        if position == .bottom || position == .top {
            // Frame-driven edges hold a single panel; a newcomer replaces it.
            for currentId in visibleOrder[position] ?? [] where currentId != id {
                forceHide(id: currentId)
            }
            visibleOrder[position] = [id]
        } else {
            // Side edges stack panels side by side (newest innermost).
            var order = visibleOrder[position] ?? []
            if !order.contains(id) { order.append(id) }
            visibleOrder[position] = order
        }
        slot.isVisible = true
        relayoutSideChain(position)

        let offset = exitOffset(for: slot)
        slot.shadowWrapper.isHidden = false
        slot.resizeHandle.isHidden = slot.isFullscreen
        if slot.isFullscreen {
            bringPanelToFront(slot)
        }
        slot.shadowWrapper.layer?.transform = CATransform3DMakeTranslation(offset.x, offset.y, 0)

        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = Self.animationDuration
            ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
            ctx.allowsImplicitAnimation = true
            slot.shadowWrapper.layer?.transform = CATransform3DIdentity
            lxWorkspaceStdoutLog(
                "showPanel animate id=\(id) trailingInset=\(String(format: "%.1f", cardTrailingInset())) bottomInset=\(String(format: "%.1f", cardBottomInset()))"
            )
            onCardEdgesChanged?(cardTrailingInset(), cardBottomInset(), cardTopInset(), cardLeadingInset())
            overlayParent?.layoutSubtreeIfNeeded()
            layoutFrameDrivenPanels()
        }
        os_log("Panel shown: %@ trailing=%.1f bottom=%.1f", log: Self.log, type: .info, id, cardTrailingInset(), cardBottomInset())
        DispatchQueue.main.async { [weak self, weak slot] in
            guard let self, let slot else { return }
            self.layoutFrameDrivenPanels()
            lxWorkspaceStdoutLog(
                "showPanel after-layout id=\(id) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame)) handleFrame=\(lxWorkspaceFormatRect(slot.resizeHandle.frame)) parentBounds=\(lxWorkspaceFormatRect(self.overlayParent?.bounds ?? .zero)) trailingInset=\(String(format: "%.1f", self.cardTrailingInset())) bottomInset=\(String(format: "%.1f", self.cardBottomInset()))"
            )
        }
    }

    /// Hide a panel, animating it out and restoring the WebView card's space.
    func hidePanel(id: String) {
        guard let slot = panels[id], slot.isVisible else {
            lxWorkspaceStdoutLog("hidePanel ignored id=\(id)")
            return
        }
        lxWorkspaceStdoutLog(
            "hidePanel start id=\(id) position=\(slot.config.position.rawValue) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame))"
        )
        hidePanelInternal(id: id, duration: Self.animationDuration, updateCardEdges: true)
        os_log("Panel hidden: %@", log: Self.log, type: .info, id)
    }

    func togglePanel(id: String) {
        guard let slot = panels[id] else {
            lxWorkspaceStdoutLog("togglePanel missing id=\(id)")
            return
        }
        lxWorkspaceStdoutLog("togglePanel id=\(id) visible=\(slot.isVisible) position=\(slot.config.position.rawValue)")
        slot.isVisible ? hidePanel(id: id) : showPanel(id: id)
    }

    func isPanelVisible(id: String) -> Bool { panels[id]?.isVisible ?? false }

    /// Ids of registered panels that are currently visible. Every registered
    /// panel is an aside under the aside-layout reconciler's authority (main
    /// content lives in `contentContainer`, not the panel registry), so this is
    /// the reconciler's "currently-placed asides" set — derived from the
    /// view-registry rather than a private mirror.
    func visiblePanelIds() -> Set<String> {
        Set(panels.compactMap { $0.value.isVisible ? $0.key : nil })
    }

    /// The edge a registered panel is currently docked to, or `nil` if unknown.
    /// The aside-layout reconciler reads this to decide whether a panel needs to
    /// be re-placed at the core tree's edge.
    func panelPosition(id: String) -> PanelPosition? { panels[id]?.config.position }

    /// Move a registered panel to a different edge, preserving its attached
    /// content view. No-op when the panel is already at `position`. Used by the
    /// aside-layout reconciler (the sole placement authority) when the core's
    /// tree edge differs from the edge the content path registered the panel at.
    /// The panel is left hidden after a move; the caller re-shows it.
    func repositionPanel(id: String, to position: PanelPosition) {
        guard let old = panels[id] else {
            lxWorkspaceStdoutLog("repositionPanel missing id=\(id)")
            return
        }
        guard old.config.position != position else { return }
        lxWorkspaceStdoutLog(
            "repositionPanel id=\(id) from=\(old.config.position.rawValue) to=\(position.rawValue)"
        )

        // Detach the panel's content view so it survives the slot swap.
        let content = old.containerView.subviews
        for view in content { view.removeFromSuperview() }

        // Tear the old slot's cards out of the dock and forget it.
        if old.isVisible {
            hidePanelInternal(id: id, duration: 0, updateCardEdges: true)
        }
        old.shadowWrapper.removeFromSuperview()
        old.resizeHandle.removeFromSuperview()
        panels.removeValue(forKey: id)
        visibleOrder[old.config.position]?.removeAll { $0 == id }
        relayoutSideChain(old.config.position)

        // Re-register at the new edge, preserving the prior size, and re-attach
        // the content into the fresh container.
        let config = PanelConfig(id: id, position: position, defaultSize: old.config.defaultSize)
        let container = registerPanel(config)
        panels[id]?.currentSize = old.currentSize
        panels[id]?.sizeConstraint?.constant = old.currentSize
        for view in content {
            view.translatesAutoresizingMaskIntoConstraints = false
            container.addSubview(view)
            NSLayoutConstraint.activate([
                view.topAnchor.constraint(equalTo: container.topAnchor),
                view.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                view.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                view.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            ])
        }
    }

    func panelContainer(id: String) -> NSView? {
        guard let slot = panels[id] else {
            lxWorkspaceStdoutLog("panelContainer missing id=\(id)")
            return nil
        }
        lxWorkspaceStdoutLog(
            "panelContainer id=\(id) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame)) containerFrame=\(lxWorkspaceFormatRect(slot.containerView.frame)) containerBounds=\(lxWorkspaceFormatRect(slot.containerView.bounds)) visible=\(slot.isVisible)"
        )
        return slot.containerView
    }

    func relayoutPanels() {
        layoutFrameDrivenPanels()
    }

    /// Whether `id`'s panel is currently expanded fullscreen over the main area.
    func isPanelFullscreen(id: String) -> Bool {
        panels[id]?.isFullscreen ?? false
    }

    func setPanelFullscreen(id: String, enabled: Bool) {
        guard let slot = panels[id] else {
            lxWorkspaceStdoutLog("setPanelFullscreen missing id=\(id)")
            return
        }
        guard slot.isFullscreen != enabled else {
            lxWorkspaceStdoutLog("setPanelFullscreen no-op id=\(id) enabled=\(enabled)")
            return
        }

        slot.isFullscreen = enabled
        lxWorkspaceStdoutLog("setPanelFullscreen id=\(id) enabled=\(enabled) visible=\(slot.isVisible)")
        if slot.config.position == .left || slot.config.position == .right {
            applySideFullscreen(slot, enabled: enabled)
        }
        if enabled {
            bringPanelToFront(slot)
        }

        if slot.isVisible {
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = Self.animationDuration
                ctx.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                ctx.allowsImplicitAnimation = true
                relayoutSideChain(slot.config.position)
                onCardEdgesChanged?(cardTrailingInset(), cardBottomInset(), cardTopInset(), cardLeadingInset())
                overlayParent?.layoutSubtreeIfNeeded()
                layoutFrameDrivenPanels()
            }
        } else {
            relayoutSideChain(slot.config.position)
            onCardEdgesChanged?(cardTrailingInset(), cardBottomInset(), cardTopInset(), cardLeadingInset())
            layoutFrameDrivenPanels()
        }
    }

    /// Swap a side panel between its docked constraints and covering the
    /// content pane exactly (flush against the sidebar, its vertical extent,
    /// out to the window edge — no sliver), matching bottom/top expand.
    private func applySideFullscreen(_ slot: PanelSlot, enabled: Bool) {
        guard let parent = overlayParent, let sidebar = sidebarRef else { return }
        if enabled {
            if slot.sideFullscreenConstraints.isEmpty {
                let wrapper = slot.shadowWrapper
                slot.sideFullscreenConstraints = [
                    wrapper.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor),
                    wrapper.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
                    wrapper.topAnchor.constraint(equalTo: sidebar.topAnchor),
                    wrapper.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor),
                ]
            }
            NSLayoutConstraint.deactivate(
                slot.sideDockConstraints + [slot.sizeConstraint, slot.edgeConstraint].compactMap { $0 })
            NSLayoutConstraint.activate(slot.sideFullscreenConstraints)
            slot.resizeHandle.isHidden = true
        } else {
            NSLayoutConstraint.deactivate(slot.sideFullscreenConstraints)
            NSLayoutConstraint.activate(
                slot.sideDockConstraints + [slot.sizeConstraint, slot.edgeConstraint].compactMap { $0 })
            slot.resizeHandle.isHidden = !slot.isVisible
        }
    }

    // MARK: - Resize

    /// Update panel size in real-time during drag. No animation.
    private func updatePanelSize(id: String, newSize: CGFloat) {
        guard let slot = panels[id] else { return }
        let clamped = clampedPanelSize(newSize, for: slot.config.position, excluding: id)
        lxWorkspaceStdoutLog(
            "resizePanel id=\(id) requested=\(String(format: "%.1f", newSize)) clamped=\(String(format: "%.1f", clamped)) position=\(slot.config.position.rawValue)"
        )
        slot.currentSize = clamped
        slot.sizeConstraint?.constant = clamped
        relayoutSideChain(slot.config.position)
        onCardEdgesChanged?(cardTrailingInset(), cardBottomInset(), cardTopInset(), cardLeadingInset())
        overlayParent?.layoutSubtreeIfNeeded()
        layoutFrameDrivenPanels()
    }

    // MARK: - Private

    /// Place the panel card and its resize handle in `overlayParent`.
    /// The handle lives as a sibling of the card in the gap between card and WebView.
    private func installCard(_ slot: PanelSlot, in parent: NSView, sidebar: NSView) {
        let wrapper = slot.shadowWrapper
        wrapper.translatesAutoresizingMaskIntoConstraints = false
        wrapper.isHidden = true

        let handle = slot.resizeHandle
        handle.isHidden = true

        parent.addSubview(wrapper)
        parent.addSubview(handle)

        let p = padding
        let h = Self.handleSize
        let size = slot.currentSize
        // Side panels belong to the stable workspace shell, not to the main
        // card. Top/bottom panels shrink the main card; they must not move a
        // left/right aside by pulling on the main card's toolbar anchor.
        let sideTopAnchor = sidebar.topAnchor
        let sideTopInset = p

        switch slot.config.position {
        case .right:
            let wc = wrapper.widthAnchor.constraint(equalToConstant: size)
            slot.sizeConstraint = wc
            let edge = wrapper.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -p)
            slot.edgeConstraint = edge
            slot.sideDockConstraints = [
                wrapper.topAnchor.constraint(equalTo: sideTopAnchor, constant: sideTopInset),
                wrapper.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
            ]
            NSLayoutConstraint.activate(slot.sideDockConstraints + [
                edge,
                wc,
                // Handle sits in the gap to the left of the panel card
                handle.topAnchor.constraint(equalTo: sideTopAnchor, constant: sideTopInset),
                handle.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                handle.trailingAnchor.constraint(equalTo: wrapper.leadingAnchor),
                handle.widthAnchor.constraint(equalToConstant: h),
            ])

        case .left:
            let wc = wrapper.widthAnchor.constraint(equalToConstant: size)
            slot.sizeConstraint = wc
            let edge = wrapper.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: p)
            slot.edgeConstraint = edge
            slot.sideDockConstraints = [
                wrapper.topAnchor.constraint(equalTo: sideTopAnchor, constant: sideTopInset),
                wrapper.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
            ]
            NSLayoutConstraint.activate(slot.sideDockConstraints + [
                edge,
                wc,
                // Handle sits in the gap to the right of the panel card
                handle.topAnchor.constraint(equalTo: sideTopAnchor, constant: sideTopInset),
                handle.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                handle.leadingAnchor.constraint(equalTo: wrapper.trailingAnchor),
                handle.widthAnchor.constraint(equalToConstant: h),
            ])

        case .bottom:
            slot.sizeConstraint = nil
            wrapper.translatesAutoresizingMaskIntoConstraints = true
            handle.translatesAutoresizingMaskIntoConstraints = true
            wrapper.autoresizingMask = []
            handle.autoresizingMask = []
            lxWorkspaceStdoutLog(
                "installCard bottom frameDriven id=\(slot.config.id) size=\(String(format: "%.1f", size)) parentBounds=\(lxWorkspaceFormatRect(parent.bounds)) sidebarFrame=\(lxWorkspaceFormatRect(sidebar.frame)) padding=\(String(format: "%.1f", p))"
            )
            layoutBottomPanel(slot, in: parent, sidebar: sidebar)

        case .top:
            slot.sizeConstraint = nil
            wrapper.translatesAutoresizingMaskIntoConstraints = true
            handle.translatesAutoresizingMaskIntoConstraints = true
            wrapper.autoresizingMask = []
            handle.autoresizingMask = []
            lxWorkspaceStdoutLog(
                "installCard top frameDriven id=\(slot.config.id) size=\(String(format: "%.1f", size)) parentBounds=\(lxWorkspaceFormatRect(parent.bounds)) sidebarFrame=\(lxWorkspaceFormatRect(sidebar.frame)) padding=\(String(format: "%.1f", p))"
            )
            layoutTopPanel(slot, in: parent, sidebar: sidebar)
        }

        parent.layoutSubtreeIfNeeded()
        lxWorkspaceStdoutLog(
            "installCard complete id=\(slot.config.id) position=\(slot.config.position.rawValue) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame)) handleFrame=\(lxWorkspaceFormatRect(slot.resizeHandle.frame))"
        )
    }

    /// Visible slots on an edge, outermost first.
    private func visibleSlots(at position: PanelPosition) -> [PanelSlot] {
        (visibleOrder[position] ?? []).compactMap { panels[$0] }.filter(\.isVisible)
    }

    private func cardInset(for position: PanelPosition) -> CGFloat {
        let p = padding
        // A fullscreen panel overlays the card instead of pushing it.
        let slots = visibleSlots(at: position).filter { !$0.isFullscreen }
        guard !slots.isEmpty else { return p }
        // Every visible panel on the edge pushes the card further in.
        return slots.reduce(p) { $0 + clampedPanelSize($1.currentSize, for: position, excluding: $1.config.id) + p }
    }

    private func cardTrailingInset() -> CGFloat { cardInset(for: .right) }
    private func cardBottomInset() -> CGFloat { cardInset(for: .bottom) }
    private func cardTopInset() -> CGFloat { cardInset(for: .top) }

    /// Leading inset is 0 when no left panel is open (the content card sits flush
    /// against the sidebar), and `padding + panels + padding` when some are, so
    /// the card shrinks to exactly clear them.
    private func cardLeadingInset() -> CGFloat {
        let slots = visibleSlots(at: .left).filter { !$0.isFullscreen }
        guard !slots.isEmpty else { return 0 }
        return slots.reduce(padding) { $0 + clampedPanelSize($1.currentSize, for: .left, excluding: $1.config.id) + padding }
    }

    /// Re-chain a side edge: each visible panel's edge constraint shifts inward
    /// past the panels outside it, so they sit side by side.
    private func relayoutSideChain(_ position: PanelPosition) {
        guard position == .left || position == .right else { return }
        let p = padding
        var offset: CGFloat = p
        for slot in visibleSlots(at: position) {
            // A fullscreen panel covers the pane (its dock constraints are
            // swapped out) and consumes no chain room.
            if slot.isFullscreen {
                continue
            }
            slot.edgeConstraint?.constant = position == .right ? -offset : offset
            offset += clampedPanelSize(slot.currentSize, for: position, excluding: slot.config.id) + p
        }
    }

    private func bringPanelToFront(_ slot: PanelSlot) {
        guard let parent = overlayParent else { return }
        parent.addSubview(slot.shadowWrapper, positioned: .above, relativeTo: nil)
        parent.addSubview(slot.resizeHandle, positioned: .above, relativeTo: slot.shadowWrapper)
    }

    private func exitOffset(for slot: PanelSlot) -> (x: CGFloat, y: CGFloat) {
        let size = clampedPanelSize(slot.currentSize, for: slot.config.position) + padding
        switch slot.config.position {
        case .right:
            if slot.isFullscreen, let parent = overlayParent {
                return (x: parent.bounds.width + padding, y: 0)
            }
            return (x: size, y: 0)
        case .left:
            if slot.isFullscreen, let parent = overlayParent {
                return (x: -(parent.bounds.width + padding), y: 0)
            }
            return (x: -size, y: 0)
        case .bottom:
            if slot.isFullscreen, let parent = overlayParent {
                return (x: 0, y: -(parent.bounds.height + padding))
            }
            return (x: 0, y: -size)
        case .top:
            if slot.isFullscreen, let parent = overlayParent {
                return (x: 0, y: parent.bounds.height + padding)
            }
            return (x: 0, y: size)
        }
    }

    private func forceHide(id: String) {
        hidePanelInternal(id: id, duration: Self.animationDuration * 0.5, updateCardEdges: false)
    }

    private func hidePanelInternal(id: String, duration: TimeInterval, updateCardEdges: Bool) {
        guard let slot = panels[id] else { return }
        slot.isVisible = false
        visibleOrder[slot.config.position]?.removeAll { $0 == id }
        let offset = exitOffset(for: slot)
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = duration
            ctx.timingFunction = CAMediaTimingFunction(name: .easeIn)
            ctx.allowsImplicitAnimation = true
            slot.shadowWrapper.layer?.transform = CATransform3DMakeTranslation(offset.x, offset.y, 0)
            relayoutSideChain(slot.config.position)
            if updateCardEdges {
                onCardEdgesChanged?(cardTrailingInset(), cardBottomInset(), cardTopInset(), cardLeadingInset())
                overlayParent?.layoutSubtreeIfNeeded()
                layoutFrameDrivenPanels()
            }
        } completionHandler: { [weak slot] in
            Task { @MainActor in
                guard let slot, !slot.isVisible else { return }
                slot.shadowWrapper.isHidden = true
                slot.shadowWrapper.layer?.transform = CATransform3DIdentity
                slot.resizeHandle.isHidden = true
                lxWorkspaceStdoutLog(
                    "hidePanel complete id=\(slot.config.id) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame)) handleFrame=\(lxWorkspaceFormatRect(slot.resizeHandle.frame))"
                )
            }
        }
    }

    private func layoutFrameDrivenPanels() {
        guard let parent = overlayParent, let sidebar = sidebarRef else { return }
        for slot in panels.values {
            switch slot.config.position {
            case .bottom:
                layoutBottomPanel(slot, in: parent, sidebar: sidebar)
            case .top:
                layoutTopPanel(slot, in: parent, sidebar: sidebar)
            case .left, .right:
                break
            }
        }
    }

    private func layoutBottomPanel(_ slot: PanelSlot, in parent: NSView, sidebar: NSView) {
        let p = padding
        if slot.isFullscreen {
            // Expand fills the CONTENT pane exactly — flush against the sidebar,
            // matching its vertical extent, no padding — so it fully covers the
            // webview (no sliver) while the sidebar/switcher stays reachable
            // (switching mains then collapses the aside).
            let leading = max(0, sidebar.frame.maxX)
            slot.shadowWrapper.frame = NSRect(
                x: leading,
                y: sidebar.frame.minY,
                width: max(0, parent.bounds.width - leading),
                height: sidebar.frame.height
            )
            slot.resizeHandle.frame = .zero
            slot.resizeHandle.isHidden = true
        } else {
            // Bottom panel docks under the WebView card only — its trailing edge
            // aligns with the card's, so an active right panel sits next to the terminal rather than over it.
            let size = clampedPanelSize(slot.currentSize, for: .bottom)
            let leading = max(0, sidebar.frame.maxX + cardLeadingInset())
            let trailing = max(leading, parent.bounds.width - cardTrailingInset())
            let width = max(0, trailing - leading)
            let bottom = p
            slot.shadowWrapper.frame = NSRect(
                x: leading,
                y: bottom,
                width: width,
                height: size
            )
            slot.resizeHandle.frame = NSRect(
                x: leading,
                y: bottom + size,
                width: width,
                height: Self.handleSize
            )
            slot.resizeHandle.isHidden = !slot.isVisible
        }
        slot.shadowWrapper.layoutSubtreeIfNeeded()
        lxWorkspaceStdoutLog(
            "layoutBottomPanel id=\(slot.config.id) fullscreen=\(slot.isFullscreen) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame)) handleFrame=\(lxWorkspaceFormatRect(slot.resizeHandle.frame)) parentBounds=\(lxWorkspaceFormatRect(parent.bounds)) sidebarFrame=\(lxWorkspaceFormatRect(sidebar.frame)) visible=\(slot.isVisible)"
        )
    }

    private func layoutTopPanel(_ slot: PanelSlot, in parent: NSView, sidebar: NSView) {
        let p = padding
        if slot.isFullscreen {
            // Expand fills the CONTENT pane exactly — flush against the sidebar,
            // matching its vertical extent, no padding — so it fully covers the
            // webview (no sliver) while the sidebar/switcher stays reachable.
            let leading = max(0, sidebar.frame.maxX)
            slot.shadowWrapper.frame = NSRect(
                x: leading,
                y: sidebar.frame.minY,
                width: max(0, parent.bounds.width - leading),
                height: sidebar.frame.height
            )
            slot.resizeHandle.frame = .zero
            slot.resizeHandle.isHidden = true
            slot.shadowWrapper.layoutSubtreeIfNeeded()
            lxWorkspaceStdoutLog(
                "layoutTopPanel fullscreen id=\(slot.config.id) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame))"
            )
            return
        }
        // Top panel docks above the WebView card, spanning the same horizontal
        // band as the bottom panel (after the sidebar, before any right panel),
        // and its resize handle hangs just below its lower edge.
        let size = clampedPanelSize(slot.currentSize, for: .top)
        let leading = max(0, sidebar.frame.maxX + cardLeadingInset())
        let trailing = max(leading, parent.bounds.width - cardTrailingInset())
        let width = max(0, trailing - leading)
        let top = max(0, parent.bounds.height - p - size)
        slot.shadowWrapper.frame = NSRect(
            x: leading,
            y: top,
            width: width,
            height: size
        )
        slot.resizeHandle.frame = NSRect(
            x: leading,
            y: top - Self.handleSize,
            width: width,
            height: Self.handleSize
        )
        slot.resizeHandle.isHidden = !slot.isVisible
        slot.shadowWrapper.layoutSubtreeIfNeeded()
        lxWorkspaceStdoutLog(
            "layoutTopPanel id=\(slot.config.id) wrapperFrame=\(lxWorkspaceFormatRect(slot.shadowWrapper.frame)) handleFrame=\(lxWorkspaceFormatRect(slot.resizeHandle.frame)) parentBounds=\(lxWorkspaceFormatRect(parent.bounds)) sidebarFrame=\(lxWorkspaceFormatRect(sidebar.frame)) visible=\(slot.isVisible)"
        )
    }

    /// Clamp panel size by absolute bounds and current window space,
    /// so main webview region always keeps a minimum visible area.
    private func clampedPanelSize(
        _ requested: CGFloat,
        for position: PanelPosition,
        excluding excludedId: String? = nil
    ) -> CGFloat {
        let base = max(requested, Self.panelMinSize)
        guard let parent = overlayParent else {
            return min(base, Self.panelFallbackMaxSize)
        }

        let p = padding
        let maxByWindow: CGFloat
        switch position {
        case .right, .left:
            // Siblings sharing either side edge also eat into the width budget.
            let siblings = (visibleSlots(at: .left) + visibleSlots(at: .right))
                .filter { $0.config.id != excludedId }
                .reduce(CGFloat(0)) { $0 + $1.currentSize + p }
            maxByWindow = parent.bounds.width - (sidebarRef?.frame.width ?? 0) - p * 3
                - Self.minMainRegionWidth - siblings
        case .bottom, .top:
            maxByWindow = parent.bounds.height - p * 3 - Self.minMainRegionHeight
        }

        let result = min(base, max(Self.panelMinSize, maxByWindow))
        lxWorkspaceStdoutLog(
            "clampPanel position=\(position.rawValue) requested=\(String(format: "%.1f", requested)) base=\(String(format: "%.1f", base)) maxByWindow=\(String(format: "%.1f", maxByWindow)) result=\(String(format: "%.1f", result)) parentBounds=\(lxWorkspaceFormatRect(parent.bounds))"
        )
        return result
    }
}

#endif
