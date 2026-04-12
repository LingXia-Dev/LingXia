#if os(macOS)
import AppKit
import WebKit
import os.log
import CLingXiaRustAPI

// MARK: - Host Protocol

@MainActor
protocol BrowserCoordinatorHost: AnyObject {
    /// Container view for browser content (workspaceManager.contentContainer).
    var browserContentContainer: NSView { get }
    /// The window.
    var hostWindow: NSWindow? { get }
    /// Returns owner (appId, sessionId) for creating a new browser tab.
    func browserOwnerForNewTab() -> (appId: String, sessionId: UInt64)?
    /// Called before a browser tab becomes active. Host should pause current VC.
    func browserWillActivateTab()
    /// Switch display to the lxapp tab with this appId.
    func switchToLxAppTab(_ appId: String)
    /// Currently active lxapp tab appId (if any).
    func activeAppTabId() -> String?
    /// Update sidebar browser items.
    func updateSidebarBrowserItems(_ items: [(id: UUID, title: String, favicon: NSImage?)], activeId: UUID?)
    /// Clear all sidebar highlights.
    func clearSidebarHighlights()
    /// Show/hide the lxapp navigation toolbar.
    func forceHideNavigationToolbar(_ hidden: Bool)
    /// Distance from leading edge needed to clear traffic light buttons.
    func trafficLightClearance() -> CGFloat
    /// Whether sidebar is collapsed.
    func isSidebarCollapsed() -> Bool
    /// Current lxapp WKWebView (for DevTools).
    func currentLxAppWebView() -> WKWebView?
}

// MARK: - BrowserTabCoordinator

@MainActor
final class BrowserTabCoordinator: NSObject {

    private static let log = OSLog(subsystem: "LingXia", category: "BrowserTabCoordinator")
    private static let attachMaxRetry = 5
    private static let devToolsMaxRetry = 30
    private static let devToolsRetryDelay: TimeInterval = 0.05
    private static let lxappDevToolsDetached = true
    private static let lxappDevToolsMaxRetry = 30
    private static let lxappDevToolsRetryDelay: TimeInterval = 0.05

    struct Layout {
        static let toolbarHeight: CGFloat = 38
        static let toolbarCenterY: CGFloat = 19
        static let buttonSize: CGFloat = 28
        static let toolbarIconSize: CGFloat = 14
        static let addressBarHeight: CGFloat = 26
        static let buttonLeading: CGFloat = 8
    }

    weak var host: BrowserCoordinatorHost?

    // Tab state
    private let settingsTabId = UUID().uuidString.lowercased()
    private let downloadsTabId = UUID().uuidString.lowercased()
    private(set) var activeTabId: UUID?
    private var tabIds: [UUID] = []
    private var tabTitles: [UUID: String] = [:]
    private var tabFavicons: [UUID: NSImage] = [:]
    private var tabFaviconRequestOrigins: [UUID: String] = [:]
    private var lastObservedURLs: [UUID: String] = [:]

    // UI
    private var browserView: NSView?
    private let toolbar = NSView()
    private let toolbarSeparator = NSView()
    private let backButton = NSButton()
    private let forwardButton = NSButton()
    private let refreshButton = NSButton()
    private let addressBarContainer = NSView()
    private let addressField = NSTextField()
    private let webContainer = NSView()
    private var activeWebView: WKWebView?
    private var backButtonLeadingConstraint: NSLayoutConstraint?
    private var toolbarCenterYConstraints: [NSLayoutConstraint] = []

    // KVO
    nonisolated(unsafe) private var titleObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var urlObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var canGoBackObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var canGoForwardObservation: NSKeyValueObservation?

    private var devToolsRequestToken: UInt64 = 0
    private var lxappDevToolsRequestToken: UInt64 = 0

    var isActive: Bool { activeTabId != nil }

    // MARK: - Lifecycle

    nonisolated func cleanup() {
        titleObservation?.invalidate()
        urlObservation?.invalidate()
        canGoBackObservation?.invalidate()
        canGoForwardObservation?.invalidate()
    }

