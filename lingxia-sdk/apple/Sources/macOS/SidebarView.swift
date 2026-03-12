#if os(macOS)
import AppKit
import CLingXiaRustAPI
import os.log

// MARK: - Resize Handle

/// Draggable handle on the right edge of the sidebar for resizing.
@MainActor
private class SidebarResizeHandle: NSView {
    var onDrag: ((CGFloat) -> Void)?
    var onDragEnd: ((CGFloat) -> Void)?
    private var initialMouseX: CGFloat = 0
    private var initialWidth: CGFloat = 0

    // Prevent window drag — this view handles its own mouse events
    override var mouseDownCanMoveWindow: Bool { false }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .resizeLeftRight)
    }

    // Always return self so the separator subview doesn't steal events
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, frame.contains(point) else { return nil }
        return self
    }

    override func mouseDown(with event: NSEvent) {
        initialMouseX = event.locationInWindow.x
        initialWidth = superview?.frame.width ?? 0
    }

    override func mouseDragged(with event: NSEvent) {
        let deltaX = event.locationInWindow.x - initialMouseX
        let newWidth = initialWidth + deltaX
        onDrag?(newWidth)
    }

    override func mouseUp(with event: NSEvent) {
        let deltaX = event.locationInWindow.x - initialMouseX
        let newWidth = initialWidth + deltaX
        onDragEnd?(newWidth)
    }
}

// MARK: - SidebarView

/// The main sidebar container view, modeled after Chrome vertical tab groups.
/// Supports drag-to-resize and a minimized mode with colored dots.
/// In minimized mode, clicking a dot selects the tab; clicking elsewhere expands.
@MainActor
public class SidebarView: NSView {

    struct Layout {
        static let expandedWidth: CGFloat = 180
        static let minCollapsedWidth: CGFloat = 48
        static let maxWidth: CGFloat = 400
        static let collapseThreshold: CGFloat = 80
        static let fullyHiddenThreshold: CGFloat = 1
        static let trafficLightsHeight: CGFloat = 38
        static let toggleButtonSize: CGFloat = 28
        static let resizeHandleWidth: CGFloat = 5
        static let dotDiameter: CGFloat = 12
        static let dotSpacing: CGFloat = 16
        static let dotTopOffset: CGFloat = 50
    }

    private let headerView = NSView()
    private let toggleButton = NSButton()
    private let settingsButton = NSButton()
    private let downloadButton = NSButton()
    private let scrollView = NSScrollView()
    private let resizeHandle = SidebarResizeHandle()
    private let minimizedDotsContainer = NSView()

    private var groupViews: [String: SidebarGroupView] = [:]
    private var dotViews: [(appId: String, dot: NSView)] = []
    private var currentTabs: [LxAppTab] = []

    // Browser tab views
    private var browserItemViews: [UUID: SidebarBrowserItemView] = [:]
    private var browserItemTopConstraints: [UUID: NSLayoutConstraint] = [:]
    private var browserTabOrder: [UUID] = []
    private let addButton = NSButton()
    private var addButtonTopConstraint: NSLayoutConstraint?
    private var groupTopConstraints: [String: NSLayoutConstraint] = [:]
    private var addButtonTrackingArea: NSTrackingArea?

    /// True when the sidebar is at minimized width (showing dots only)
    var isMinimized: Bool {
        return frame.width <= Layout.minCollapsedWidth + 1
    }

    var isFullyHidden: Bool {
        return frame.width < Layout.fullyHiddenThreshold
    }

