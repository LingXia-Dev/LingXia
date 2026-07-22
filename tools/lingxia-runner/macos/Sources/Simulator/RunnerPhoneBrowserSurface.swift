import AppKit
import WebKit
import os.log
@_spi(Runner) import lingxia

/// In-app browser chrome for the simulated phone. Self and aside tabs share the
/// surface but keep separate switcher groups and chrome policies.
@MainActor
final class RunnerPhoneBrowserSurface {
    private static let log = OSLog(subsystem: "LingXiaRunner", category: "PhoneBrowserSurface")

    private var overlayView: NSView?
    private var webContainer: NSView?
    private weak var hostWindow: NSWindow?
    private weak var phoneContentView: NSView?

    // Ordered tab model (mirrors iOS LxAppBrowser.openTabIds). Tabs are opened by
    // the lxapp (each `target="self"` navigation calls present) and by the new-tab
    // button; this surface switches, opens, and closes them.
    private var openTabIds: [String] = []
    private var activeTabId: String?
    /// Tabs the user has interacted with (page click or address navigation).
    /// Until then, auto-created history (SPA pushState redirects) must not
    /// light back/forward — mirroring Chrome's history intervention.
    private var interactedTabIds: Set<String> = []
    private var interactionMonitor: Any?
    // Owner of the most recent presentation, used to open new tabs ("+").
    private var ownerAppId: String?
    private var ownerSessionId: UInt64 = 0
    private var dismissible = true

    private var addressField: NSTextField?
    private var addressIcon: NSImageView?
    private var addressPill: NSView?
    private var backButton: NSButton?
    private var forwardButton: NSButton?
    private var refreshButton: NSButton?
    private var asideRefreshButton: NSButton?
    private var newTabButton: NSButton?
    private var tabsButton: NSButton?
    private var tabsBadge: NSTextField?
    private var bottomBarHeightConstraint: NSLayoutConstraint?
    private var actionRowTopWithAddress: NSLayoutConstraint?
    private var actionRowTopWithoutAddress: NSLayoutConstraint?
    private var tabSwitcherOverlay: NSView?
    var onDismiss: (() -> Void)?
    private var urlObservation: NSKeyValueObservation?
    private var canGoBackObservation: NSKeyValueObservation?
    private var canGoForwardObservation: NSKeyValueObservation?

    var activeWebView: WKWebView? {
        guard activeTabId != nil, overlayView?.isHidden == false else { return nil }
        return webContainer?.subviews.compactMap { $0 as? WKWebView }.first
    }

    /// Whether the browser overlay is currently on screen with at least one tab.
    /// Used to route in-page new-tab requests (owned by the builtin browser app)
    /// to the host that is presenting the browser.
    var isPresenting: Bool {
        overlayView?.isHidden == false && !openTabIds.isEmpty
    }

    /// Show `tabId`, registering it. Existing tabs stay open; the displayed
    /// webview is swapped to this tab. `ownerAppId`/`ownerSessionId` are the
    /// lxapp (or builtin browser) that owns the tab, reused to open new tabs.
    func present(
        tabId: String,
        ownerAppId: String,
        ownerSessionId: UInt64,
        in phoneContent: NSView,
        window: NSWindow?,
        dismissible: Bool = true
    ) {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }

        self.ownerAppId = ownerAppId
        self.ownerSessionId = ownerSessionId
        self.dismissible = dismissible
        hostWindow = window
        phoneContentView = phoneContent
        register(tabId: normalized)
        show(in: phoneContent)
        activate(tabId: normalized, allowModeSwitch: true)
    }

    /// Hide the browser while preserving its groups. Window teardown passes
    /// `closeTab` to release the managed tabs as well.
    func dismiss(closeTab: Bool) {
        clearWebViewAttachment()
        dismissTabSwitcher()
        overlayView?.isHidden = true

        if closeTab {
            for tabId in openTabIds {
                _ = RunnerSupport.Browser.closeTab(tabId: tabId)
            }
            openTabIds.removeAll()
            interactedTabIds.removeAll()
            activeTabId = nil
        }
    }

    private func register(tabId: String) {
        if !openTabIds.contains(tabId) {
            openTabIds.append(tabId)
        }
    }

    /// Make an open tab the active, displayed one. Detaches (without closing) the
    /// previous tab's webview and attaches the new one.
    private func activate(tabId: String, allowModeSwitch: Bool = false) {
        guard openTabIds.contains(tabId) else { return }
        if !allowModeSwitch, let activeTabId,
           tabIsAside(activeTabId) != tabIsAside(tabId) {
            return
        }
        activeTabId = tabId
        clearWebViewAttachment()
        // Blank the address and back/forward until the new tab's webview
        // attaches — never show the previous tab's state (they are per-tab).
        // End any in-progress address editing first: updateAddress skips a
        // field that is being edited, which would leave the old URL behind.
        if addressField?.currentEditor() != nil {
            hostWindow?.makeFirstResponder(nil)
        }
        updateAddress(url: nil)
        updateNavigationButtons()
        updateTabsBadge()
        applyActiveModeChrome()
        attachWebView(tabId: tabId, attempt: 0)
    }

    /// Close a tab and select a neighbor in its group; hide the browser when
    /// that group becomes empty.
    private func closeTab(_ tabId: String) {
        guard let index = openTabIds.firstIndex(of: tabId) else { return }
        let wasActive = activeTabId == tabId
        let aside = tabIsAside(tabId)
        let groupIndex = tabIds(forAside: aside).firstIndex(of: tabId) ?? 0

        if !dismissible, wasActive, tabIds(forAside: aside).count == 1 {
            _ = RunnerSupport.Browser.navigate(tabId: tabId, url: "about:blank")
            interactedTabIds.remove(tabId)
            activate(tabId: tabId)
            return
        }

        _ = RunnerSupport.Browser.closeTab(tabId: tabId)
        openTabIds.remove(at: index)
        interactedTabIds.remove(tabId)
        updateTabsBadge()

        guard wasActive else { return }
        let remaining = tabIds(forAside: aside)
        if remaining.isEmpty {
            activeTabId = nil
            dismiss(closeTab: false)
            return
        }
        let neighbor = min(groupIndex, remaining.count - 1)
        activate(tabId: remaining[neighbor])
    }

    private func tabIsAside(_ tabId: String) -> Bool {
        RunnerSupport.Browser.isAside(tabId: tabId)
    }

    private func tabIds(forAside aside: Bool) -> [String] {
        openTabIds.filter { tabIsAside($0) == aside }
    }

    private var activeTabIsAside: Bool {
        activeTabId.map(tabIsAside) ?? false
    }

    private var visibleTabIds: [String] {
        guard activeTabId != nil else { return [] }
        return tabIds(forAside: activeTabIsAside)
    }

    /// Open a fresh blank tab against the presenting owner and switch to it.
    /// The runner bundles no browser webui, so a new tab is `about:blank`
    /// (a blank page) rather than the `lingxia://newtab` start page.
    private func openNewTab() {
        if let tabId = activeTabId, RunnerSupport.Browser.isAside(tabId: tabId) {
            return
        }
        guard let ownerAppId, ownerSessionId > 0,
              let newId = RunnerSupport.Browser.openTab(
                ownerAppId: ownerAppId,
                ownerSessionId: ownerSessionId,
                url: "about:blank"
              )?.trimmingCharacters(in: .whitespacesAndNewlines),
              !newId.isEmpty else {
            os_log("Failed to open new browser tab", log: Self.log, type: .error)
            return
        }
        register(tabId: newId)
        activate(tabId: newId)
    }

    private func setupIfNeeded(in phoneContent: NSView) {
        guard overlayView == nil else { return }

        let overlay = NSView()
        overlay.translatesAutoresizingMaskIntoConstraints = false
        overlay.wantsLayer = true
        overlay.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor

        let webContainer = NSView()
        webContainer.translatesAutoresizingMaskIntoConstraints = false
        webContainer.wantsLayer = true
        overlay.addSubview(webContainer)

        let bottomBar = NSView()
        bottomBar.translatesAutoresizingMaskIntoConstraints = false
        bottomBar.wantsLayer = true
        overlay.addSubview(bottomBar)

        let barBackground = NSVisualEffectView()
        barBackground.translatesAutoresizingMaskIntoConstraints = false
        barBackground.material = .hudWindow
        barBackground.blendingMode = .withinWindow
        barBackground.state = .active
        barBackground.wantsLayer = true
        bottomBar.addSubview(barBackground)

        // Address row: icon + editable URL field + refresh, in a pill.
        let addressPill = NSView()
        addressPill.translatesAutoresizingMaskIntoConstraints = false
        addressPill.wantsLayer = true
        addressPill.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.07).cgColor
        addressPill.layer?.cornerRadius = 16
        addressPill.layer?.borderWidth = 1
        addressPill.layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.7).cgColor
        addressPill.layer?.masksToBounds = true
        barBackground.addSubview(addressPill)

        let addressIcon = NSImageView()
        addressIcon.translatesAutoresizingMaskIntoConstraints = false
        addressIcon.imageScaling = .scaleProportionallyDown
        addressPill.addSubview(addressIcon)

        let addressField = NSTextField()
        addressField.translatesAutoresizingMaskIntoConstraints = false
        addressField.isBordered = false
        addressField.drawsBackground = false
        addressField.focusRingType = .none
        addressField.font = NSFont.systemFont(ofSize: 13)
        addressField.placeholderString = L10n.Browser.addressPlaceholder
        addressField.lineBreakMode = .byTruncatingMiddle
        addressField.usesSingleLineMode = true
        addressField.target = self
        addressField.action = #selector(addressSubmitted)
        addressPill.addSubview(addressField)

        let refreshButton = makeIconButton(named: "icon_browser_refresh", action: #selector(refreshClicked), side: 30)
        addressPill.addSubview(refreshButton)

        // Action row: back / forward — spacer — new-tab / tabs / close, matching
        // the iOS browser. (No downloads/settings overflow menu: those are
        // `lingxia://` pages and the runner bundles no browser webui for them.)
        let actionRow = NSStackView()
        actionRow.translatesAutoresizingMaskIntoConstraints = false
        actionRow.orientation = .horizontal
        actionRow.alignment = .centerY
        actionRow.spacing = 4
        barBackground.addSubview(actionRow)

        let backButton = makeIconButton(named: "icon_back", action: #selector(backClicked))
        let forwardButton = makeIconButton(named: "icon_forward", action: #selector(forwardClicked))
        let asideRefreshButton = makeIconButton(named: "icon_browser_refresh", action: #selector(refreshClicked))
        asideRefreshButton.isHidden = true
        let newTabButton = makeIconButton(named: "icon_browser_plus", action: #selector(newTabClicked))
        let tabsButton = makeIconButton(named: "icon_browser_tabs", action: #selector(tabsClicked))
        let closeButton = makeIconButton(named: "icon_close_x", action: #selector(closeClicked))
        closeButton.isHidden = !dismissible

        let spacer = NSView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)

        // Open-tab count overlaid on the tabs glyph.
        let tabsBadge = NSTextField(labelWithString: "0")
        tabsBadge.translatesAutoresizingMaskIntoConstraints = false
        tabsBadge.font = NSFont.systemFont(ofSize: 9, weight: .semibold)
        tabsBadge.textColor = NSColor.labelColor
        tabsBadge.alignment = .center
        tabsButton.addSubview(tabsBadge)

        actionRow.addArrangedSubview(backButton)
        actionRow.addArrangedSubview(forwardButton)
        actionRow.addArrangedSubview(asideRefreshButton)
        actionRow.addArrangedSubview(spacer)
        actionRow.addArrangedSubview(newTabButton)
        actionRow.addArrangedSubview(tabsButton)
        actionRow.addArrangedSubview(closeButton)

        self.backButton = backButton
        self.forwardButton = forwardButton
        self.refreshButton = refreshButton
        self.asideRefreshButton = asideRefreshButton
        self.newTabButton = newTabButton
        self.tabsButton = tabsButton
        self.tabsBadge = tabsBadge
        self.addressField = addressField
        self.addressIcon = addressIcon
        self.addressPill = addressPill

        phoneContent.addSubview(overlay, positioned: .above, relativeTo: nil)
        NSLayoutConstraint.activate([
            overlay.topAnchor.constraint(equalTo: phoneContent.topAnchor),
            overlay.leadingAnchor.constraint(equalTo: phoneContent.leadingAnchor),
            overlay.trailingAnchor.constraint(equalTo: phoneContent.trailingAnchor),
            overlay.bottomAnchor.constraint(equalTo: phoneContent.bottomAnchor),

            webContainer.topAnchor.constraint(equalTo: overlay.topAnchor),
            webContainer.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
            webContainer.trailingAnchor.constraint(equalTo: overlay.trailingAnchor),
            webContainer.bottomAnchor.constraint(equalTo: bottomBar.topAnchor),

            bottomBar.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
            bottomBar.trailingAnchor.constraint(equalTo: overlay.trailingAnchor),
            bottomBar.bottomAnchor.constraint(equalTo: overlay.bottomAnchor),

            barBackground.leadingAnchor.constraint(equalTo: bottomBar.leadingAnchor),
            barBackground.trailingAnchor.constraint(equalTo: bottomBar.trailingAnchor),
            barBackground.topAnchor.constraint(equalTo: bottomBar.topAnchor),
            barBackground.bottomAnchor.constraint(equalTo: bottomBar.bottomAnchor),

            addressPill.leadingAnchor.constraint(equalTo: barBackground.leadingAnchor, constant: 10),
            addressPill.trailingAnchor.constraint(equalTo: barBackground.trailingAnchor, constant: -10),
            addressPill.topAnchor.constraint(equalTo: barBackground.topAnchor, constant: 8),
            addressPill.heightAnchor.constraint(equalToConstant: 34),

            addressIcon.leadingAnchor.constraint(equalTo: addressPill.leadingAnchor, constant: 10),
            addressIcon.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),
            addressIcon.widthAnchor.constraint(equalToConstant: 14),
            addressIcon.heightAnchor.constraint(equalToConstant: 14),

            addressField.leadingAnchor.constraint(equalTo: addressIcon.trailingAnchor, constant: 6),
            addressField.trailingAnchor.constraint(equalTo: refreshButton.leadingAnchor, constant: -4),
            addressField.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),

            refreshButton.trailingAnchor.constraint(equalTo: addressPill.trailingAnchor, constant: -2),
            refreshButton.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),

            actionRow.leadingAnchor.constraint(equalTo: barBackground.leadingAnchor, constant: 6),
            actionRow.trailingAnchor.constraint(equalTo: barBackground.trailingAnchor, constant: -6),

            tabsBadge.centerXAnchor.constraint(equalTo: tabsButton.centerXAnchor, constant: 1),
            tabsBadge.centerYAnchor.constraint(equalTo: tabsButton.centerYAnchor, constant: -1),
        ])

        let bottomBarHeight = bottomBar.heightAnchor.constraint(equalToConstant: 96)
        let actionTopWithAddress = actionRow.topAnchor.constraint(equalTo: addressPill.bottomAnchor, constant: 4)
        let actionTopWithoutAddress = actionRow.topAnchor.constraint(equalTo: barBackground.topAnchor, constant: 8)
        bottomBarHeight.isActive = true
        actionTopWithAddress.isActive = true
        bottomBarHeightConstraint = bottomBarHeight
        actionRowTopWithAddress = actionTopWithAddress
        actionRowTopWithoutAddress = actionTopWithoutAddress

        overlayView = overlay
        self.webContainer = webContainer
        updateNavigationButtons()
        updateTabsBadge()
    }

    /// Compact aside uses a single action row. Address editing and tab creation
    /// remain exclusive to self browser chrome.
    private func applyActiveModeChrome() {
        guard let tabId = activeTabId else { return }
        let aside = tabIsAside(tabId)
        newTabButton?.isHidden = aside
        asideRefreshButton?.isHidden = !aside
        if aside, addressField?.currentEditor() != nil {
            hostWindow?.makeFirstResponder(nil)
        }
        addressField?.isEditable = !aside
        addressField?.isSelectable = !aside
        addressPill?.isHidden = aside
        if aside {
            actionRowTopWithAddress?.isActive = false
            actionRowTopWithoutAddress?.isActive = true
        } else {
            actionRowTopWithoutAddress?.isActive = false
            actionRowTopWithAddress?.isActive = true
        }
        bottomBarHeightConstraint?.constant = aside ? 56 : 96
    }

    private func show(in phoneContent: NSView) {
        setupIfNeeded(in: phoneContent)
        // First click inside the web area marks the active tab as interacted
        // (observe-only; the event passes through untouched).
        if interactionMonitor == nil {
            interactionMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
                MainActor.assumeIsolated {
                    guard let self, let container = self.webContainer,
                          let window = container.window, event.window === window,
                          self.overlayView?.isHidden == false else { return }
                    let point = container.convert(event.locationInWindow, from: nil)
                    if container.bounds.contains(point) {
                        self.markActiveTabInteracted()
                    }
                }
                return event
            }
        }
        guard let overlay = overlayView else { return }

        phoneContent.addSubview(overlay, positioned: .above, relativeTo: nil)
        overlay.isHidden = false
        phoneContent.needsLayout = true
        phoneContent.layoutSubtreeIfNeeded()
    }

    private func attachWebView(tabId: String, attempt: Int) {
        guard activeTabId == tabId else { return }

        if let webView = RunnerSupport.Browser.webView(tabId: tabId),
           let webContainer {
            invalidateObservations()
            RunnerSupport.WebView.attach(webView, to: webContainer)
            observe(webView)
            updateAddress(url: webView.url)
            updateNavigationButtons()
            return
        }

        guard attempt < 20 else {
            os_log("Failed to attach browser webview after retries tab=%@", log: Self.log, type: .error, tabId)
            closeTab(tabId)
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            self?.attachWebView(tabId: tabId, attempt: attempt + 1)
        }
    }

    private func clearWebViewAttachment() {
        invalidateObservations()
        webContainer?.subviews.forEach { subview in
            subview.removeFromSuperview()
        }
    }

    private func observe(_ webView: WKWebView) {
        urlObservation = webView.observe(\.url, options: [.initial, .new]) { [weak self] webView, _ in
            Task { @MainActor in
                self?.updateAddress(url: webView.url)
            }
        }
        canGoBackObservation = webView.observe(\.canGoBack, options: [.initial, .new]) { [weak self] _, _ in
            Task { @MainActor in
                self?.updateNavigationButtons()
            }
        }
        canGoForwardObservation = webView.observe(\.canGoForward, options: [.initial, .new]) { [weak self] _, _ in
            Task { @MainActor in
                self?.updateNavigationButtons()
            }
        }
    }

    private func invalidateObservations() {
        urlObservation?.invalidate()
        canGoBackObservation?.invalidate()
        canGoForwardObservation?.invalidate()
        urlObservation = nil
        canGoBackObservation = nil
        canGoForwardObservation = nil
    }

    private func updateAddress(url: URL?) {
        guard addressField?.currentEditor() == nil else { return }
        // A blank new tab (about:blank) reads as empty, like a fresh tab.
        if url == nil || url?.absoluteString == "about:blank" {
            addressField?.stringValue = ""
            addressIcon?.isHidden = true
            return
        }
        addressIcon?.isHidden = false
        addressField?.stringValue = url?.absoluteString ?? ""
    }

    private func updateNavigationButtons() {
        let webView = activeWebView
        let interacted = activeTabId.map(interactedTabIds.contains) ?? false
        let back = (webView?.canGoBack ?? false) && interacted
        let forward = (webView?.canGoForward ?? false) && interacted
        backButton?.isEnabled = back
        backButton?.alphaValue = back ? 1.0 : 0.35
        forwardButton?.isEnabled = forward
        forwardButton?.alphaValue = forward ? 1.0 : 0.35
        refreshButton?.isEnabled = webView != nil
        refreshButton?.alphaValue = webView == nil ? 0.35 : 1.0
        asideRefreshButton?.isEnabled = webView != nil
        asideRefreshButton?.alphaValue = webView == nil ? 0.35 : 1.0
    }

    /// Mark the active tab as user-interacted and refresh the nav affordances.
    private func markActiveTabInteracted() {
        guard let tabId = activeTabId, !interactedTabIds.contains(tabId) else { return }
        interactedTabIds.insert(tabId)
        updateNavigationButtons()
    }

    private func updateTabsBadge() {
        tabsBadge?.stringValue = String(visibleTabIds.count)
    }

    private func tabLabel(forTabId tabId: String) -> String {
        if let webView = RunnerSupport.Browser.webView(tabId: tabId) {
            if let title = webView.title, !title.isEmpty { return title }
            if let host = webView.url?.host, !host.isEmpty { return host }
        }
        return "New Tab"
    }

    private func makeIconButton(named iconName: String, action: Selector, side: CGFloat = 38) -> NSButton {
        let button = NSButton()
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.image = RunnerSupport.Assets.image(named: iconName, size: CGSize(width: 20, height: 20))
        button.imageScaling = .scaleProportionallyDown
        button.target = self
        button.action = action
        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: side),
            button.heightAnchor.constraint(equalToConstant: side),
        ])
        return button
    }

    // MARK: - Tab switcher

    /// The switcher is an in-frame bottom sheet (a subview of the browser overlay)
    /// rather than an NSPopover — a popover floats relative to the screen and
    /// escapes the simulated phone's window. Mirrors the iOS tab-switcher sheet.
    @objc private func tabsClicked() {
        if tabSwitcherOverlay != nil {
            dismissTabSwitcher()
            return
        }
        guard let overlay = overlayView else { return }
        presentTabSwitcher(in: overlay)
    }

    private func dismissTabSwitcher() {
        tabSwitcherOverlay?.removeFromSuperview()
        tabSwitcherOverlay = nil
    }

    private func presentTabSwitcher(in overlay: NSView) {
        let dim = TapCatcherView()
        dim.translatesAutoresizingMaskIntoConstraints = false
        dim.wantsLayer = true
        dim.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.35).cgColor
        dim.onClick = { [weak self] in self?.dismissTabSwitcher() }
        overlay.addSubview(dim, positioned: .above, relativeTo: nil)

        let panel = SheetPanelView()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.material = .hudWindow
        panel.blendingMode = .withinWindow
        panel.state = .active
        panel.wantsLayer = true
        panel.layer?.cornerRadius = 18
        // Round only the top edge (maxY in the unflipped layer geometry).
        panel.layer?.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        panel.layer?.masksToBounds = true
        dim.addSubview(panel)

        let title = NSTextField(labelWithString: "Tabs")
        title.translatesAutoresizingMaskIntoConstraints = false
        title.font = NSFont.systemFont(ofSize: 15, weight: .semibold)
        panel.addSubview(title)

        let list = NSStackView()
        list.translatesAutoresizingMaskIntoConstraints = false
        list.orientation = .vertical
        list.alignment = .leading
        list.spacing = 2
        panel.addSubview(list)

        NSLayoutConstraint.activate([
            dim.topAnchor.constraint(equalTo: overlay.topAnchor),
            dim.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
            dim.trailingAnchor.constraint(equalTo: overlay.trailingAnchor),
            dim.bottomAnchor.constraint(equalTo: overlay.bottomAnchor),

            panel.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: overlay.trailingAnchor),
            panel.bottomAnchor.constraint(equalTo: overlay.bottomAnchor),
            panel.heightAnchor.constraint(lessThanOrEqualTo: overlay.heightAnchor, multiplier: 0.6),

            title.topAnchor.constraint(equalTo: panel.topAnchor, constant: 14),
            title.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 16),

            list.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 8),
            list.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 8),
            list.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -8),
            list.bottomAnchor.constraint(equalTo: panel.bottomAnchor, constant: -16),
        ])

        tabSwitcherOverlay = dim
        for tabId in visibleTabIds {
            list.addArrangedSubview(makeTabSwitcherRow(tabId: tabId, width: overlay.bounds.width - 16))
        }
    }

    private func makeTabSwitcherRow(tabId: String, width: CGFloat) -> NSView {
        let row = TabRowView()
        row.translatesAutoresizingMaskIntoConstraints = false
        row.onSelect = { [weak self] in
            self?.dismissTabSwitcher()
            self?.activate(tabId: tabId)
        }

        let isActive = activeTabId == tabId
        let label = NSTextField(labelWithString: tabLabel(forTabId: tabId))
        label.translatesAutoresizingMaskIntoConstraints = false
        label.font = NSFont.systemFont(ofSize: 13, weight: isActive ? .semibold : .regular)
        label.textColor = isActive ? NSColor.labelColor : NSColor.secondaryLabelColor
        label.lineBreakMode = .byTruncatingTail
        label.isEditable = false
        label.isSelectable = false
        label.drawsBackground = false
        label.isBordered = false
        row.addSubview(label)

        let close = NSButton()
        close.translatesAutoresizingMaskIntoConstraints = false
        close.isBordered = false
        close.image = RunnerSupport.Assets.image(named: "icon_close_x", size: CGSize(width: 14, height: 14))
        close.imageScaling = .scaleProportionallyDown
        close.target = self
        close.action = #selector(closeTabFromSwitcher(_:))
        close.identifier = NSUserInterfaceItemIdentifier(tabId)
        row.addSubview(close)

        NSLayoutConstraint.activate([
            row.heightAnchor.constraint(equalToConstant: 40),
            row.widthAnchor.constraint(equalToConstant: max(width, 120)),
            label.leadingAnchor.constraint(equalTo: row.leadingAnchor, constant: 10),
            label.trailingAnchor.constraint(equalTo: close.leadingAnchor, constant: -8),
            label.centerYAnchor.constraint(equalTo: row.centerYAnchor),
            close.trailingAnchor.constraint(equalTo: row.trailingAnchor, constant: -8),
            close.centerYAnchor.constraint(equalTo: row.centerYAnchor),
            close.widthAnchor.constraint(equalToConstant: 28),
            close.heightAnchor.constraint(equalToConstant: 28),
        ])
        return row
    }

    @objc private func closeTabFromSwitcher(_ sender: NSButton) {
        guard let tabId = sender.identifier?.rawValue else { return }
        closeTab(tabId)
        if visibleTabIds.isEmpty {
            dismissTabSwitcher()
        } else {
            // Rebuild the list to drop the closed row / refresh active styling.
            dismissTabSwitcher()
            if let overlay = overlayView { presentTabSwitcher(in: overlay) }
        }
    }

    // MARK: - Actions

    @objc private func closeClicked() {
        dismiss(closeTab: false)
        onDismiss?()
    }

    @objc private func newTabClicked() {
        openNewTab()
    }

    @objc private func backClicked() {
        guard let webView = activeWebView, webView.canGoBack else { return }
        webView.goBack()
    }

    @objc private func forwardClicked() {
        guard let webView = activeWebView, webView.canGoForward else { return }
        webView.goForward()
    }

    @objc private func refreshClicked() {
        activeWebView?.reload()
    }

    @objc private func addressSubmitted() {
        guard let tabId = activeTabId else { return }
        guard !RunnerSupport.Browser.isAside(tabId: tabId) else {
            hostWindow?.makeFirstResponder(nil)
            updateAddress(url: activeWebView?.url)
            return
        }
        // Parse the typed address here (like iOS LxAppBrowser): the runner's
        // native lib has no `browser-shell` feature, so the rust address-input
        // resolver is unavailable. A full URL loads as-is; a bare host gets
        // https; anything else is left to the page (no search provider here).
        let input = (addressField?.stringValue ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !input.isEmpty else { return }
        let target: URL?
        if let url = URL(string: input),
           let scheme = url.scheme?.lowercased(),
           scheme == "http" || scheme == "https" || scheme == "lingxia" {
            target = url
        } else if !input.contains(" "), input.contains("."), let url = URL(string: "https://\(input)") {
            target = url
        } else {
            target = nil
        }
        guard let target else {
            updateAddress(url: activeWebView?.url)
            return
        }
        hostWindow?.makeFirstResponder(nil)
        // An address-bar navigation is a user interaction.
        markActiveTabInteracted()
        // Navigate via the managed runtime (a raw WKWebView.load is ignored by
        // the browser's navigation policy), then re-attach: leaving a blank tab
        // can swap in a fresh webview, so the displayed one would be stale.
        if RunnerSupport.Browser.navigate(tabId: tabId, url: target.absoluteString) {
            updateAddress(url: target)
            attachWebView(tabId: tabId, attempt: 0)
        } else {
            updateAddress(url: activeWebView?.url)
        }
    }
}

/// A row for the tab switcher: clicking the row activates its tab. The trailing
/// close button is a subview, so AppKit routes its clicks to it directly; only
/// clicks on the rest of the row reach `mouseDown`.
@MainActor
private final class TabRowView: NSView {
    var onSelect: (() -> Void)?

    override func mouseDown(with event: NSEvent) {
        onSelect?()
    }
}

/// Full-bleed dim backdrop for the tab-switcher sheet: a click anywhere on it
/// (outside the panel, which sits above and consumes its own clicks) dismisses.
@MainActor
private final class TapCatcherView: NSView {
    var onClick: (() -> Void)?

    override func mouseDown(with event: NSEvent) {
        onClick?()
    }
}

/// The sheet panel swallows clicks so a tap on its chrome (title / empty space)
/// doesn't bubble up to the dim backdrop and dismiss the sheet.
@MainActor
private final class SheetPanelView: NSVisualEffectView {
    override func mouseDown(with event: NSEvent) {}
}