    /// Deactivate browser UI (called when switching to an lxapp tab). Idempotent.
    func deactivate() {
        guard activeTabId != nil else { return }
        clearWebViewAttachment()
        hideBrowserView()
        activeTabId = nil
        host?.forceHideNavigationToolbar(false)
    }

    func syncToolbarCenterY(_ centerY: CGFloat) {
        toolbarCenterYConstraints.forEach { $0.constant = centerY }
    }

    // MARK: - Public Tab Operations

    func addTab() {
        addTabWithURL("")
    }

    func openSettings() {
        addTabWithURL("lingxia://settings", stableTabId: settingsTabId)
    }

    func openDownloads() {
        addTabWithURL("lingxia://downloads", stableTabId: downloadsTabId)
    }

    func selectTab(id: UUID) {
        switchToTab(id: id)
        host?.updateSidebarBrowserItems(sidebarItems(), activeId: id)
    }

    func closeTab(id: UUID) {
        guard let index = tabIds.firstIndex(of: id) else { return }

        // Detach WebView from UI BEFORE Rust destroy to prevent ObjC exceptions
        // during WebViewInner::Drop (removeFromSuperview/release on attached view).
        if activeTabId == id {
            clearWebViewAttachment()
        }

        tabTitles.removeValue(forKey: id)
        tabFavicons.removeValue(forKey: id)
        tabFaviconRequestOrigins.removeValue(forKey: id)
        lastObservedURLs.removeValue(forKey: id)
        tabIds.remove(at: index)

        // Destroy Rust state (triggers WebView Drop — safe now that UI is detached)
        _ = browserTabClose(tabIdString(id))

        if activeTabId == id {
            activeTabId = nil

            if let lastBrowser = tabIds.last {
                switchToTab(id: lastBrowser)
                host?.updateSidebarBrowserItems(sidebarItems(), activeId: lastBrowser)
            } else {
                hideBrowserView()
                host?.forceHideNavigationToolbar(false)
                if let appId = host?.activeAppTabId() {
                    host?.switchToLxAppTab(appId)
                }
            }
        }

        host?.updateSidebarBrowserItems(sidebarItems(), activeId: activeTabId)
    }

    func closeAllTabs(notifyRust: Bool = true) {
        if notifyRust {
            for id in tabIds {
                _ = browserTabClose(tabIdString(id))
            }
        }
        clearWebViewAttachment()
        tabIds.removeAll()
        tabTitles.removeAll()
        tabFavicons.removeAll()
        tabFaviconRequestOrigins.removeAll()
        lastObservedURLs.removeAll()
        activeTabId = nil
        hideBrowserView()
        host?.updateSidebarBrowserItems([], activeId: nil)
    }

    func presentInternalBrowserTab(id: UUID) {
        if !tabIds.contains(id) {
            tabIds.append(id)
        }
        switchToTab(id: id)
    }

    @MainActor
    func consumeSelfTargetNavigationInActiveBrowserTab(urlString: String) -> Bool {
        guard activeTabId != nil else { return false }
        guard let webView = activeWebView else { return false }
        let trimmed = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !displayableURL(trimmed).isEmpty else { return true }
        if webView.url?.absoluteString == trimmed { return true }
        return openAddressInActiveTab(trimmed)
    }

    // MARK: - DevTools

    func toggleActiveDevTools() -> Bool {
        if let activeId = activeTabId {
            devToolsRequestToken &+= 1
            let token = devToolsRequestToken
            return toggleBrowserDevToolsWhenReady(tabId: activeId, attempt: 0, token: token)
        }
        return toggleActiveLxAppDevTools()
    }

    private func toggleActiveLxAppDevTools() -> Bool {
        guard activeTabId == nil else { return false }
        guard let webView = host?.currentLxAppWebView() else { return false }
        lxappDevToolsRequestToken &+= 1
        let token = lxappDevToolsRequestToken
        return toggleLxAppDevToolsWhenReady(webView: webView, attempt: 0, token: token)
    }