    /// Called when user selects a page: (appId, itemIndex)
    var onAppPageSelected: ((String, Int) -> Void)?
    /// Called when user requests to close an app: (appId)
    var onAppCloseRequested: ((String) -> Void)?
    /// Called when toggle button is clicked or minimized area is clicked
    var onToggleRequested: (() -> Void)?
    /// Called when width changes via drag: (width, animated)
    var onWidthChanged: ((CGFloat, Bool) -> Void)?
    /// Called when "+" button is clicked to add a browser tab
    var onAddBrowserTab: (() -> Void)?
    /// Called when a browser tab is selected
    var onBrowserTabSelected: ((UUID) -> Void)?
    /// Called when a browser tab close is requested
    var onBrowserTabCloseRequested: ((UUID) -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    // Prevent window drag when SidebarView itself receives events (minimized mode)
    override public var mouseDownCanMoveWindow: Bool { false }

    // MARK: - Hit Testing (minimized mode)

    override public func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, frame.contains(point) else { return nil }

        if isMinimized {
            // In minimized mode, only dots and resize handle receive direct events.
            // Everything else returns self → mouseDown triggers expand.
            if let hit = super.hitTest(point) {
                if hit === resizeHandle || dotViews.contains(where: { $0.dot === hit }) {
                    return hit
                }
            }
            return self
        }

        return super.hitTest(point)
    }

    override public func mouseDown(with event: NSEvent) {
        if isMinimized {
            onToggleRequested?()
            return
        }
        super.mouseDown(with: event)
    }

    // MARK: - Setup

    private func setupViews() {
        wantsLayer = true
        clipsToBounds = true

        // Header (traffic lights + toggle)
        headerView.translatesAutoresizingMaskIntoConstraints = false
        headerView.wantsLayer = true
        addSubview(headerView)

        // Settings and download buttons — top-right in header
        settingsButton.translatesAutoresizingMaskIntoConstraints = false
        settingsButton.image = NSImage(systemSymbolName: "gearshape", accessibilityDescription: nil)
        settingsButton.isBordered = false
        settingsButton.bezelStyle = .regularSquare
        settingsButton.imagePosition = .imageOnly
        settingsButton.contentTintColor = NSColor.secondaryLabelColor
        settingsButton.target = self
        settingsButton.action = #selector(settingsClicked)
        headerView.addSubview(settingsButton)

        downloadButton.translatesAutoresizingMaskIntoConstraints = false
        downloadButton.image = NSImage(systemSymbolName: "arrow.down.circle", accessibilityDescription: nil)
        downloadButton.isBordered = false
        downloadButton.bezelStyle = .regularSquare
        downloadButton.imagePosition = .imageOnly
        downloadButton.contentTintColor = NSColor.secondaryLabelColor
        downloadButton.target = self
        downloadButton.action = #selector(downloadClicked)
        headerView.addSubview(downloadButton)

        // Toggle button — right-aligned in header
        toggleButton.translatesAutoresizingMaskIntoConstraints = false
        toggleButton.image = NSImage(systemSymbolName: "sidebar.left", accessibilityDescription: "Toggle sidebar")
        toggleButton.isBordered = false
        toggleButton.bezelStyle = .regularSquare
        toggleButton.imagePosition = .imageOnly
        toggleButton.contentTintColor = NSColor.secondaryLabelColor
        toggleButton.target = self
        toggleButton.action = #selector(toggleClicked)
        headerView.addSubview(toggleButton)

        // Scroll view (trailing inset to leave room for resize handle)
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        addSubview(scrollView)

        // Document view (flipped)
        let flipView = FlippedView()
        flipView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.documentView = flipView

        // Minimized dots container (hidden by default)
        minimizedDotsContainer.translatesAutoresizingMaskIntoConstraints = false
        minimizedDotsContainer.isHidden = true
        addSubview(minimizedDotsContainer)

        // Resize handle on right edge
        resizeHandle.translatesAutoresizingMaskIntoConstraints = false
        resizeHandle.wantsLayer = true
        addSubview(resizeHandle)

        NSLayoutConstraint.activate([
            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerView.heightAnchor.constraint(equalToConstant: Layout.trafficLightsHeight),

            // Settings and download buttons: right-aligned in header, next to toggle button
            downloadButton.trailingAnchor.constraint(equalTo: toggleButton.leadingAnchor, constant: -4),
            downloadButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            downloadButton.widthAnchor.constraint(equalToConstant: Layout.toggleButtonSize),
            downloadButton.heightAnchor.constraint(equalToConstant: Layout.toggleButtonSize),

            settingsButton.trailingAnchor.constraint(equalTo: downloadButton.leadingAnchor, constant: -4),
            settingsButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            settingsButton.widthAnchor.constraint(equalToConstant: Layout.toggleButtonSize),
            settingsButton.heightAnchor.constraint(equalToConstant: Layout.toggleButtonSize),

            // Toggle button: right-aligned in header
            toggleButton.trailingAnchor.constraint(equalTo: headerView.trailingAnchor, constant: -12),
            toggleButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            toggleButton.widthAnchor.constraint(equalToConstant: Layout.toggleButtonSize),
            toggleButton.heightAnchor.constraint(equalToConstant: Layout.toggleButtonSize),

            // Scroll view: inset trailing by resize handle width, extends to bottom
            scrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),

            minimizedDotsContainer.topAnchor.constraint(equalTo: topAnchor, constant: Layout.dotTopOffset),
            minimizedDotsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            minimizedDotsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            minimizedDotsContainer.bottomAnchor.constraint(equalTo: bottomAnchor),

            // Resize handle: right edge, full height
            resizeHandle.topAnchor.constraint(equalTo: topAnchor),
            resizeHandle.trailingAnchor.constraint(equalTo: trailingAnchor),
            resizeHandle.bottomAnchor.constraint(equalTo: bottomAnchor),
            resizeHandle.widthAnchor.constraint(equalToConstant: Layout.resizeHandleWidth),
        ])

