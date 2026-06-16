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
    private static let panelMaxSize: CGFloat = 700
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
    private var activeByPosition: [PanelPosition: String] = [:]

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
            os_log("registerPanel: configure() not called yet — panel %@ deferred", log: Self.log, type: .error, config.id)
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
        let normalizedSize = clampedPanelSize(slot.currentSize, for: slot.config.position)
        if normalizedSize != slot.currentSize {
            lxWorkspaceStdoutLog(
                "showPanel normalized id=\(id) from=\(String(format: "%.1f", slot.currentSize)) to=\(String(format: "%.1f", normalizedSize))"
            )
            slot.currentSize = normalizedSize
            slot.sizeConstraint?.constant = normalizedSize
        }
        if let currentId = activeByPosition[slot.config.position], currentId != id {
            forceHide(id: currentId)
        }
        activeByPosition[slot.config.position] = id
        slot.isVisible = true

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

    func setPanelFullscreen(id: String, enabled: Bool) {
        guard let slot = panels[id] else {
            lxWorkspaceStdoutLog("setPanelFullscreen missing id=\(id)")
            return
        }
        guard slot.config.position == .bottom else {
            lxWorkspaceStdoutLog("setPanelFullscreen ignored id=\(id) position=\(slot.config.position.rawValue)")
            return
        }
        guard slot.isFullscreen != enabled else {
            lxWorkspaceStdoutLog("setPanelFullscreen no-op id=\(id) enabled=\(enabled)")
            return
        }

        slot.isFullscreen = enabled
        lxWorkspaceStdoutLog("setPanelFullscreen id=\(id) enabled=\(enabled) visible=\(slot.isVisible)")
        if enabled {
            bringPanelToFront(slot)
        }

        if slot.isVisible {
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = Self.animationDuration
                ctx.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                ctx.allowsImplicitAnimation = true
                onCardEdgesChanged?(cardTrailingInset(), cardBottomInset(), cardTopInset(), cardLeadingInset())
                overlayParent?.layoutSubtreeIfNeeded()
                layoutFrameDrivenPanels()
            }
        } else {
            onCardEdgesChanged?(cardTrailingInset(), cardBottomInset(), cardTopInset(), cardLeadingInset())
            layoutFrameDrivenPanels()
        }
    }

    // MARK: - Resize

    /// Update panel size in real-time during drag. No animation.
    private func updatePanelSize(id: String, newSize: CGFloat) {
        guard let slot = panels[id] else { return }
        let clamped = clampedPanelSize(newSize, for: slot.config.position)
        lxWorkspaceStdoutLog(
            "resizePanel id=\(id) requested=\(String(format: "%.1f", newSize)) clamped=\(String(format: "%.1f", clamped)) position=\(slot.config.position.rawValue)"
        )
        slot.currentSize = clamped
        slot.sizeConstraint?.constant = clamped
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

        switch slot.config.position {
        case .right:
            let wc = wrapper.widthAnchor.constraint(equalToConstant: size)
            slot.sizeConstraint = wc
            NSLayoutConstraint.activate([
                wrapper.topAnchor.constraint(equalTo: parent.topAnchor, constant: p),
                wrapper.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                wrapper.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -p),
                wc,
                // Handle sits in the gap to the left of the panel card
                handle.topAnchor.constraint(equalTo: parent.topAnchor, constant: p),
                handle.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                handle.trailingAnchor.constraint(equalTo: wrapper.leadingAnchor),
                handle.widthAnchor.constraint(equalToConstant: h),
            ])

        case .left:
            let wc = wrapper.widthAnchor.constraint(equalToConstant: size)
            slot.sizeConstraint = wc
            NSLayoutConstraint.activate([
                wrapper.topAnchor.constraint(equalTo: parent.topAnchor, constant: p),
                wrapper.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -p),
                wrapper.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: p),
                wc,
                // Handle sits in the gap to the right of the panel card
                handle.topAnchor.constraint(equalTo: parent.topAnchor, constant: p),
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

    private func cardInset(for position: PanelPosition) -> CGFloat {
        let p = padding
        guard let id = activeByPosition[position], let slot = panels[id], slot.isVisible else { return p }
        if position == .bottom, slot.isFullscreen {
            return p
        }
        return p + clampedPanelSize(slot.currentSize, for: position) + p
    }

    private func cardTrailingInset() -> CGFloat { cardInset(for: .right) }
    private func cardBottomInset() -> CGFloat { cardInset(for: .bottom) }
    private func cardTopInset() -> CGFloat { cardInset(for: .top) }

    /// Leading inset is 0 when no left panel is open (the content card sits flush
    /// against the sidebar), and `padding + panel + padding` when one is, so the
    /// card shrinks to exactly clear the left panel.
    private func cardLeadingInset() -> CGFloat {
        guard let id = activeByPosition[.left], let slot = panels[id], slot.isVisible else {
            return 0
        }
        return padding + clampedPanelSize(slot.currentSize, for: .left) + padding
    }

    private func bringPanelToFront(_ slot: PanelSlot) {
        guard let parent = overlayParent else { return }
        parent.addSubview(slot.shadowWrapper, positioned: .above, relativeTo: nil)
        parent.addSubview(slot.resizeHandle, positioned: .above, relativeTo: slot.shadowWrapper)
    }

    private func exitOffset(for slot: PanelSlot) -> (x: CGFloat, y: CGFloat) {
        let size = clampedPanelSize(slot.currentSize, for: slot.config.position) + padding
        switch slot.config.position {
        case .right:  return (x: size, y: 0)
        case .left:   return (x: -size, y: 0)
        case .bottom:
            if slot.isFullscreen, let parent = overlayParent {
                return (x: 0, y: -(parent.bounds.height + padding))
            }
            return (x: 0, y: -size)
        case .top:
            return (x: 0, y: size)
        }
    }

    private func forceHide(id: String) {
        hidePanelInternal(id: id, duration: Self.animationDuration * 0.5, updateCardEdges: false)
    }

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
            let width = max(0, parent.bounds.width - (p * 2))
            let height = max(0, parent.bounds.height - (p * 2))
            slot.shadowWrapper.frame = NSRect(
                x: p,
                y: p,
                width: width,
                height: height
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
    private func clampedPanelSize(_ requested: CGFloat, for position: PanelPosition) -> CGFloat {
        let base = min(max(requested, Self.panelMinSize), Self.panelMaxSize)
        guard let parent = overlayParent else { return base }

        let p = padding
        let maxByWindow: CGFloat
        switch position {
        case .right, .left:
            maxByWindow = parent.bounds.width - (sidebarRef?.frame.width ?? 0) - p * 3 - Self.minMainRegionWidth
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