    @discardableResult
    private func toggleLxAppDevToolsWhenReady(webView: WKWebView, attempt: Int, token: UInt64) -> Bool {
        guard token == lxappDevToolsRequestToken else { return false }
        guard activeTabId == nil else { return false }

        prepareLxAppWebViewForDevTools(webView, detached: Self.lxappDevToolsDetached)

        guard isWebViewDisplayReady(webView) else {
            return scheduleLxAppDevToolsRetry(webView: webView, attempt: attempt, token: token)
        }

        let ptr = swiftWebViewPointer(webView)
        return toggleWebViewDevtoolsByPtr(ptr, Self.lxappDevToolsDetached)
    }

    private func prepareLxAppWebViewForDevTools(_ webView: WKWebView, detached: Bool) {
        webView.isHidden = false
        if let container = webView.superview {
            let constraintsToDeactivate = container.constraints.filter { constraint in
                constraint.firstItem as AnyObject === webView || constraint.secondItem as AnyObject === webView
            }
            if !constraintsToDeactivate.isEmpty {
                NSLayoutConstraint.deactivate(constraintsToDeactivate)
            }
            webView.translatesAutoresizingMaskIntoConstraints = true
            webView.autoresizingMask = [.width, .height]
            webView.frame = container.bounds
            webView.setFrameSize(container.bounds.size)
            container.needsLayout = true
            container.layoutSubtreeIfNeeded()
        }
        webView.needsLayout = true
        webView.layoutSubtreeIfNeeded()
        if detached {
            clearInspectorAttachment(webView)
        } else {
            configureInspectorAttachment(webView)
        }
        host?.hostWindow?.contentView?.layoutSubtreeIfNeeded()
    }

