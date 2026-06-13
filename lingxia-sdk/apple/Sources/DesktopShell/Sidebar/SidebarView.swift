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

@MainActor
private final class SidebarClipView: NSClipView {
    override var mouseDownCanMoveWindow: Bool { false }
}

@MainActor
private final class SidebarScrollView: NSScrollView {
    override var mouseDownCanMoveWindow: Bool { false }
}

// MARK: - PanelIconItem

/// Minimal display info for a panel icon in the sidebar footer.
/// SidebarView only needs these — routing details (appId, path) are in Panel.swift.
struct PanelIconItem {
    let id: String
    let iconURL: URL?
    let label: String
}

// MARK: - SidebarView

/// The main sidebar container view, modeled after Chrome vertical tab groups.
/// Supports drag-to-resize and a fully hidden state.
@MainActor
class SidebarView: NSView {
    private static let log = OSLog(subsystem: "LingXia", category: "Sidebar")

    struct Layout {
        static let expandedWidth: CGFloat = 180
        static let maxWidth: CGFloat = 400
        static let collapseThreshold: CGFloat = 80
        static let fullyHiddenThreshold: CGFloat = 1
        // Reserve only the shared traffic-light / toolbar row; the titlebar offset is
        // already handled by `buttonCenterYFromTop`.
        static let trafficLightsHeight: CGFloat = 38
        static let actionButtonSize: CGFloat = 28
        static let resizeHandleWidth: CGFloat = 5
        /// Bottom dock height — tall enough for one row of icon buttons plus breathing room.
        static let footerHeight: CGFloat = 48
        /// Square icon button size in the dock.
        static let footerButtonSize: CGFloat = 28
        /// Rendered glyph size inside footer icon buttons.
        static let footerIconSize: CGFloat = 16
        /// Horizontal/vertical padding inside the dock.
        static let footerInset: CGFloat = 6
    }

    private let headerView = NSView()
    private let settingsButton = NSButton()
    private let downloadButton = NSButton()
    private let scrollView = SidebarScrollView()
    private let resizeHandle = SidebarResizeHandle()
    private let footerView = NSView()
    private let footerSeparator = NSView()
    /// Horizontal stack that holds trailing product/action buttons.
    private let panelStack = NSStackView()
    private let hideButton = NSButton()
    private var hideButtonTrackingArea: NSTrackingArea?
    private var panelButtons: [NSButton] = []
    private var appUIOnlyMode = false

    /// Called when a panel icon button is clicked: (panelId)
    var onPanelItemToggled: ((String) -> Void)?

    /// Called when the update callout is clicked, with its current state
    /// (`.ready` → restart, `.available` → install).
    var onUpdateActionRequested: ((UpdateCalloutState) -> Void)?

    /// The transient "ready to update" callout shown above the footer dock.
    private var updateReadyCallout: UpdateReadyCallout?

    private var groupViews: [String: SidebarGroupView] = [:]
    private var currentTabs: [LxAppTab] = []

    // Browser tab views
    private var browserItemViews: [String: SidebarBrowserItemView] = [:]
    private var browserItemTopConstraints: [String: NSLayoutConstraint] = [:]
    private var browserTabOrder: [String] = []
    private let addButton = NSButton()
    private var addButtonTopConstraint: NSLayoutConstraint?
    private var groupTopConstraints: [String: NSLayoutConstraint] = [:]
    private var addButtonTrackingArea: NSTrackingArea?

    /// Target center Y for the header buttons, measured from the header's top edge.
    var buttonCenterYFromTop: CGFloat = Layout.trafficLightsHeight / 2 {
        didSet {
            guard oldValue != buttonCenterYFromTop else { return }
            buttonCenterYConstraints.forEach { $0.constant = buttonCenterYFromTop }
        }
    }
    private var buttonCenterYConstraints: [NSLayoutConstraint] = []

    var isFullyHidden: Bool {
        return frame.width < Layout.fullyHiddenThreshold
    }

    /// Called when user selects a page: (appId, itemIndex)
    var onAppPageSelected: ((String, Int) -> Void)?
    /// Called when user requests to close an app: (appId)
    var onAppCloseRequested: ((String) -> Void)?
    /// Called when the bottom hide button is clicked
    var onHideRequested: (() -> Void)?
    /// Called when width changes via drag: (width, animated)
    var onWidthChanged: ((CGFloat, Bool) -> Void)?
    /// Called when "+" button is clicked to add a browser tab
    var onAddBrowserTab: (() -> Void)?
    /// Called when a browser tab is selected
    var onBrowserTabSelected: ((String) -> Void)?
    /// Called when a browser tab close is requested
    var onBrowserTabCloseRequested: ((String) -> Void)?
    /// Called when settings button is clicked
    var onOpenSettings: (() -> Void)?
    /// Called when downloads button is clicked
    var onOpenDownloads: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    // Prevent window drag when SidebarView itself receives events
    override public var mouseDownCanMoveWindow: Bool { false }