        // Document view fills scroll view width
        if let docView = scrollView.documentView {
            NSLayoutConstraint.activate([
                docView.leadingAnchor.constraint(equalTo: scrollView.contentView.leadingAnchor),
                docView.trailingAnchor.constraint(equalTo: scrollView.contentView.trailingAnchor),
                docView.topAnchor.constraint(equalTo: scrollView.contentView.topAnchor),
            ])
        }

        // Separator line inside the resize handle (hidden for seamless blending with Layer 2)
        let separatorLine = NSView()
        separatorLine.translatesAutoresizingMaskIntoConstraints = false
        separatorLine.wantsLayer = true
        separatorLine.layer?.backgroundColor = NSColor.clear.cgColor  // Hidden for seamless appearance
        resizeHandle.addSubview(separatorLine)

        NSLayoutConstraint.activate([
            separatorLine.topAnchor.constraint(equalTo: resizeHandle.topAnchor),
            separatorLine.trailingAnchor.constraint(equalTo: resizeHandle.trailingAnchor),
            separatorLine.bottomAnchor.constraint(equalTo: resizeHandle.bottomAnchor),
            separatorLine.widthAnchor.constraint(equalToConstant: 1),
        ])

        resizeHandle.onDrag = { [weak self] newWidth in
            self?.handleDrag(proposedWidth: newWidth)
        }
        resizeHandle.onDragEnd = { [weak self] newWidth in
            self?.handleDragEnd(proposedWidth: newWidth)
        }
    }

    // MARK: - Drag Handling

    private func handleDrag(proposedWidth: CGFloat) {
        let clamped = min(max(proposedWidth, 0), Layout.maxWidth)
        onWidthChanged?(clamped, false)
    }

    private func handleDragEnd(proposedWidth: CGFloat) {
        if proposedWidth < Layout.collapseThreshold {
            onWidthChanged?(0, true)
        } else {
            let clamped = min(max(proposedWidth, Layout.collapseThreshold), Layout.maxWidth)
            onWidthChanged?(clamped, true)
        }
    }

    // MARK: - Minimized Mode

    /// Update display mode based on current width
    func updateMinimizedState() {
        let hidden = isFullyHidden
        let minimized = isMinimized && !hidden

        scrollView.isHidden = hidden || minimized
        toggleButton.isHidden = hidden || minimized
        settingsButton.isHidden = hidden || minimized
        downloadButton.isHidden = hidden || minimized
        minimizedDotsContainer.isHidden = !minimized
        resizeHandle.isHidden = hidden
    }

    /// Rebuild colored dots for current groups
    private func rebuildDots(tabs: [LxAppTab]) {
        for item in dotViews {
            item.dot.removeFromSuperview()
        }
        dotViews.removeAll()

        var yOffset: CGFloat = 0
        for (tabIndex, tab) in tabs.enumerated() {
            let palette = SidebarGroupColor.palette(for: tabIndex)
            let dot = NSView()
            dot.translatesAutoresizingMaskIntoConstraints = false
            dot.wantsLayer = true
            dot.layer?.backgroundColor = palette.headerBg.cgColor
            dot.layer?.cornerRadius = Layout.dotDiameter / 2
            minimizedDotsContainer.addSubview(dot)

            NSLayoutConstraint.activate([
                dot.centerXAnchor.constraint(equalTo: minimizedDotsContainer.centerXAnchor),
                dot.topAnchor.constraint(equalTo: minimizedDotsContainer.topAnchor, constant: yOffset),
                dot.widthAnchor.constraint(equalToConstant: Layout.dotDiameter),
                dot.heightAnchor.constraint(equalToConstant: Layout.dotDiameter),
            ])

            let clickGesture = NSClickGestureRecognizer(target: self, action: #selector(dotClicked(_:)))
            dot.addGestureRecognizer(clickGesture)

            dotViews.append((appId: tab.appId, dot: dot))
            yOffset += Layout.dotDiameter + Layout.dotSpacing
        }
    }

    @objc private func dotClicked(_ gesture: NSClickGestureRecognizer) {
        guard let clickedDot = gesture.view else { return }
        if let match = dotViews.first(where: { $0.dot === clickedDot }) {
            onAppPageSelected?(match.appId, 0)
        }
    }

    // MARK: - Public API

    /// Rebuild all groups based on current tabs
    func updateForTabs(_ tabs: [LxAppTab], activeTab: LxAppTab?) {
        guard let docView = scrollView.documentView else { return }

        currentTabs = tabs

        // Remove groups for apps no longer in tabs
        let currentAppIds = Set(tabs.map { $0.appId })
        for (appId, groupView) in groupViews {
            if !currentAppIds.contains(appId) {
                groupView.removeFromSuperview()
                groupViews.removeValue(forKey: appId)
                groupTopConstraints.removeValue(forKey: appId)
            }
        }

        // Add/update groups
        var yOffset: CGFloat = 4
        for (tabIndex, tab) in tabs.enumerated() {
            let groupView: SidebarGroupView
            if let existing = groupViews[tab.appId] {
                groupView = existing
            } else {
                groupView = SidebarGroupView(appId: tab.appId)
                groupView.onPageSelected = { [weak self] appId, itemIndex in
                    self?.onAppPageSelected?(appId, itemIndex)
                }
                groupView.onCloseRequested = { [weak self] appId in
                    self?.onAppCloseRequested?(appId)
                }
                groupView.onLayoutChanged = { [weak self] in
                    self?.relayoutAfterGroupToggle()
                }
                groupViews[tab.appId] = groupView
            }

            groupView.setColorIndex(tabIndex)

            if groupView.superview !== docView {
                groupView.removeFromSuperview()
                docView.addSubview(groupView)
                NSLayoutConstraint.activate([
                    groupView.leadingAnchor.constraint(equalTo: docView.leadingAnchor),
                    groupView.trailingAnchor.constraint(equalTo: docView.trailingAnchor),
                ])
            }

            if let tc = groupTopConstraints[tab.appId] {
                tc.constant = yOffset
            } else {
                let tc = groupView.topAnchor.constraint(equalTo: docView.topAnchor, constant: yOffset)
                tc.isActive = true
                groupTopConstraints[tab.appId] = tc
            }

            groupView.layoutSubtreeIfNeeded()
            yOffset += groupView.fittingSize.height + 8
        }

        // Layout browser separator, browser items, and "+" button after groups
        yOffset = layoutBrowserSection(in: docView, yOffset: yOffset)

        // Update document view height
        let docHeight = max(yOffset, scrollView.contentView.bounds.height)
        if let docView = scrollView.documentView {
            docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: docHeight)
        }

        if let activeAppId = activeTab?.appId {
            setActiveHighlight(appId: activeAppId)
        }

        rebuildDots(tabs: tabs)
    }

    /// Refresh a specific app group from Rust data
    func refreshAppGroup(appId: String) {
        groupViews[appId]?.refreshFromRust()
    }

    /// Set active highlight on the appropriate group and item
    func setActiveHighlight(appId: String, pageIndex: Int? = nil) {
        // Clear browser selections when an lxapp is selected
        for (_, itemView) in browserItemViews {
            itemView.isSelected = false
        }

        for (id, group) in groupViews {
            if id == appId {
                if let idx = pageIndex {
                    group.setActiveHighlight(pageIndex: idx)
                } else {
                    if let tabBar = getTabBar(appId) {
                        group.setActiveHighlight(pageIndex: Int(tabBar.selected_index))
                    }
                }
            } else {
                group.clearHighlight()
            }
        }
    }

    /// Clear all highlights (both lxapp and browser)
    func clearAllHighlights() {
        for (_, group) in groupViews {
            group.clearHighlight()
        }
        for (_, itemView) in browserItemViews {
            itemView.isSelected = false
        }
    }

    // MARK: - Browser Items

    /// Update browser tab items in the sidebar
    func updateBrowserItems(_ items: [(id: UUID, title: String, favicon: NSImage?)], activeId: UUID?) {
        guard let docView = scrollView.documentView else { return }

        // Store ordering
        browserTabOrder = items.map { $0.id }

        // Remove browser items no longer present
        let currentIds = Set(items.map { $0.id })
        for (id, itemView) in browserItemViews {
            if !currentIds.contains(id) {
                if let topConstraint = browserItemTopConstraints[id] {
                    topConstraint.isActive = false
                    browserItemTopConstraints.removeValue(forKey: id)
                }
                itemView.removeFromSuperview()
                browserItemViews.removeValue(forKey: id)
            }
        }

        // Add/update browser items
        for item in items {
            if let existing = browserItemViews[item.id] {
                existing.configure(title: item.title, isSelected: item.id == activeId, favicon: item.favicon)
            } else {
                let itemView = SidebarBrowserItemView(id: item.id)
                itemView.translatesAutoresizingMaskIntoConstraints = false
                itemView.onClick = { [weak self] id in
                    self?.onBrowserTabSelected?(id)
                }
                itemView.onClose = { [weak self] id in
                    self?.onBrowserTabCloseRequested?(id)
                }
                itemView.configure(title: item.title, isSelected: item.id == activeId, favicon: item.favicon)
                browserItemViews[item.id] = itemView
            }
        }

        // Re-layout browser section
        relayoutBrowserSection(in: docView)
    }

    /// Layout browser items and add button after lxapp groups
    private func layoutBrowserSection(in docView: NSView, yOffset startY: CGFloat) -> CGFloat {
        let groupInset: CGFloat = SidebarGroupView.Layout.groupInset
        var yOffset = startY

        // Browser item views (ordered by browserTabOrder)
        for tabId in browserTabOrder {
            guard let itemView = browserItemViews[tabId] else { continue }
            ensureSubview(itemView, in: docView) {
                NSLayoutConstraint.activate([
                    itemView.leadingAnchor.constraint(equalTo: docView.leadingAnchor, constant: groupInset),
                    itemView.trailingAnchor.constraint(equalTo: docView.trailingAnchor, constant: -groupInset),
                ])
            }

            if let tc = browserItemTopConstraints[tabId] {
                tc.constant = yOffset
            } else {
                let tc = itemView.topAnchor.constraint(equalTo: docView.topAnchor, constant: yOffset)
                tc.isActive = true
                browserItemTopConstraints[tabId] = tc
            }
            yOffset += SidebarBrowserItemView.Layout.height + 2
        }

        // "+" button
        ensureSubview(addButton, in: docView) {
            setupAddButton()
            NSLayoutConstraint.activate([
                addButton.leadingAnchor.constraint(equalTo: docView.leadingAnchor, constant: groupInset),
                addButton.trailingAnchor.constraint(equalTo: docView.trailingAnchor, constant: -groupInset),
                addButton.heightAnchor.constraint(equalToConstant: 28),
            ])
        }
        updateOrCreate(&addButtonTopConstraint, on: addButton, in: docView, constant: yOffset)
        yOffset += 28 + 8

        return yOffset
    }

    /// Ensure a view is a subview of parent; run setup closure only on first add
    private func ensureSubview(_ view: NSView, in parent: NSView, setup: () -> Void) {
        if view.superview !== parent {
            view.removeFromSuperview()
            parent.addSubview(view)
            setup()
        }
    }

    /// Update an existing top constraint's constant, or create one
    private func updateOrCreate(_ constraint: inout NSLayoutConstraint?, on view: NSView, in parent: NSView, constant: CGFloat) {
        if let c = constraint {
            c.constant = constant
        } else {
            let c = view.topAnchor.constraint(equalTo: parent.topAnchor, constant: constant)
            c.isActive = true
            constraint = c
        }
    }

    /// Calculate Y offset after all groups (using tab order)
    private func yOffsetAfterGroups() -> CGFloat {
        var yOffset: CGFloat = 4
        for tab in currentTabs {
            if let groupView = groupViews[tab.appId] {
                groupView.layoutSubtreeIfNeeded()
                yOffset += groupView.fittingSize.height + 8
            }
        }
        return yOffset
    }

    /// Re-layout after a group expands/collapses — repositions groups + browser section
    private func relayoutAfterGroupToggle() {
        guard let docView = scrollView.documentView else { return }

        // Reposition all groups using stored top constraints
        var yOffset: CGFloat = 4
        for tab in currentTabs {
            guard let groupView = groupViews[tab.appId] else { continue }
            groupTopConstraints[tab.appId]?.constant = yOffset
            groupView.layoutSubtreeIfNeeded()
            yOffset += groupView.fittingSize.height + 8
        }

        // Re-layout browser section below groups
        yOffset = layoutBrowserSection(in: docView, yOffset: yOffset)

        let docHeight = max(yOffset, scrollView.contentView.bounds.height)
        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: docHeight)
    }

    /// Re-layout just the browser section (for title updates without full tab rebuild)
    private func relayoutBrowserSection(in docView: NSView) {
        let yOffset = layoutBrowserSection(in: docView, yOffset: yOffsetAfterGroups())

        let docHeight = max(yOffset, scrollView.contentView.bounds.height)
        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: docHeight)
    }

    private func setupAddButton() {
        addButton.translatesAutoresizingMaskIntoConstraints = false
        addButton.title = ""
        addButton.image = NSImage(systemSymbolName: "plus", accessibilityDescription: "Add browser tab")
        addButton.isBordered = false
        addButton.bezelStyle = .regularSquare
        addButton.imagePosition = .imageOnly
        addButton.contentTintColor = NSColor.secondaryLabelColor
        addButton.wantsLayer = true
        addButton.layer?.cornerRadius = 6
        addButton.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        addButton.target = self
        addButton.action = #selector(addButtonClicked)
    }

    @objc private func addButtonClicked() {
        onAddBrowserTab?()
    }

    @objc private func toggleClicked() {
        onToggleRequested?()
    }

    @objc private func settingsClicked() {
        os_log("Settings button clicked", log: .default)
    }

    @objc private func downloadClicked() {
        os_log("Download button clicked", log: .default)
    }

    // MARK: - Add button hover

    override public func updateTrackingAreas() {
        super.updateTrackingAreas()

        // Add button hover tracking
        if let existing = addButtonTrackingArea {
            addButton.removeTrackingArea(existing)
        }
        let area = NSTrackingArea(
            rect: addButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addButton.addTrackingArea(area)
        addButtonTrackingArea = area
    }

    override public func mouseEntered(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(true)
        }
    }

    override public func mouseExited(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(false)
        }
    }

    private func setAddButtonHovered(_ hovered: Bool) {
        let alpha: CGFloat = hovered ? 0.12 : 0.06
        addButton.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(alpha).cgColor
    }
}

/// NSView subclass with flipped coordinate system (top-left origin)
@MainActor
private class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

#endif