    private func scheduleLxAppDevToolsRetry(webView: WKWebView, attempt: Int, token: UInt64) -> Bool {
        guard attempt < Self.lxappDevToolsMaxRetry else { return false }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.lxappDevToolsRetryDelay) { [weak self, weak webView] in
            guard let self, let webView else { return }
            _ = self.toggleLxAppDevToolsWhenReady(webView: webView, attempt: attempt + 1, token: token)
        }
        return true
    }

    @discardableResult
    private func toggleBrowserDevToolsWhenReady(tabId: UUID, attempt: Int, token: UInt64) -> Bool {
        guard token == devToolsRequestToken else { return false }
        guard activeTabId == tabId else { return false }
        guard let webView = findWebView(for: tabId) else {
            return scheduleBrowserDevToolsRetry(tabId: tabId, attempt: attempt, token: token, reason: "webview-missing")
        }

        prepareBrowserWebViewForDevTools(webView)
        guard isWebViewDisplayReady(webView) else {
            return scheduleBrowserDevToolsRetry(tabId: tabId, attempt: attempt, token: token, reason: "display-not-ready")
        }

        let ptr = swiftWebViewPointer(webView)
        let ok = toggleWebViewDevtoolsByPtr(ptr, false)
        if ok {
            scheduleBrowserDevToolsDetachedFallback(tabId: tabId, webView: webView, token: token)
        }
        return ok
    }

    private func scheduleBrowserDevToolsRetry(tabId: UUID, attempt: Int, token: UInt64, reason: String) -> Bool {
        guard attempt < Self.devToolsMaxRetry else {
            os_log(
                "toggleBrowserDevToolsWhenReady timed out after %d attempts for tab=%{public}@ reason=%{public}@",
                log: Self.log,
                type: .error,
                attempt,
                tabIdString(tabId),
                reason
            )
            return false
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.devToolsRetryDelay) { [weak self] in
            _ = self?.toggleBrowserDevToolsWhenReady(tabId: tabId, attempt: attempt + 1, token: token)
        }
        return true
    }

    private func prepareBrowserWebViewForDevTools(_ webView: WKWebView) {
        showBrowserView()
        if webView.superview !== webContainer {
            attachWebViewToContainer(webView)
            activeWebView = webView
            observeActiveWebView(webView)
        }
        webView.isHidden = false
        configureInspectorAttachment(webView)
        host?.hostWindow?.contentView?.layoutSubtreeIfNeeded()
    }

    private func scheduleBrowserDevToolsDetachedFallback(tabId: UUID, webView: WKWebView, token: UInt64) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self, weak webView] in
            guard let self, let webView else { return }
            guard token == self.devToolsRequestToken else { return }
            guard self.activeTabId == tabId else { return }
            guard self.isWebViewDisplayReady(webView) else { return }
            guard self.inspectorVisible(for: webView) == false else { return }
            _ = toggleWebViewDevtoolsByPtr(self.swiftWebViewPointer(webView), true)
        }
    }

    // MARK: - Inspector Helpers

    private func configureInspectorAttachment(_ webView: WKWebView) {
        let setSelector = NSSelectorFromString("_setInspectorAttachmentView:")
        guard webView.responds(to: setSelector) else { return }
        _ = webView.perform(setSelector, with: webView)
    }

    private func clearInspectorAttachment(_ webView: WKWebView) {
        let setSelector = NSSelectorFromString("_setInspectorAttachmentView:")
        guard webView.responds(to: setSelector) else { return }
        _ = webView.perform(setSelector, with: nil)
    }

    private func inspectorVisible(for webView: WKWebView) -> Bool? {
        let inspectorSelector = NSSelectorFromString("_inspector")
        guard webView.responds(to: inspectorSelector),
              let inspectorObject = webView.perform(inspectorSelector)?.takeUnretainedValue() else {
            return nil
        }
        let visibleSelector = NSSelectorFromString("isVisible")
        guard inspectorObject.responds(to: visibleSelector),
              let visibleObject = inspectorObject.perform(visibleSelector)?.takeUnretainedValue() else {
            return nil
        }
        if let number = visibleObject as? NSNumber {
            return number.boolValue
        }
        return nil
    }

    private func isWebViewDisplayReady(_ webView: WKWebView) -> Bool {
        guard webView.superview != nil else { return false }
        guard let window = webView.window, window.isVisible else { return false }
        guard window.screen != nil else { return false }
        if webView.isHidden || webView.isHiddenOrHasHiddenAncestor {
            return false
        }
        let bounds = webView.bounds.integral
        guard bounds.width > 1, bounds.height > 1 else { return false }
        return true
    }

    private func swiftWebViewPointer(_ webView: WKWebView) -> UInt {
        UInt(bitPattern: Unmanaged.passUnretained(webView).toOpaque())
    }

    // MARK: - WebView Management

    private func findWebView(for id: UUID) -> WKWebView? {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else {
            return nil
        }
        let path = browserTabPathForId(tabIdString(id)).toString()
        return WebViewManager.findWebView(
            appId: appId,
            path: path,
            sessionId: sessionId
        )
    }

    private func attachWebViewToContainer(_ webView: WKWebView) {
        let constraintsToDeactivate = webContainer.constraints.filter { constraint in
            constraint.firstItem as AnyObject === webView || constraint.secondItem as AnyObject === webView
        }
        if !constraintsToDeactivate.isEmpty {
            NSLayoutConstraint.deactivate(constraintsToDeactivate)
        }

        if webView.superview !== webContainer {
            webView.removeFromSuperview()
            webContainer.addSubview(webView)
        }

        // WebKit's attached inspector on macOS does not reliably handle Auto Layout-managed WKWebViews.
        webView.translatesAutoresizingMaskIntoConstraints = true
        webView.autoresizingMask = [.width, .height]
        webView.frame = webContainer.bounds
        webView.setFrameSize(webContainer.bounds.size)

        webContainer.needsLayout = true
        webContainer.layoutSubtreeIfNeeded()
        webView.needsLayout = true
        webView.layoutSubtreeIfNeeded()
        webView.resumeWebView()

        if let appId = webView.appId, let path = webView.currentPath {
            lingxia.onPageShow(appId, path)
        }
    }

    private func observeActiveWebView(_ webView: WKWebView) {
        titleObservation?.invalidate()
        urlObservation?.invalidate()
        canGoBackObservation?.invalidate()
        canGoForwardObservation?.invalidate()

        titleObservation = webView.observe(\.title, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, let activeId = self.activeTabId else { return }
                let title = (webView.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !title.isEmpty {
                    self.handleTitleChanged(id: activeId, title: title)
                }
                _ = updateBrowserTabInfo(self.tabIdString(activeId), webView.url?.absoluteString ?? "", webView.title ?? "")
            }
        }

        urlObservation = webView.observe(\.url, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, let activeId = self.activeTabId else { return }
                let rawURL = webView.url?.absoluteString ?? ""
                if self.lastObservedURLs[activeId] == rawURL {
                    return
                }
                self.lastObservedURLs[activeId] = rawURL

                if self.addressField.currentEditor() == nil {
                    self.addressField.stringValue = self.displayableURL(rawURL)
                }
                if let origin = webView.url.flatMap({ self.faviconRequestOrigin(for: $0) }) {
                    if origin != self.tabFaviconRequestOrigins[activeId] {
                        self.tabFavicons.removeValue(forKey: activeId)
                        self.tabFaviconRequestOrigins[activeId] = origin
                        self.host?.updateSidebarBrowserItems(self.sidebarItems(), activeId: activeId)
                    }
                    if self.tabFavicons[activeId] == nil {
                        self.fetchFavicon(for: origin, tabId: activeId)
                    }
                }
                _ = updateBrowserTabInfo(self.tabIdString(activeId), webView.url?.absoluteString ?? "", webView.title ?? "")
            }
        }

        canGoBackObservation = webView.observe(\.canGoBack, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                self?.updateBackButtonState(canGoBack: webView.canGoBack)
            }
        }

        canGoForwardObservation = webView.observe(\.canGoForward, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                self?.updateForwardButtonState(canGoForward: webView.canGoForward)
            }
        }
    }

    private func clearWebViewAttachment() {
        titleObservation?.invalidate()
        urlObservation?.invalidate()
        canGoBackObservation?.invalidate()
        canGoForwardObservation?.invalidate()
        titleObservation = nil
        urlObservation = nil
        canGoBackObservation = nil
        canGoForwardObservation = nil
        if let activeWebView {
            clearInspectorAttachment(activeWebView)
        }
        activeWebView?.removeFromSuperview()
        activeWebView = nil
        updateBackButtonState(canGoBack: false)
        updateForwardButtonState(canGoForward: false)
    }

    // MARK: - Internal Tab Operations

    private func addTabWithURL(_ url: String, stableTabId: String? = nil) {
        guard let owner = host?.browserOwnerForNewTab() else {
            os_log("Cannot create browser tab without active lxapp session", log: Self.log, type: .error)
            return
        }

        let openedTab = if let stableTabId {
            openBrowserTabWithId(owner.appId, owner.sessionId, url, stableTabId)
        } else {
            openBrowserTab(owner.appId, owner.sessionId, url)
        }

        guard let openedTab else {
            os_log(
                "openBrowserTab failed for %{public}@/%{public}llu url=%{public}@ stableTabId=%{public}@",
                log: Self.log,
                type: .error,
                owner.appId,
                owner.sessionId,
                url,
                stableTabId ?? ""
            )
            return
        }

        let tabId = openedTab.toString().lowercased()
        guard let id = UUID(uuidString: tabId) else {
            os_log(
                "openBrowserTab returned invalid tab id=%{public}@ for %{public}@/%{public}llu",
                log: Self.log,
                type: .error,
                tabId,
                owner.appId,
                owner.sessionId,
            )
            return
        }

        presentInternalBrowserTab(id: id)
    }

    private func switchToTab(id: UUID) {
        guard tabIds.contains(id) else { return }
        if activeTabId == id {
            host?.updateSidebarBrowserItems(sidebarItems(), activeId: id)
            return
        }

        host?.browserWillActivateTab()
        clearWebViewAttachment()

        activeTabId = id

        host?.clearSidebarHighlights()
        host?.updateSidebarBrowserItems(sidebarItems(), activeId: id)
        host?.forceHideNavigationToolbar(true)

        showBrowserView()
        addressField.stringValue = ""
        updateBackButtonState(canGoBack: false)

        attachWebView(for: id, attempt: 0)
    }

    private func attachWebView(for tabId: UUID, attempt: Int) {
        guard activeTabId == tabId else { return }

        if let webView = findWebView(for: tabId) {
            if #available(macOS 13.3, *) {
                webView.isInspectable = true
            }
            webView.configuration.preferences.setValue(true, forKey: "developerExtrasEnabled")
            showBrowserView()
            attachWebViewToContainer(webView)
            configureInspectorAttachment(webView)
            activeWebView = webView
            observeActiveWebView(webView)
            addressField.stringValue = displayableURL(webView.url?.absoluteString)
            updateBackButtonState(canGoBack: webView.canGoBack)
            return
        }

        guard attempt < Self.attachMaxRetry else {
            os_log("Failed to attach browser webview after %d retries for tab=%{public}@",
                   log: Self.log, type: .error, attempt, tabIdString(tabId))
            if activeTabId == tabId {
                clearWebViewAttachment()
                hideBrowserView()
                activeTabId = nil
                host?.forceHideNavigationToolbar(false)
                host?.updateSidebarBrowserItems(sidebarItems(), activeId: nil)
                if let appId = host?.activeAppTabId() {
                    host?.switchToLxAppTab(appId)
                }
            }
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            self?.attachWebView(for: tabId, attempt: attempt + 1)
        }
    }

    // MARK: - Navigation Actions

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

    @objc private func addressSubmitted(_ sender: NSTextField) {
        guard let result = handleBrowserAddressSubmission(
            rawInput: sender.stringValue,
            currentURL: activeWebView?.url?.absoluteString,
            tabId: activeTabId?.uuidString
        ) else { return }
        _ = openAddressInActiveTab(result.url)
    }

    private func openAddressInActiveTab(_ urlString: String) -> Bool {
        guard let webView = activeWebView,
              let url = URL(string: urlString) else { return false }
        addressField.stringValue = urlString
        webView.load(URLRequest(url: url))
        return true
    }

    // MARK: - Browser View Setup

    private func setupBrowserViewIfNeeded() {
        guard browserView == nil else { return }

        let bv = NSView()
        bv.translatesAutoresizingMaskIntoConstraints = false
        bv.wantsLayer = true

        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.wantsLayer = true
        toolbar.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        bv.addSubview(toolbar)

        configureButton(backButton, iconName: "icon_back", action: #selector(backClicked))
        toolbar.addSubview(backButton)

        configureButton(forwardButton, iconName: "icon_forward", action: #selector(forwardClicked))
        forwardButton.isEnabled = false
        forwardButton.alphaValue = 0.4
        toolbar.addSubview(forwardButton)

        configureButton(refreshButton, iconName: "icon_browser_refresh", action: #selector(refreshClicked))
        toolbar.addSubview(refreshButton)

        addressBarContainer.translatesAutoresizingMaskIntoConstraints = false
        addressBarContainer.wantsLayer = true
        addressBarContainer.layer?.cornerRadius = 6
        addressBarContainer.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        toolbar.addSubview(addressBarContainer)

        addressField.translatesAutoresizingMaskIntoConstraints = false
        addressField.font = NSFont.systemFont(ofSize: 13)
        addressField.placeholderString = "Enter URL"
        addressField.isBordered = false
        addressField.drawsBackground = false
        addressField.focusRingType = .none
        addressField.usesSingleLineMode = true
        addressField.cell?.wraps = false
        addressField.cell?.isScrollable = true
        addressField.cell?.lineBreakMode = .byTruncatingTail
        addressField.target = self
        addressField.action = #selector(addressSubmitted(_:))
        addressBarContainer.addSubview(addressField)

        toolbarSeparator.translatesAutoresizingMaskIntoConstraints = false
        toolbarSeparator.wantsLayer = true
        toolbarSeparator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        bv.addSubview(toolbarSeparator)

        webContainer.translatesAutoresizingMaskIntoConstraints = false
        webContainer.wantsLayer = true
        bv.addSubview(webContainer)

        let backCenterY = backButton.centerYAnchor.constraint(equalTo: toolbar.topAnchor, constant: Layout.toolbarCenterY)
        let forwardCenterY = forwardButton.centerYAnchor.constraint(equalTo: toolbar.topAnchor, constant: Layout.toolbarCenterY)
        let refreshCenterY = refreshButton.centerYAnchor.constraint(equalTo: toolbar.topAnchor, constant: Layout.toolbarCenterY)
        let addressCenterY = addressBarContainer.centerYAnchor.constraint(equalTo: toolbar.topAnchor, constant: Layout.toolbarCenterY)
        toolbarCenterYConstraints = [backCenterY, forwardCenterY, refreshCenterY, addressCenterY]

        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: bv.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: bv.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: bv.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: Layout.toolbarHeight),

            {
                let c = backButton.leadingAnchor.constraint(equalTo: toolbar.leadingAnchor, constant: currentButtonLeading())
                backButtonLeadingConstraint = c
                return c
            }(),
            backCenterY,
            backButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            backButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            forwardButton.leadingAnchor.constraint(equalTo: backButton.trailingAnchor, constant: 4),
            forwardCenterY,
            forwardButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            forwardButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            refreshButton.leadingAnchor.constraint(equalTo: forwardButton.trailingAnchor, constant: 4),
            refreshCenterY,
            refreshButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            refreshButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            addressBarContainer.leadingAnchor.constraint(equalTo: refreshButton.trailingAnchor, constant: 8),
            addressBarContainer.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor, constant: -8),
            addressCenterY,
            addressBarContainer.heightAnchor.constraint(equalToConstant: Layout.addressBarHeight),

            addressField.leadingAnchor.constraint(equalTo: addressBarContainer.leadingAnchor, constant: 8),
            addressField.trailingAnchor.constraint(equalTo: addressBarContainer.trailingAnchor, constant: -8),
            addressField.centerYAnchor.constraint(equalTo: addressBarContainer.centerYAnchor),

            toolbarSeparator.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            toolbarSeparator.leadingAnchor.constraint(equalTo: bv.leadingAnchor),
            toolbarSeparator.trailingAnchor.constraint(equalTo: bv.trailingAnchor),
            toolbarSeparator.heightAnchor.constraint(equalToConstant: 1),

            webContainer.topAnchor.constraint(equalTo: toolbarSeparator.bottomAnchor),
            webContainer.leadingAnchor.constraint(equalTo: bv.leadingAnchor),
            webContainer.trailingAnchor.constraint(equalTo: bv.trailingAnchor),
            webContainer.bottomAnchor.constraint(equalTo: bv.bottomAnchor),
        ])

        browserView = bv
        updateBackButtonState(canGoBack: false)
    }

    private func showBrowserView() {
        setupBrowserViewIfNeeded()
        guard let bv = browserView, let container = host?.browserContentContainer else { return }

        if bv.superview !== container {
            container.addSubview(bv)
            NSLayoutConstraint.activate([
                bv.topAnchor.constraint(equalTo: container.topAnchor),
                bv.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                bv.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                bv.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            ])
        }

        host?.hostWindow?.contentView?.layoutSubtreeIfNeeded()
    }

    private func hideBrowserView() {
        browserView?.removeFromSuperview()
    }

    // MARK: - UI Helpers

    private func updateBackButtonState(canGoBack: Bool) {
        backButton.isEnabled = canGoBack
        backButton.alphaValue = canGoBack ? 1.0 : 0.4
    }

    private func updateForwardButtonState(canGoForward: Bool) {
        forwardButton.isEnabled = canGoForward
        forwardButton.alphaValue = canGoForward ? 1.0 : 0.4
    }

    private func configureButton(_ button: NSButton, iconName: String, action: Selector) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.imagePosition = .imageOnly
        button.imageScaling = .scaleProportionallyDown
        button.target = self
        button.action = action

        button.image = loadToolbarIcon(named: iconName, size: Layout.toolbarIconSize)
        button.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
    }

    private func loadToolbarIcon(named iconName: String, size: CGFloat) -> NSImage? {
        return LxIcon.image(named: iconName, size: CGSize(width: size, height: size))
    }

    private func currentButtonLeading() -> CGFloat {
        let collapsed = host?.isSidebarCollapsed() ?? false
        return collapsed ? (host?.trafficLightClearance() ?? 80) : Layout.buttonLeading
    }

    // MARK: - Data Helpers

    private func tabIdString(_ id: UUID) -> String {
        id.uuidString.lowercased()
    }

    private func displayableURL(_ raw: String?) -> String {
        guard let raw else { return "" }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        return browserUrlIsHidden(trimmed) ? "" : trimmed
    }

    private func sidebarItems() -> [(id: UUID, title: String, favicon: NSImage?)] {
        tabIds.map { id in
            (id, tabTitles[id] ?? "New Tab", tabFavicons[id])
        }
    }

    private func handleTitleChanged(id: UUID, title: String) {
        guard tabIds.contains(id) else { return }
        if tabTitles[id] == title {
            return
        }
        tabTitles[id] = title
        host?.updateSidebarBrowserItems(sidebarItems(), activeId: activeTabId)
    }

    private func faviconRequestOrigin(for url: URL) -> String? {
        guard let scheme = url.scheme?.lowercased(),
              let host = url.host?.lowercased() else {
            return nil
        }
        if scheme == "lingxia" {
            return "\(scheme)://\(host)"
        }
        guard scheme == "http" || scheme == "https" else {
            return nil
        }
        let port: String
        if let rawPort = url.port, !((scheme == "http" && rawPort == 80) || (scheme == "https" && rawPort == 443)) {
            port = ":\(rawPort)"
        } else {
            port = ""
        }
        return "\(scheme)://\(host)\(port)"
    }

    private func bundledFavicon() -> NSImage? {
        #if SWIFT_PACKAGE
        let bundle = Bundle.module
        #else
        let bundle = Bundle(for: BrowserTabCoordinator.self)
        #endif
        guard let faviconURL = bundle.url(forResource: "favicon", withExtension: "ico") else {
            return nil
        }
        return NSImage(contentsOf: faviconURL)
    }

    private func fetchFavicon(for origin: String, tabId: UUID) {
        if origin.hasPrefix("lingxia://") {
            guard let image = bundledFavicon() else { return }
            tabFavicons[tabId] = image
            host?.updateSidebarBrowserItems(sidebarItems(), activeId: activeTabId)
            return
        }

        guard let faviconURL = URL(string: "\(origin)/favicon.ico") else { return }
        URLSession.shared.dataTask(with: faviconURL) { [weak self] data, response, _ in
            guard let data,
                  let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200,
                  let contentType = httpResponse.value(forHTTPHeaderField: "Content-Type"),
                  !contentType.hasPrefix("text/"),
                  let image = NSImage(data: data), image.isValid else { return }
            Task { @MainActor in
                guard let self,
                      self.tabIds.contains(tabId),
                      self.tabFaviconRequestOrigins[tabId] == origin else { return }
                self.tabFavicons[tabId] = image
                self.host?.updateSidebarBrowserItems(self.sidebarItems(), activeId: self.activeTabId)
            }
        }.resume()
    }
}

#endif
