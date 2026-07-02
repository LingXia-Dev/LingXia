#if os(macOS)
import AppKit
import WebKit
import OSLog
import CLingXiaRustAPI

/// A multi-tab browser docked beside the main content as an aside. Hosts one or
/// more external-content tabs (https/file) behind a Chrome-style title tab strip
/// — there is **no address input** (that is the only thing distinguishing the
/// aside from the self/main browser). Tabs are opened only through the API
/// (`openSurface({ url, as: 'aside' })`), deduped by URL; each tab is a real
/// LingXia browser webview (created via the Rust browser path) so it takes
/// native input and drives back/forward.
///
/// One panel per window: every web-aside surface node becomes a tab here. The
/// panel is owned by `BrowserTabCoordinator`; `LxAppSurface` routes each node to
/// `addOrFocusTab` / `removeTab`. Closing the last tab tears the panel down.
@MainActor
final class DockedBrowser: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "DockedBrowser")
    private static let maxAttachRetry = 30
    private static let attachRetryDelay: TimeInterval = 0.1

    private enum Layout {
        static let toolbarHeight: CGFloat = 38
        static let buttonSize: CGFloat = 28
        static let closeSize: CGFloat = 24
        static let iconSize: CGFloat = 14
        static let edge: CGFloat = 8
        static let maxTabWidth: CGFloat = 180
    }

    /// One open tab: a web-aside surface node backed by a Rust browser tab.
    @MainActor
    private final class Tab {
        let surfaceId: String
        let browserTabId: String
        var url: String
        var title: String = ""
        var webView: WKWebView?
        /// Chrome-style history intervention: until the user interacts with
        /// the page, auto-created history (SPA pushState redirects) must not
        /// light up back/forward.
        var userInteracted = false
        let button = NSButton()
        let closeButton = NSButton()
        let row = NSStackView()
        nonisolated(unsafe) var urlObs: NSKeyValueObservation?
        nonisolated(unsafe) var backObs: NSKeyValueObservation?
        nonisolated(unsafe) var forwardObs: NSKeyValueObservation?
        nonisolated(unsafe) var titleObs: NSKeyValueObservation?
        init(surfaceId: String, browserTabId: String, url: String) {
            self.surfaceId = surfaceId
            self.browserTabId = browserTabId
            self.url = url
        }
        func invalidate() {
            urlObs?.invalidate(); backObs?.invalidate()
            forwardObs?.invalidate(); titleObs?.invalidate()
            urlObs = nil; backObs = nil; forwardObs = nil; titleObs = nil
        }
    }

    /// Root view handed to the aside panel slot.
    let containerView = NSView()

    private let owner: (appId: String, sessionId: UInt64)
    /// A tab's X was clicked: close that surface node (routes back through the
    /// core so the graph stays in sync).
    private let onCloseTab: (String) -> Void
    /// The close-aside affordance was clicked: close every tab/node.
    private let onCloseAside: () -> Void

    private let toolbar = NSView()
    private let backButton = NSButton()
    private let forwardButton = NSButton()
    private let refreshButton = NSButton()
    private let closeAsideButton = NSButton()
    private let tabStrip = NSStackView()
    private let separator = NSView()
    private let webContainer = NSView()

    private var tabs: [Tab] = []
    private var activeSurfaceId: String?
    private var torn = false
    private var interactionMonitor: Any?

    /// Create the panel with an initial tab for `surfaceId` → `url`. Returns nil
    /// if the first browser tab could not be created.
    init?(
        owner: (appId: String, sessionId: UInt64),
        surfaceId: String,
        url: String,
        onCloseTab: @escaping (String) -> Void,
        onCloseAside: @escaping () -> Void
    ) {
        self.owner = owner
        self.onCloseTab = onCloseTab
        self.onCloseAside = onCloseAside
        super.init()
        buildChrome()
        guard addOrFocusTab(surfaceId: surfaceId, url: url) else { return nil }
        // First click inside the web area marks the active tab as interacted
        // (observe-only; the event passes through untouched).
        interactionMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
            MainActor.assumeIsolated {
                self?.noteInteractionIfInWebArea(event)
            }
            return event
        }
    }

    /// Mark the active tab as user-interacted when a click lands in its web
    /// content, then refresh back/forward (they stay dimmed until first
    /// interaction, mirroring Chrome's history-manipulation intervention).
    private func noteInteractionIfInWebArea(_ event: NSEvent) {
        guard !torn,
              let window = webContainer.window,
              event.window === window,
              let tab = activeTab(),
              !tab.userInteracted else { return }
        let point = webContainer.convert(event.locationInWindow, from: nil)
        guard webContainer.bounds.contains(point) else { return }
        tab.userInteracted = true
        if let webView = tab.webView {
            updateBackForward(canGoBack: webView.canGoBack, canGoForward: webView.canGoForward)
        }
    }

    /// The active tab's surface id, so the surface layer can re-anchor the shell
    /// panel to a survivor when the anchor node closes.
    var anchorSurfaceId: String? { tabs.first?.surfaceId }

    /// Surface ids of every open tab (front = anchor).
    var tabSurfaceIds: [String] { tabs.map { $0.surfaceId } }

    func contains(surfaceId: String) -> Bool {
        tabs.contains { $0.surfaceId == surfaceId }
    }

    // MARK: - Tab management

    /// Add a tab for `url`, or focus an existing tab with the same URL (dedup).
    /// Returns false only if the underlying browser tab could not be created.
    @discardableResult
    func addOrFocusTab(surfaceId: String, url: String) -> Bool {
        if torn { return false }
        if let existing = tabs.first(where: { Self.sameURL($0.url, url) }) {
            activate(existing.surfaceId)
            return true
        }
        guard let opened = openStandaloneBrowserTab(owner.appId, owner.sessionId, url) else {
            os_log("openStandaloneBrowserTab failed url=%{public}@", log: Self.log, type: .error, url)
            return false
        }
        let browserTabId = opened.toString().trimmingCharacters(in: .whitespacesAndNewlines)
        guard !browserTabId.isEmpty else { return false }
        let tab = Tab(surfaceId: surfaceId, browserTabId: browserTabId, url: url)
        tab.title = Self.shortTitle(for: url)
        tabs.append(tab)
        buildTabRow(tab)
        attachWhenReady(tab: tab, attempt: 0)
        activate(surfaceId)
        return true
    }

    /// Remove the tab for `surfaceId`. Returns true if the panel is now empty
    /// (the caller should tear it down).
    @discardableResult
    func removeTab(surfaceId: String) -> Bool {
        guard let idx = tabs.firstIndex(where: { $0.surfaceId == surfaceId }) else {
            return tabs.isEmpty
        }
        let tab = tabs.remove(at: idx)
        teardownTab(tab)
        if activeSurfaceId == surfaceId {
            activeSurfaceId = nil
            if let next = tabs.first { activate(next.surfaceId) }
        }
        updateTabStripSelection()
        return tabs.isEmpty
    }

    func tearDown() {
        guard !torn else { return }
        torn = true
        if let monitor = interactionMonitor {
            NSEvent.removeMonitor(monitor)
            interactionMonitor = nil
        }
        for tab in tabs { teardownTab(tab) }
        tabs.removeAll()
        activeSurfaceId = nil
    }

    func containsBrowserTab(_ browserTabId: String) -> Bool {
        tabs.contains { $0.browserTabId == browserTabId }
    }

    /// Activate the tab backing `browserTabId` and make its webview first
    /// responder in place (native input prep) — never relocates it out of the
    /// aside. Returns false until the webview has attached.
    func focusForInput(browserTabId: String) -> Bool {
        guard let tab = tabs.first(where: { $0.browserTabId == browserTabId }) else { return false }
        if tab.surfaceId != activeSurfaceId { activate(tab.surfaceId) }
        guard let webView = tab.webView, let window = webView.window else { return false }
        window.makeFirstResponder(webView)
        return true
    }

    private func activeTab() -> Tab? {
        guard let id = activeSurfaceId else { return nil }
        return tabs.first { $0.surfaceId == id }
    }

    private func activate(_ surfaceId: String) {
        guard let tab = tabs.first(where: { $0.surfaceId == surfaceId }) else { return }
        activeSurfaceId = surfaceId
        // Show only the active tab's webview.
        for other in tabs where other.surfaceId != surfaceId {
            other.webView?.isHidden = true
        }
        if let webView = tab.webView {
            webView.isHidden = false
            if webView.superview !== webContainer {
                WebViewManager.attachWebViewToContainer(webView, container: webContainer)
            }
            webView.window?.makeFirstResponder(webView)
            updateBackForward(canGoBack: webView.canGoBack, canGoForward: webView.canGoForward)
        } else {
            updateBackForward(canGoBack: false, canGoForward: false)
        }
        updateTabStripSelection()
    }

    private func teardownTab(_ tab: Tab) {
        tab.invalidate()
        tab.webView?.stopLoading()
        tab.webView?.removeFromSuperview()
        tab.webView = nil
        // removeArrangedSubview FIRST: removeFromSuperview already detaches
        // the view from the arranged list, and removing a no-longer-arranged
        // view raises an NSStackView assertion.
        tabStrip.removeArrangedSubview(tab.row)
        tab.row.removeFromSuperview()
        _ = browserTabClose(tab.browserTabId)
    }

    // MARK: - WebView wiring

    private func attachWhenReady(tab: Tab, attempt: Int) {
        guard !torn, tabs.contains(where: { $0 === tab }) else { return }
        if let webView = resolveWebView(browserTabId: tab.browserTabId) {
            attach(webView, to: tab)
            return
        }
        guard attempt < Self.maxAttachRetry else {
            os_log("docked tab webview never ready tab=%{public}@", log: Self.log, type: .error, tab.browserTabId)
            onCloseTab(tab.surfaceId)
            return
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.attachRetryDelay) { [weak self, weak tab] in
            guard let self, let tab else { return }
            self.attachWhenReady(tab: tab, attempt: attempt + 1)
        }
    }

    private func resolveWebView(browserTabId: String) -> WKWebView? {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else { return nil }
        let path = browserTabPathForId(browserTabId).toString()
        return WebViewManager.resolveWebView(appId: appId, path: path, sessionId: sessionId)
    }

    private func attach(_ webView: WKWebView, to tab: Tab) {
        if #available(macOS 13.3, *) { webView.isInspectable = true }
        tab.webView = webView
        observe(webView, tab: tab)
        if tab.surfaceId == activeSurfaceId {
            activate(tab.surfaceId)
        } else {
            webView.isHidden = true
        }
    }

    private func observe(_ webView: WKWebView, tab: Tab) {
        tab.urlObs = webView.observe(\.url, options: [.initial, .new]) { [weak self, weak tab] webView, _ in
            Task { @MainActor in
                guard let self, let tab, !self.torn else { return }
                tab.url = webView.url?.absoluteString ?? tab.url
                _ = updateBrowserTabInfo(tab.browserTabId, webView.url?.absoluteString ?? "", webView.title ?? "")
            }
        }
        tab.titleObs = webView.observe(\.title, options: [.initial, .new]) { [weak self, weak tab] webView, _ in
            Task { @MainActor in
                guard let self, let tab, !self.torn else { return }
                let t = (webView.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                tab.title = t.isEmpty ? Self.shortTitle(for: tab.url) : t
                self.updateTabButtonTitle(tab)
            }
        }
        tab.backObs = webView.observe(\.canGoBack, options: [.initial, .new]) { [weak self, weak tab] webView, _ in
            Task { @MainActor in
                guard let self, let tab, !self.torn, tab.surfaceId == self.activeSurfaceId else { return }
                self.updateBackForward(canGoBack: webView.canGoBack, canGoForward: webView.canGoForward)
            }
        }
        tab.forwardObs = webView.observe(\.canGoForward, options: [.initial, .new]) { [weak self, weak tab] webView, _ in
            Task { @MainActor in
                guard let self, let tab, !self.torn, tab.surfaceId == self.activeSurfaceId else { return }
                self.updateBackForward(canGoBack: webView.canGoBack, canGoForward: webView.canGoForward)
            }
        }
    }

    private func updateBackForward(canGoBack: Bool, canGoForward: Bool) {
        // Pre-interaction history is auto-created (redirects/pushState) and
        // must not light the affordances.
        let interacted = activeTab()?.userInteracted ?? false
        NavButtonState.apply(backButton, enabled: canGoBack && interacted)
        NavButtonState.apply(forwardButton, enabled: canGoForward && interacted)
    }

    // MARK: - Chrome

    private func buildChrome() {
        containerView.wantsLayer = true
        containerView.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor

        toolbar.translatesAutoresizingMaskIntoConstraints = false
        containerView.addSubview(toolbar)

        configureToolButton(backButton, iconName: "icon_back", action: #selector(backClicked))
        configureToolButton(forwardButton, iconName: "icon_forward", action: #selector(forwardClicked))
        configureToolButton(refreshButton, iconName: "icon_browser_refresh", action: #selector(refreshClicked))
        toolbar.addSubview(backButton)
        toolbar.addSubview(forwardButton)
        toolbar.addSubview(refreshButton)

        closeAsideButton.translatesAutoresizingMaskIntoConstraints = false
        closeAsideButton.isBordered = false
        closeAsideButton.imagePosition = .imageOnly
        closeAsideButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close aside")
        closeAsideButton.contentTintColor = NSColor.secondaryLabelColor
        closeAsideButton.toolTip = "Close"
        closeAsideButton.target = self
        closeAsideButton.action = #selector(closeAsideClicked)
        toolbar.addSubview(closeAsideButton)

        // Title tab strip in the SAME bar. Tabs sit adjacent, leading-packed:
        // equal widths capped at maxTabWidth, shrinking evenly when crowded
        // (the strip grows with its tabs instead of filling the whole bar).
        tabStrip.orientation = .horizontal
        tabStrip.spacing = 4
        tabStrip.alignment = .centerY
        tabStrip.distribution = .fillEqually
        tabStrip.translatesAutoresizingMaskIntoConstraints = false
        toolbar.addSubview(tabStrip)

        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.wantsLayer = true
        separator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        containerView.addSubview(separator)

        webContainer.translatesAutoresizingMaskIntoConstraints = false
        webContainer.wantsLayer = true
        containerView.addSubview(webContainer)

        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: containerView.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: Layout.toolbarHeight),

            backButton.leadingAnchor.constraint(equalTo: toolbar.leadingAnchor, constant: Layout.edge),
            backButton.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            backButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            backButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            forwardButton.leadingAnchor.constraint(equalTo: backButton.trailingAnchor, constant: 4),
            forwardButton.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            forwardButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            forwardButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            refreshButton.leadingAnchor.constraint(equalTo: forwardButton.trailingAnchor, constant: 4),
            refreshButton.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            refreshButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            refreshButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            closeAsideButton.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor, constant: -Layout.edge),
            closeAsideButton.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            closeAsideButton.widthAnchor.constraint(equalToConstant: Layout.closeSize),
            closeAsideButton.heightAnchor.constraint(equalToConstant: Layout.closeSize),

            // Tabs live in the SAME bar as back/forward/refresh (one row),
            // leading-packed; the strip may grow up to the close-aside button.
            tabStrip.leadingAnchor.constraint(equalTo: refreshButton.trailingAnchor, constant: Layout.edge),
            tabStrip.trailingAnchor.constraint(lessThanOrEqualTo: closeAsideButton.leadingAnchor, constant: -Layout.edge),
            tabStrip.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            tabStrip.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            separator.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            separator.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            separator.heightAnchor.constraint(equalToConstant: 1),

            webContainer.topAnchor.constraint(equalTo: separator.bottomAnchor),
            webContainer.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            webContainer.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            webContainer.bottomAnchor.constraint(equalTo: containerView.bottomAnchor),
        ])

        updateBackForward(canGoBack: false, canGoForward: false)
    }

    private func configureToolButton(_ button: NSButton, iconName: String, action: Selector) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.imagePosition = .imageOnly
        button.imageScaling = .scaleProportionallyDown
        button.target = self
        button.action = action
        button.image = LxIcon.image(named: iconName, size: CGSize(width: Layout.iconSize, height: Layout.iconSize))
        button.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
    }

    /// A tab chip: a title button (activates) + a small close button.
    private func buildTabRow(_ tab: Tab) {
        let title = tab.button
        title.translatesAutoresizingMaskIntoConstraints = false
        title.isBordered = false
        title.bezelStyle = .regularSquare
        title.font = .systemFont(ofSize: 12)
        title.alignment = .left
        title.lineBreakMode = .byTruncatingTail
        title.title = tab.title
        title.contentTintColor = .labelColor
        title.target = self
        title.action = #selector(tabButtonClicked(_:))
        objc_setAssociatedObject(title, &AssociatedKeys.surfaceId, tab.surfaceId, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)

        let close = tab.closeButton
        close.translatesAutoresizingMaskIntoConstraints = false
        close.isBordered = false
        close.imagePosition = .imageOnly
        close.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close tab")
        close.contentTintColor = .tertiaryLabelColor
        close.target = self
        close.action = #selector(tabCloseClicked(_:))
        objc_setAssociatedObject(close, &AssociatedKeys.surfaceId, tab.surfaceId, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)

        let row = tab.row
        row.orientation = .horizontal
        row.spacing = 2
        row.alignment = .centerY
        row.wantsLayer = true
        row.layer?.cornerRadius = 6
        row.edgeInsets = NSEdgeInsets(top: 2, left: 8, bottom: 2, right: 6)
        row.addArrangedSubview(title)
        row.addArrangedSubview(close)
        row.translatesAutoresizingMaskIntoConstraints = false
        // Equal share up to the cap; the title truncates when compressed.
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        title.setContentHuggingPriority(.defaultLow, for: .horizontal)
        row.widthAnchor.constraint(lessThanOrEqualToConstant: Layout.maxTabWidth).isActive = true
        close.widthAnchor.constraint(equalToConstant: 16).isActive = true
        tabStrip.addArrangedSubview(row)
        updateTabStripSelection()
    }

    private func updateTabButtonTitle(_ tab: Tab) {
        tab.button.title = tab.title
    }

    private func updateTabStripSelection() {
        for tab in tabs {
            let selected = tab.surfaceId == activeSurfaceId
            tab.row.layer?.backgroundColor = selected
                ? NSColor.labelColor.withAlphaComponent(0.10).cgColor
                : NSColor.clear.cgColor
            tab.button.contentTintColor = selected ? .labelColor : .secondaryLabelColor
        }
        tabStrip.isHidden = tabs.isEmpty
    }

    private enum AssociatedKeys { nonisolated(unsafe) static var surfaceId = 0 }

    // MARK: - Actions

    @objc private func backClicked() {
        guard let webView = activeTab()?.webView, webView.canGoBack else { return }
        webView.goBack()
    }

    @objc private func forwardClicked() {
        guard let webView = activeTab()?.webView, webView.canGoForward else { return }
        webView.goForward()
    }

    @objc private func refreshClicked() {
        activeTab()?.webView?.reload()
    }

    @objc private func closeAsideClicked() {
        onCloseAside()
    }

    @objc private func tabButtonClicked(_ sender: NSButton) {
        guard let id = objc_getAssociatedObject(sender, &AssociatedKeys.surfaceId) as? String else { return }
        activate(id)
    }

    @objc private func tabCloseClicked(_ sender: NSButton) {
        guard let id = objc_getAssociatedObject(sender, &AssociatedKeys.surfaceId) as? String else { return }
        onCloseTab(id)
    }

    // MARK: - Helpers

    /// Dedup key: compare normalized URLs (ignore trailing slash + fragment).
    static func sameURL(_ a: String, _ b: String) -> Bool {
        normalizeURL(a) == normalizeURL(b)
    }

    private static func normalizeURL(_ raw: String) -> String {
        var s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if let hash = s.firstIndex(of: "#") { s = String(s[..<hash]) }
        if s.hasSuffix("/") { s.removeLast() }
        return s.lowercased()
    }

    private static func shortTitle(for url: String) -> String {
        if let host = URL(string: url)?.host { return host }
        if let u = URL(string: url), u.isFileURL { return u.lastPathComponent }
        return url
    }
}
#endif