    // MARK: - Setup

    private func setupViews() {
        wantsLayer = true
        clipsToBounds = true

        // Header (traffic lights + actions)
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

        let shellEnabled = (LxAppCore.capabilities & LxAppCore.capShell) != 0
        os_log(
            "Sidebar setup shellEnabled=%{public}@ capabilities=%{public}u",
            log: Self.log,
            type: .info,
            shellEnabled ? "true" : "false",
            LxAppCore.capabilities
        )
        settingsButton.isHidden = !shellEnabled
        downloadButton.isHidden = !shellEnabled

        // Scroll view (trailing inset to leave room for resize handle)
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.contentView = SidebarClipView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
        scrollView.verticalScrollElasticity = .none
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        addSubview(scrollView)

        // Document view (flipped)
        let flipView = FlippedView()
        flipView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.documentView = flipView

        // Footer dock — bottom toolbar row for icon buttons
        footerView.translatesAutoresizingMaskIntoConstraints = false
        footerView.wantsLayer = true
        addSubview(footerView)

        // Hairline separator between scroll content and footer
        footerSeparator.translatesAutoresizingMaskIntoConstraints = false
        footerSeparator.wantsLayer = true
        footerSeparator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        footerView.addSubview(footerSeparator)

        panelStack.translatesAutoresizingMaskIntoConstraints = false
        panelStack.orientation = .horizontal
        panelStack.spacing = 4
        panelStack.alignment = .centerY
        panelStack.distribution = .fill
        footerView.addSubview(panelStack)

        hideButton.translatesAutoresizingMaskIntoConstraints = false
        hideButton.title = ""
        hideButton.image = NSImage(systemSymbolName: "chevron.left", accessibilityDescription: "Hide sidebar")
        hideButton.imagePosition = .imageOnly
        hideButton.isBordered = false
        hideButton.bezelStyle = .regularSquare
        hideButton.contentTintColor = NSColor.secondaryLabelColor
        hideButton.wantsLayer = true
        hideButton.layer?.cornerRadius = 6
        hideButton.layer?.backgroundColor = NSColor.clear.cgColor
        hideButton.toolTip = "Hide sidebar"
        hideButton.target = self
        hideButton.action = #selector(hideButtonClicked)
        footerView.addSubview(hideButton)

        // Resize handle on right edge
        resizeHandle.translatesAutoresizingMaskIntoConstraints = false
        resizeHandle.wantsLayer = true
        addSubview(resizeHandle)

        NSLayoutConstraint.activate([
            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerView.heightAnchor.constraint(equalToConstant: Layout.trafficLightsHeight),

            downloadButton.trailingAnchor.constraint(equalTo: headerView.trailingAnchor, constant: -12),
            downloadButton.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            downloadButton.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),

            settingsButton.trailingAnchor.constraint(equalTo: downloadButton.leadingAnchor, constant: -4),
            settingsButton.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            settingsButton.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),

            // Scroll view: inset trailing by resize handle width, extends above footer
            scrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            scrollView.bottomAnchor.constraint(equalTo: footerView.topAnchor),

            footerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            footerView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            footerView.bottomAnchor.constraint(equalTo: bottomAnchor),
            footerView.heightAnchor.constraint(equalToConstant: Layout.footerHeight),

            footerSeparator.topAnchor.constraint(equalTo: footerView.topAnchor),
            footerSeparator.leadingAnchor.constraint(equalTo: footerView.leadingAnchor),
            footerSeparator.trailingAnchor.constraint(equalTo: footerView.trailingAnchor),
            footerSeparator.heightAnchor.constraint(equalToConstant: 0.5),

            hideButton.leadingAnchor.constraint(equalTo: footerView.leadingAnchor, constant: Layout.footerInset),
            hideButton.centerYAnchor.constraint(equalTo: footerView.centerYAnchor),

            hideButton.widthAnchor.constraint(equalToConstant: Layout.footerButtonSize),
            hideButton.heightAnchor.constraint(equalToConstant: Layout.footerButtonSize),

            panelStack.trailingAnchor.constraint(equalTo: footerView.trailingAnchor, constant: -Layout.footerInset),
            panelStack.centerYAnchor.constraint(equalTo: footerView.centerYAnchor),
            panelStack.leadingAnchor.constraint(greaterThanOrEqualTo: hideButton.trailingAnchor, constant: 4),

            // Resize handle: right edge, full height
            resizeHandle.topAnchor.constraint(equalTo: topAnchor),
            resizeHandle.trailingAnchor.constraint(equalTo: trailingAnchor),
            resizeHandle.bottomAnchor.constraint(equalTo: bottomAnchor),
            resizeHandle.widthAnchor.constraint(equalToConstant: Layout.resizeHandleWidth),
        ])

        // Button center constraints — stored so we can align them to the effective traffic-light center.
        let centerY = buttonCenterYFromTop
        let downloadCenter = downloadButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        let settingsCenter = settingsButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        buttonCenterYConstraints = [downloadCenter, settingsCenter]
        NSLayoutConstraint.activate(buttonCenterYConstraints)

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

    func updateVisibilityState() {
        let hidden = isFullyHidden
        let shellEnabled = (LxAppCore.capabilities & LxAppCore.capShell) != 0
        os_log(
            "Sidebar visibility hidden=%{public}@ shellEnabled=%{public}@ capabilities=%{public}u",
            log: Self.log,
            type: .debug,
            hidden ? "true" : "false",
            shellEnabled ? "true" : "false",
            LxAppCore.capabilities
        )
        scrollView.isHidden = hidden || appUIOnlyMode
        settingsButton.isHidden = hidden || !shellEnabled || appUIOnlyMode
        downloadButton.isHidden = hidden || !shellEnabled || appUIOnlyMode
        footerView.isHidden = hidden
        resizeHandle.isHidden = hidden
    }

    func setAppUIOnlyMode(_ enabled: Bool) {
        appUIOnlyMode = enabled

        guard enabled else {
            updateVisibilityState()
            return
        }

        currentTabs.removeAll()
        groupViews.values.forEach { $0.removeFromSuperview() }
        groupViews.removeAll()
        groupTopConstraints.removeAll()

        browserItemViews.values.forEach { $0.removeFromSuperview() }
        browserItemViews.removeAll()
        browserItemTopConstraints.removeAll()
        browserTabOrder.removeAll()
        addButton.removeFromSuperview()
        addButtonTopConstraint = nil

        if let docView = scrollView.documentView {
            docView.subviews.forEach { $0.removeFromSuperview() }
            docView.frame = .zero
        }

        updateVisibilityState()
    }

    /// Populate panel icon buttons in the footer.
    /// `PanelIconItem` only carries what the UI needs — id, icon, label.
    /// Routing details (appId, path, position) stay in Panel.swift.
    func updatePanelItems(_ items: [PanelIconItem]) {
        // Remove existing panel buttons.
        panelButtons.forEach {
            panelStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        panelButtons.removeAll()

        guard !items.isEmpty else { return }

        for item in items {
            let btn = NSButton()
            btn.translatesAutoresizingMaskIntoConstraints = false
            btn.isBordered = false
            btn.bezelStyle = .regularSquare
            btn.imagePosition = .imageOnly
            btn.imageScaling = .scaleProportionallyDown
            btn.wantsLayer = true
            btn.layer?.cornerRadius = 6
            btn.layer?.backgroundColor = NSColor.clear.cgColor
            btn.toolTip = item.label
            if let iconURL = item.iconURL,
               let image = NSImage(contentsOf: iconURL) {
                image.size = NSSize(width: Layout.footerIconSize, height: Layout.footerIconSize)
                image.isTemplate = true
                btn.image = image
                btn.contentTintColor = NSColor.secondaryLabelColor
            } else {
                let fallback = NSImage(systemSymbolName: "square.grid.2x2", accessibilityDescription: item.label)
                fallback?.size = NSSize(width: Layout.footerIconSize, height: Layout.footerIconSize)
                btn.image = fallback
                btn.contentTintColor = NSColor.secondaryLabelColor
            }
            btn.target = self
            btn.action = #selector(panelButtonClicked(_:))
            // Store panel id in the button's identifier
            btn.identifier = NSUserInterfaceItemIdentifier(item.id)
            NSLayoutConstraint.activate([
                btn.widthAnchor.constraint(equalToConstant: Layout.footerButtonSize),
                btn.heightAnchor.constraint(equalToConstant: Layout.footerButtonSize),
            ])
            panelStack.addArrangedSubview(btn)
            panelButtons.append(btn)
        }
    }

    // MARK: - Update-ready callout

    /// Show the update callout floating just above the footer dock,
    /// leading-aligned over the bottom-left icon. `.ready` → click to restart,
    /// `.available` → click to install. Idempotent — replaces any existing one.
    func presentUpdateReadyCallout(appName: String, state: UpdateCalloutState) {
        updateReadyCallout?.removeFromSuperview()

        let callout = UpdateReadyCallout(appName: appName, state: state) { [weak self] in
            self?.onUpdateActionRequested?(state)
        }
        callout.translatesAutoresizingMaskIntoConstraints = false
        addSubview(callout, positioned: .above, relativeTo: footerView)
        updateReadyCallout = callout

        NSLayoutConstraint.activate([
            callout.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.footerInset),
            callout.trailingAnchor.constraint(
                lessThanOrEqualTo: footerView.trailingAnchor, constant: -Layout.footerInset),
            callout.bottomAnchor.constraint(equalTo: footerView.topAnchor, constant: -6),
        ])
    }

    /// Remove the callout (e.g. once the update is applied or dismissed).
    func dismissUpdateReadyCallout() {
        updateReadyCallout?.removeFromSuperview()
        updateReadyCallout = nil
    }

    /// Update a panel button's icon from a file:// URL (resolved via resolveLxUri after lxapp installs).
    func updatePanelIcon(panelId: String, iconFileUrl: String) {
        guard let btn = panelButtons.first(where: { $0.identifier?.rawValue == panelId }),
              let url = URL(string: iconFileUrl),
              let image = NSImage(contentsOf: url) else { return }
        btn.image = image
        btn.contentTintColor = nil
    }

    @objc private func panelButtonClicked(_ sender: NSButton) {
        guard let panelId = sender.identifier?.rawValue else { return }
        onPanelItemToggled?(panelId)
    }

    // MARK: - Public API

    /// Rebuild all groups based on current tabs
    func updateForTabs(_ tabs: [LxAppTab], activeTab: LxAppTab?) {
        guard !appUIOnlyMode else { return }
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
        if let docView = scrollView.documentView {
            docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: yOffset)
        }

        if let activeAppId = activeTab?.appId {
            setActiveHighlight(appId: activeAppId)
        }

    }

    /// Refresh a specific app group from Rust data
    func refreshAppGroup(appId: String) {
        guard !appUIOnlyMode else { return }
        groupViews[appId]?.refreshFromRust()
    }

    /// Set active highlight on the appropriate group and item
    func setActiveHighlight(appId: String, pageIndex: Int? = nil) {
        guard !appUIOnlyMode else { return }
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
        guard !appUIOnlyMode else { return }
        for (_, group) in groupViews {
            group.clearHighlight()
        }
        for (_, itemView) in browserItemViews {
            itemView.isSelected = false
        }
    }

    // MARK: - Browser Items

    /// Update browser tab items in the sidebar
    func updateBrowserItems(_ items: [(id: String, title: String, favicon: NSImage?)], activeId: String?) {
        guard !appUIOnlyMode else { return }
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

        // "+" button — only shown when shell (browser) capability is available
        if (LxAppCore.capabilities & LxAppCore.capShell) != 0 {
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
        } else {
            addButton.removeFromSuperview()
            addButtonTopConstraint = nil
        }

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

        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: yOffset)
    }

    /// Re-layout just the browser section (for title updates without full tab rebuild)
    private func relayoutBrowserSection(in docView: NSView) {
        let yOffset = layoutBrowserSection(in: docView, yOffset: yOffsetAfterGroups())
        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: yOffset)
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

    @objc private func hideButtonClicked() {
        onHideRequested?()
    }

    @objc private func settingsClicked() {
        onOpenSettings?()
    }

    @objc private func downloadClicked() {
        onOpenDownloads?()
    }

    // MARK: - Footer / Add button hover

    override public func updateTrackingAreas() {
        super.updateTrackingAreas()

        // Add button hover tracking
        if let existing = addButtonTrackingArea {
            addButton.removeTrackingArea(existing)
        }
        let addArea = NSTrackingArea(
            rect: addButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addButton.addTrackingArea(addArea)
        addButtonTrackingArea = addArea

        // Hide button hover tracking
        if let existing = hideButtonTrackingArea {
            hideButton.removeTrackingArea(existing)
        }
        let hideArea = NSTrackingArea(
            rect: hideButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        hideButton.addTrackingArea(hideArea)
        hideButtonTrackingArea = hideArea
    }

    override public func mouseEntered(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(true)
        } else if event.trackingArea === hideButtonTrackingArea {
            setHideButtonHovered(true)
        }
    }

    override public func mouseExited(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(false)
        } else if event.trackingArea === hideButtonTrackingArea {
            setHideButtonHovered(false)
        }
    }

    private func setAddButtonHovered(_ hovered: Bool) {
        let alpha: CGFloat = hovered ? 0.12 : 0.06
        addButton.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(alpha).cgColor
    }

    private func setHideButtonHovered(_ hovered: Bool) {
        hideButton.layer?.backgroundColor = hovered
            ? NSColor.labelColor.withAlphaComponent(0.09).cgColor
            : NSColor.clear.cgColor
        hideButton.contentTintColor = hovered ? NSColor.labelColor : NSColor.secondaryLabelColor
    }
}

/// NSView subclass with flipped coordinate system (top-left origin)
@MainActor
private class FlippedView: NSView {
    override var isFlipped: Bool { true }
    override var mouseDownCanMoveWindow: Bool { false }
}

#endif
