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
    /// Whether the host has an lxapp tab to reveal after browser tabs close.
    var hasOpenTabs: Bool { get }
    /// Whether browser chrome remains usable when the last web tab closes.
    var keepsBrowserRootWithoutTabs: Bool { get }
    /// Returns owner (appId, sessionId) for creating a new browser tab.
    func browserOwnerForNewTab() -> (appId: String, sessionId: UInt64)?
    /// Called before a browser tab becomes active. Host should pause current VC.
    func browserWillActivateTab()
    /// Switch display to the lxapp tab with this appId.
    func switchToLxAppTab(_ appId: String)
    /// Currently active lxapp tab appId (if any).
    func activeAppTabId() -> String?
    /// Update sidebar browser items.
    func updateSidebarBrowserItems(_ items: [(id: String, title: String, url: String, favicon: NSImage?)], activeId: String?)
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
    private let settingsTabId = "settings"
    private let downloadsTabId = "downloads"
    private let bookmarksTabId = "bookmarks"
    private let historyTabId = "history"
    private(set) var activeTabId: String?
    private var tabIds: [String] = []
    private var tabTitles: [String: String] = [:]
    private var tabFavicons: [String: NSImage] = [:]
    private var tabFaviconRequestOrigins: [String: String] = [:]
    private var lastObservedURLs: [String: String] = [:]
    private var retainedNewTabOwner: (appId: String, sessionId: UInt64)?

    /// Tabs whose WebView has been discarded to free memory (Chrome-style).
    /// Their sidebar entry stays; the WebView is recreated on reactivation.
    private var discardedTabs: Set<String> = []
    /// Activation order, least-recently-used first — drives discard ordering.
    private var tabRecency: [String] = []
    /// When each background tab last went inactive — its idle clock.
    private var backgroundedAt: [String: Date] = [:]

    /// Reactive: free background tabs when the system reports memory pressure.
    nonisolated(unsafe) private var memoryPressureSource: DispatchSourceMemoryPressure?
    /// Proactive: periodically free tabs idle longer than `idleDiscardThreshold`,
    /// so memory stays low without waiting for pressure.
    nonisolated(unsafe) private var idleSweepTimer: Timer?
    private var memoryManagementStarted = false
    /// Background-idle time after which a tab is discarded proactively.
    private static let idleDiscardThreshold: TimeInterval = 30 * 60
    /// How often the idle sweep runs.
    private static let idleSweepInterval: TimeInterval = 60
    /// Under a `.warning` pressure event, only discard tabs idle at least this
    /// long (avoid nuking a tab you just switched away from). `.critical`
    /// discards all background tabs.
    private static let warningPressureMinIdle: TimeInterval = 60

    // UI
    private var browserView: NSView?
    private let toolbar = NSView()
    private let toolbarSeparator = NSView()
    private let backButton = NSButton()
    private let forwardButton = NSButton()
    private let refreshButton = NSButton()
    private let addressBarContainer = NSView()
    private let addressField = NSTextField()
    /// Save the current website to the bookmarks archive.
    private let starButton = NSButton()
    /// Promote the current website to a persistent sidebar shortcut.
    private let pinButton = NSButton()
    /// Open the bookmarks manager without adding sidebar chrome.
    private let bookmarksButton = NSButton()
    /// Per-tab page menu (bookmark, copy link, bookmarks page).
    private let menuButton = NSButton()
    private var pageActionsVisible = true
    private let webContainer = NSView()
    nonisolated(unsafe) private var shortcutMonitor: Any?
    private var activeWebView: WKWebView?
    private var backButtonLeadingConstraint: NSLayoutConstraint?
    private var toolbarCenterYConstraints: [NSLayoutConstraint] = []
    /// Tabs the user has interacted with (page click or address navigation).
    private var interactedTabs: Set<String> = []
    nonisolated(unsafe) private var interactionMonitor: Any?

    // KVO
    nonisolated(unsafe) private var titleObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var urlObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var canGoBackObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var canGoForwardObservation: NSKeyValueObservation?
    /// Per-tab title observations kept for *all* tabs, not just the active one.
    /// Only the active webview is attached, so without these a background tab
    /// whose title lands after it's switched away would keep the "New Tab"
    /// placeholder in the sidebar until it's clicked.
    nonisolated(unsafe) private var tabTitleObservations: [String: NSKeyValueObservation] = [:]

    private var devToolsRequestToken: UInt64 = 0
    private var lxappDevToolsRequestToken: UInt64 = 0

    var isActive: Bool { activeTabId != nil }

    // MARK: - Lifecycle

    nonisolated func cleanup() {
        // May run from a background-thread deinit. Capture the values, nil the
        // fields, then hop to main for the AppKit monitor removal and the
        // main-scheduled Timer invalidation — never capturing self.
        nonisolated(unsafe) let monitors = [shortcutMonitor, interactionMonitor].compactMap { $0 }
        shortcutMonitor = nil
        interactionMonitor = nil
        nonisolated(unsafe) let timer = idleSweepTimer
        idleSweepTimer = nil
        memoryPressureSource?.cancel()
        memoryPressureSource = nil
        titleObservation?.invalidate()
        urlObservation?.invalidate()
        canGoBackObservation?.invalidate()
        canGoForwardObservation?.invalidate()
        tabTitleObservations.values.forEach { $0.invalidate() }
        tabTitleObservations.removeAll()

        let finish = { @Sendable in
            monitors.forEach { NSEvent.removeMonitor($0) }
            timer?.invalidate()
        }
        if Thread.isMainThread {
            finish()
        } else {
            DispatchQueue.main.async(execute: finish)
        }
    }

    /// Observe a tab's title for as long as the tab exists (independent of which
    /// tab is currently attached), so the sidebar label follows background tabs.
    /// The tab's webview may not exist yet right after open, so retry briefly.
    private func ensureTitleObservation(for id: String, attempt: Int = 0) {
        guard tabIds.contains(id), tabTitleObservations[id] == nil else { return }
        guard let webView = findWebView(for: id) else {
            if attempt < Self.attachMaxRetry {
                DispatchQueue.main.asyncAfter(deadline: .now() + Self.devToolsRetryDelay) { [weak self] in
                    self?.ensureTitleObservation(for: id, attempt: attempt + 1)
                }
            }
            return
        }
        tabTitleObservations[id] = webView.observe(\.title, options: [.initial, .new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, self.tabIds.contains(id) else { return }
                let title = (webView.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                guard !title.isEmpty else { return }
                self.handleTitleChanged(id: id, title: title)
            }
        }
    }

    /// Deactivate browser UI (called when switching to an lxapp tab). Idempotent.
    func deactivate() {
        guard let previous = activeTabId else { return }
        clearWebViewAttachment()
        hideBrowserView()
        // The browser is leaving the foreground — start the idle clock for the
        // tab that was active, so it isn't treated as infinitely idle and
        // discarded prematurely on the next pressure event / sweep.
        backgroundedAt[previous] = Date()
        // No browser tab is visible while an lxapp owns the foreground. Clear
        // core active state so critical pressure can reclaim this last tab too.
        browserTabDeactivate()
        activeTabId = nil
        host?.forceHideNavigationToolbar(false)
    }

    func syncToolbarCenterY(_ centerY: CGFloat) {
        toolbarCenterYConstraints.forEach { $0.constant = centerY }
    }

    func syncToolbarLeading(collapsed: Bool, animated: Bool) {
        let targetLeading = Layout.buttonLeading
        if animated {
            backButtonLeadingConstraint?.animator().constant = targetLeading
        } else {
            backButtonLeadingConstraint?.constant = targetLeading
        }
    }

    // MARK: - Public Tab Operations

    func addTab() {
        // A persistent browser root is used by the URL Runner, which does not
        // bundle the browser webui behind `lingxia://newtab`.
        addTabWithURL(host?.keepsBrowserRootWithoutTabs == true ? "about:blank" : "")
    }

    func openSettings() {
        addTabWithURL("lingxia://settings", stableTabId: settingsTabId)
    }

    func openDownloads() {
        addTabWithURL("lingxia://downloads", stableTabId: downloadsTabId)
    }

    /// Open the bookmarks manager page (the archive; pins are its subset).
    func openBookmarks() {
        addTabWithURL("lingxia://bookmarks", stableTabId: bookmarksTabId)
    }

    func openHistory() {
        addTabWithURL("lingxia://history", stableTabId: historyTabId)
    }

    func openClearSiteData(tabId: String) {
        addTabWithURL(
            "lingxia://settings#clear-site-data?tabId=\(tabIdString(tabId))",
            stableTabId: settingsTabId
        )
    }

    /// Open a bookmark: focus an existing tab with the same URL (Arc pinned
    /// semantics — one entity, click activates) or open a fresh tab.
    func openBookmark(url: String) {
        let target = normalizedBookmarkURL(url)
        if let existing = tabIds.first(where: {
            normalizedBookmarkURL(lastObservedURLs[$0] ?? "") == target
        }) {
            selectTab(id: existing)
            return
        }
        addTabWithURL(url)
    }

    private func normalizedBookmarkURL(_ raw: String) -> String {
        SidebarBookmarksSnapshot.normalize(raw)
    }

    func selectTab(id: String) {
        switchToTab(id: id)
        host?.updateSidebarBrowserItems(sidebarItems(), activeId: id)
    }

    func closeTab(id: String) {
        guard let index = tabIds.firstIndex(of: id) else { return }

        // A browser-only host has no lxapp surface to reveal. Keep its final
        // tab as the mounted browser content instead of leaving an empty shell.
        if tabIds.count == 1,
           host?.hasOpenTabs == false,
           host?.keepsBrowserRootWithoutTabs != true {
            _ = browserTabNavigate(tabIdString(id), "about:blank")
            interactedTabs.remove(id)
            lastObservedURLs[id] = "about:blank"
            switchToTab(id: id)
            return
        }

        // Detach WebView from UI BEFORE Rust destroy to prevent ObjC exceptions
        // during WebViewInner::Drop (removeFromSuperview/release on attached view).
        if activeTabId == id {
            clearWebViewAttachment()
        }

        tabTitleObservations.removeValue(forKey: id)?.invalidate()
        tabTitles.removeValue(forKey: id)
        tabFavicons.removeValue(forKey: id)
        tabFaviconRequestOrigins.removeValue(forKey: id)
        lastObservedURLs.removeValue(forKey: id)
        interactedTabs.remove(id)
        discardedTabs.remove(id)
        tabRecency.removeAll { $0 == id }
        backgroundedAt.removeValue(forKey: id)
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

    func closeOtherTabs(keeping id: String) {
        guard tabIds.contains(id) else { return }
        for tabId in tabIds.filter({ $0 != id }) {
            closeTab(id: tabId)
        }
    }

    func closeTabsBelow(id: String) {
        guard let index = tabIds.firstIndex(of: id),
              index < tabIds.index(before: tabIds.endIndex) else { return }
        for tabId in Array(tabIds[tabIds.index(after: index)...]) {
            closeTab(id: tabId)
        }
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
        discardedTabs.removeAll()
        tabRecency.removeAll()
        backgroundedAt.removeAll()
        activeTabId = nil
        hideBrowserView()
        host?.updateSidebarBrowserItems([], activeId: nil)
    }

    func presentInternalBrowserTab(id: String) {
        if let owner = host?.browserOwnerForNewTab() {
            retainedNewTabOwner = owner
        }
        if !tabIds.contains(id) {
            tabIds.append(id)
        }
        ensureTitleObservation(for: id)
        switchToTab(id: id)
    }

    /// The single multi-tab browser aside panel for this window (nil when none is
    /// open). Every web-aside surface node is a tab in here — the panel is NOT
    /// entered into `tabIds` / the sidebar and never becomes a switchable main
    /// tab. Owned here so a second `openSurface({url,as:'aside'})` adds a tab
    /// rather than docking a second browser.
    private var dockedBrowser: DockedBrowser?

    /// Open `url` as a tab in the browser aside, creating the panel on first use.
    /// Returns the panel plus `isNew` (true only when it was just created, so the
    /// caller docks `panel.containerView`). `onCloseTab(surfaceId)` fires when a
    /// tab's X is clicked; `onCloseAside` when the whole aside is dismissed.
    /// Returns nil if there is no active session or the tab could not be created.
    func openDockedAsideTab(
        surfaceId: String,
        url: String,
        ephemeralWebData: Bool,
        urlCallback: Bool,
        onCloseTab: @escaping (String) -> Void,
        onCloseAside: @escaping () -> Void
    ) -> (browser: DockedBrowser, isNew: Bool)? {
        if let existing = dockedBrowser {
            if existing.addOrFocusTab(
                surfaceId: surfaceId,
                url: url,
                ephemeralWebData: ephemeralWebData,
                urlCallback: urlCallback) {
                return (existing, false)
            }
            // Stale panel reference (e.g. torn down without clearing): drop it
            // and rebuild below instead of failing every future aside.
            LXLog.error("docked browser reference was stale; rebuilding", category: "BrowserTabCoordinator")
            existing.tearDown()
            dockedBrowser = nil
        }
        guard let owner = host?.browserOwnerForNewTab() else {
            LXLog.error("Cannot create docked browser without active lxapp session", category: "BrowserTabCoordinator")
            return nil
        }
        guard let browser = DockedBrowser(
            owner: owner,
            surfaceId: surfaceId,
            url: url,
            ephemeralWebData: ephemeralWebData,
            urlCallback: urlCallback,
            onCloseTab: onCloseTab,
            onCloseAside: onCloseAside
        ) else { return nil }
        dockedBrowser = browser
        return (browser, true)
    }

    /// The live browser aside panel, if any (for tab removal / re-anchoring).
    var activeDockedBrowser: DockedBrowser? { dockedBrowser }

    /// Drop the panel reference after its last tab closed and it was torn down.
    func clearDockedBrowser() {
        dockedBrowser = nil
    }

    private func attachedWebViewForNativeInput(tabId: String) -> WKWebView? {
        let id = tabIdString(tabId)
        guard !id.isEmpty else { return nil }
        if activeTabId != id {
            presentInternalBrowserTab(id: id)
        }

        guard let webView = findWebView(for: id), let window = webView.window else {
            return nil
        }
        if webView.superview == nil || webView.superview !== webContainer {
            attachWebViewToContainer(webView)
            activeWebView = webView
            observeActiveWebView(webView)
        }
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(webView)
        return webView
    }

    func prepareNativeInput(tabId: String) -> Bool {
        // A docked aside browser owns its webview in the aside panel. Focus it in
        // place rather than relocating the tab into the main browser area (which
        // would empty the aside).
        if let docked = dockedBrowser, docked.containsBrowserTab(tabIdString(tabId)) {
            return docked.focusForInput(browserTabId: tabIdString(tabId))
        }
        return attachedWebViewForNativeInput(tabId: tabId) != nil
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
    private func toggleBrowserDevToolsWhenReady(tabId: String, attempt: Int, token: UInt64) -> Bool {
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

    private func scheduleBrowserDevToolsRetry(tabId: String, attempt: Int, token: UInt64, reason: String) -> Bool {
        guard attempt < Self.devToolsMaxRetry else {
            LXLog.error(
                "toggleBrowserDevToolsWhenReady timed out after \(attempt) attempts for tab=\(tabIdString(tabId)) reason=\(reason)",
                category: "BrowserTabCoordinator"
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

    private func scheduleBrowserDevToolsDetachedFallback(tabId: String, webView: WKWebView, token: UInt64) {
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

    private func findWebView(for id: String) -> WKWebView? {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else {
            return nil
        }
        let path = browserTabPathForId(tabIdString(id)).toString()
        return WebViewManager.resolveWebView(
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

        // .initial: by attach time the page often already has a title/URL —
        // a change-only observation would leave the sidebar stuck on "New Tab".
        titleObservation = webView.observe(\.title, options: [.initial, .new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, let activeId = self.activeTabId else { return }
                let title = (webView.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !title.isEmpty {
                    self.handleTitleChanged(id: activeId, title: title)
                }
            }
        }

        urlObservation = webView.observe(\.url, options: [.initial, .new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, let activeId = self.activeTabId else { return }
                let rawURL = webView.url?.absoluteString ?? ""
                let previousURL = self.lastObservedURLs[activeId]
                self.syncAddressField(rawURL)
                self.updatePageSaveButtons(for: rawURL)
                self.lastObservedURLs[activeId] = rawURL
                guard previousURL != rawURL else { return }
                if let origin = webView.url.flatMap({ self.faviconRequestOrigin(for: $0) }) {
                    if origin != self.tabFaviconRequestOrigins[activeId] {
                        self.tabFavicons.removeValue(forKey: activeId)
                        self.tabFaviconRequestOrigins[activeId] = origin
                    }
                    if self.tabFavicons[activeId] == nil {
                        self.fetchFavicon(for: origin, tabId: activeId, webView: webView)
                    }
                }
                self.host?.updateSidebarBrowserItems(self.sidebarItems(), activeId: activeId)
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
        let owner = host?.browserOwnerForNewTab()
            ?? (host?.keepsBrowserRootWithoutTabs == true ? retainedNewTabOwner : nil)
        guard let owner else {
            LXLog.error("Cannot create browser tab without active lxapp session", category: "BrowserTabCoordinator")
            return
        }
        retainedNewTabOwner = owner

        let normalizedStableTabId = stableTabId?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let requestedStableTabId = normalizedStableTabId?.isEmpty == false ? normalizedStableTabId : nil

        let openedTab = if let requestedStableTabId {
            openBrowserTabWithId(owner.appId, owner.sessionId, url, requestedStableTabId)
        } else {
            openBrowserTab(owner.appId, owner.sessionId, url)
        }

        guard let openedTab else {
            LXLog.error(
                "openBrowserTab failed for \(owner.appId)/\(owner.sessionId) url=\(url) stableTabId=\(requestedStableTabId ?? "")",
                category: "BrowserTabCoordinator"
            )
            return
        }

        let tabId = tabIdString(openedTab.toString())
        guard !tabId.isEmpty else {
            LXLog.error("openBrowserTab returned empty tab id", category: "BrowserTabCoordinator")
            return
        }

        let requestedURL = url.trimmingCharacters(in: .whitespacesAndNewlines)
        if !requestedURL.isEmpty {
            lastObservedURLs[tabId] = requestedURL
        }
        presentInternalBrowserTab(id: tabId)
    }

    private func switchToTab(id: String) {
        guard tabIds.contains(id) else { return }
        if activeTabId == id {
            host?.updateSidebarBrowserItems(sidebarItems(), activeId: id)
            return
        }

        startMemoryManagementIfNeeded()

        // If the tab was discarded to save memory, recreate its WebView and
        // reload the saved URL before attaching (reactivate also marks it
        // active in Rust); otherwise sync the Rust-side active tab so a
        // previously-active live tab can be discarded once it's in the
        // background.
        if discardedTabs.contains(id) {
            guard browserTabReactivate(tabIdString(id)) else {
                // Keep the previous tab attached and active so another click
                // can retry recreation immediately instead of hitting the
                // active-tab early return above with no WebView to present.
                LXLog.error(
                    "Failed to reactivate discarded browser tab \(id)",
                    category: "BrowserTabCoordinator"
                )
                return
            }
            discardedTabs.remove(id)
        } else {
            browserTabActivate(tabIdString(id))
        }

        host?.browserWillActivateTab()
        clearWebViewAttachment()

        // The tab we're leaving starts its background idle clock now.
        if let previous = activeTabId {
            backgroundedAt[previous] = Date()
        }

        activeTabId = id
        backgroundedAt.removeValue(forKey: id)
        touchRecency(id)

        host?.clearSidebarHighlights()
        host?.updateSidebarBrowserItems(sidebarItems(), activeId: id)
        host?.forceHideNavigationToolbar(true)

        showBrowserView()
        addressField.stringValue = ""
        updatePageSaveButtons(for: nil)
        // An aside tab hides the address bar (self chrome otherwise unchanged).
        addressBarContainer.isHidden = browserTabIsAside(tabIdString(id))
        updateBackButtonState(canGoBack: false)

        attachWebView(for: id, attempt: 0)
    }

    /// Move `id` to the most-recently-used position.
    private func touchRecency(_ id: String) {
        tabRecency.removeAll { $0 == id }
        tabRecency.append(id)
    }

    // MARK: - Adaptive memory management (no fixed tab cap)

    private func startMemoryManagementIfNeeded() {
        guard !memoryManagementStarted else { return }
        memoryManagementStarted = true

        // Reactive: discard background tabs when the system is under memory
        // pressure. `.critical` frees everything in the background; `.warning`
        // frees only tabs that have been idle for a bit.
        let source = DispatchSource.makeMemoryPressureSource(
            eventMask: [.warning, .critical], queue: .main)
        source.setEventHandler { [weak self] in
            guard let self else { return }
            let minIdle: TimeInterval =
                source.data.contains(.critical) ? 0 : Self.warningPressureMinIdle
            self.discardBackgroundTabs(minIdle: minIdle)
        }
        source.resume()
        memoryPressureSource = source

        // Proactive: sweep idle tabs so memory stays low before pressure hits.
        let idleDiscardThreshold = Self.idleDiscardThreshold
        let timer = Timer.scheduledTimer(
            withTimeInterval: Self.idleSweepInterval, repeats: true
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.discardBackgroundTabs(minIdle: idleDiscardThreshold)
            }
        }
        timer.tolerance = Self.idleSweepInterval / 2
        idleSweepTimer = timer
    }

    /// Discard every background tab idle at least `minIdle` seconds (LRU order),
    /// keeping its sidebar entry. The active tab and protected tabs are spared.
    private func discardBackgroundTabs(minIdle: TimeInterval) {
        let now = Date()
        let candidates = tabRecency.filter { id in
            id != activeTabId
                && tabIds.contains(id)
                && !discardedTabs.contains(id)
                && !isProtectedFromDiscard(id)
                && now.timeIntervalSince(backgroundedAt[id] ?? .distantPast) >= minIdle
        }
        var changed = false
        for id in candidates {
            if browserTabDiscard(tabIdString(id)) {
                discardedTabs.insert(id)
                changed = true
            }
        }
        if changed {
            host?.updateSidebarBrowserItems(sidebarItems(), activeId: activeTabId)
        }
    }

    /// Tabs that must never be discarded. The active tab is already excluded by
    /// the callers; this is the hook for audio-playing / pinned tabs.
    private func isProtectedFromDiscard(_ id: String) -> Bool {
        false
    }

    /// Manually discard a specific background tab (e.g. from a context menu).
    func discardTab(id: String) {
        guard tabIds.contains(id), id != activeTabId, !discardedTabs.contains(id) else { return }
        if browserTabDiscard(tabIdString(id)) {
            discardedTabs.insert(id)
            host?.updateSidebarBrowserItems(sidebarItems(), activeId: activeTabId)
        }
    }

    private func attachWebView(for tabId: String, attempt: Int) {
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
            LXLog.error("Failed to attach browser webview after \(attempt) retries for tab=\(tabIdString(tabId))",
                        category: "BrowserTabCoordinator")
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
        syncAddressFieldSoon(for: webView)
    }

    @objc private func forwardClicked() {
        guard let webView = activeWebView, webView.canGoForward else { return }
        webView.goForward()
        syncAddressFieldSoon(for: webView)
    }

    @objc private func refreshClicked() {
        activeWebView?.reload()
    }

    // MARK: - Page menu / bookmarks

    /// The active tab's real page URL (empty for hidden startup pages).
    private func activePageURL() -> String {
        guard let activeId = activeTabId else { return "" }
        let raw = activeWebView?.url?.absoluteString ?? lastObservedURLs[activeId] ?? ""
        return BrowserPageMenu.isPageActionable(raw) ? raw : ""
    }

    private func activePageTitle() -> String {
        guard let activeId = activeTabId else { return "" }
        let title = (activeWebView?.title ?? tabTitles[activeId] ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return title
    }

    /// Sync the distinct archive (star) and sidebar shortcut (pin) actions.
    private func updatePageSaveButtons(for url: String?) {
        guard pageActionsVisible else {
            starButton.isHidden = true
            pinButton.isHidden = true
            return
        }
        let raw = url ?? ""
        guard BrowserPageMenu.isBookmarkActionable(raw) else {
            starButton.isHidden = true
            pinButton.isHidden = true
            return
        }
        starButton.isHidden = false
        pinButton.isHidden = false
        // One O(1)-ish FFI call (bit 0 = bookmarked, bit 1 = pinned) instead of
        // decoding a full snapshot on every URL change.
        let state = browserBookmarkState(raw)
        let bookmarked = state & 0b1 != 0
        let pinned = state & 0b10 != 0
        starButton.image = LxIcon.image(
            named: bookmarked ? "icon_bookmark_filled" : "icon_bookmark",
            size: CGSize(width: 16, height: 16)
        )
        starButton.contentTintColor = bookmarked ? .controlAccentColor : .tertiaryLabelColor
        starButton.toolTip = L10n.string(bookmarked ? "lx_browser_remove_bookmark" : "lx_browser_add_bookmark")
        starButton.setAccessibilityLabel(starButton.toolTip ?? "")
        pinButton.image = LxIcon.image(
            named: pinned ? "icon_pin_filled" : "icon_pin",
            size: CGSize(width: 16, height: 16)
        )
        pinButton.contentTintColor = pinned ? .controlAccentColor : .tertiaryLabelColor
        pinButton.toolTip = L10n.string(pinned ? "lx_browser_unpin" : "lx_browser_pin_to_sidebar")
        pinButton.setAccessibilityLabel(pinButton.toolTip ?? "")
    }

    /// Re-derive star/pin state after an external bookmarks mutation (webui
    /// manager page, sidebar tile menu) so a stale filled star can't re-add.
    func refreshPageSaveButtons() {
        guard let activeId = activeTabId else { return }
        updatePageSaveButtons(for: activeWebView?.url?.absoluteString ?? lastObservedURLs[activeId])
    }

    func setPageActionsVisible(_ visible: Bool) {
        pageActionsVisible = visible
        starButton.isHidden = !visible
        pinButton.isHidden = !visible
        bookmarksButton.isHidden = !visible
        menuButton.isHidden = !visible
    }

    private func toggleActiveBookmark() {
        let url = activePageURL()
        guard BrowserPageMenu.isBookmarkActionable(url) else { return }
        _ = browserBookmarkToggle(url, activePageTitle())
        updatePageSaveButtons(for: url)
    }

    private func toggleActivePin() {
        let url = activePageURL()
        guard BrowserPageMenu.isBookmarkActionable(url) else { return }
        let normalized = SidebarBookmarksSnapshot.normalize(url)
        if let pinned = SidebarBookmarksSnapshot.loadFromHost().entries.first(where: {
            SidebarBookmarksSnapshot.normalize($0.url) == normalized
                && shellIsPinned("bookmark", $0.id)
        }) {
            _ = browserBookmarksCommand(
                #"{"op":"setPinned","id":"\#(jsonEscape(pinned.id))","pinned":false}"#
            )
        } else {
            if !browserBookmarkPin(url, activePageTitle()) {
                showShellPinLimitAlert()
            }
        }
        updatePageSaveButtons(for: url)
    }

    @objc private func starClicked() {
        toggleActiveBookmark()
    }

    @objc private func pinClicked() {
        toggleActivePin()
    }

    @objc private func bookmarksClicked() {
        openBookmarks()
    }

    @objc private func menuClicked() {
        let url = activePageURL()
        let context = BrowserPageMenu.Context(
            url: url,
            title: activePageTitle(),
            toastHost: webContainer,
            onBookmarkChanged: { [weak self] _ in
                self?.updatePageSaveButtons(for: url)
            },
            onOpenBookmarks: { [weak self] in
                self?.openBookmarks()
            },
            onOpenHistory: { [weak self] in
                self?.openHistory()
            },
            onClearSiteData: { [weak self] in
                guard let self, let tabId = self.activeTabId else { return }
                self.openClearSiteData(tabId: tabId)
            }
        )
        let menu = BrowserPageMenu.menu(for: context)
        menu.popUp(
            positioning: nil,
            at: NSPoint(x: menuButton.bounds.minX, y: menuButton.bounds.maxY + 6),
            in: menuButton
        )
    }

    /// Browser-scope keyboard shortcuts (active only while a browser tab is
    /// frontmost): ⌘D toggles bookmark, ⌘Y opens history, ⇧⌘C copies the link.
    private func handleShortcut(_ event: NSEvent) -> Bool {
        guard activeTabId != nil else { return false }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let key = event.charactersIgnoringModifiers?.lowercased()
        if flags == [.command], key == "d" {
            toggleActiveBookmark()
            return true
        }
        if flags == [.command], key == "y" {
            openHistory()
            return true
        }
        if flags == [.command, .shift], key == "c" {
            let url = activePageURL()
            guard !url.isEmpty else { return false }
            BrowserPageMenu.copyLink(url, toastHost: webContainer)
            return true
        }
        return false
    }

    @objc private func addressSubmitted(_ sender: NSTextField) {
        guard let result = handleBrowserAddressSubmission(
            rawInput: sender.stringValue,
            currentURL: activeWebView?.url?.absoluteString,
            tabId: activeTabId
        ) else { return }
        _ = openAddressInActiveTab(result.url)
    }

    private func openAddressInActiveTab(_ urlString: String) -> Bool {
        guard let webView = activeWebView,
              let url = URL(string: urlString) else { return false }
        addressField.stringValue = urlString
        addressField.window?.makeFirstResponder(webView)
        // An address-bar navigation is a user interaction.
        markActiveTabInteracted()
        // file:// needs loadFileURL (granting read access to its directory); a normal
        // request is refused by WKWebView.
        if url.isFileURL {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
            return true
        }
        // Managed navigate: a raw WKWebView.load is ignored by the browser's
        // navigation policy. On success re-attach (a blank tab can swap in a
        // fresh webview). Fall back to raw load only if the managed path fails.
        if let activeId = activeTabId, browserTabNavigate(tabIdString(activeId), urlString) {
            attachWebView(for: activeId, attempt: 0)
        } else {
            webView.load(URLRequest(url: url))
        }
        return true
    }

    // MARK: - Browser View Setup

    private func setupBrowserViewIfNeeded() {
        guard browserView == nil else { return }

        // Browser keyboard shortcuts (⌘D bookmark, ⌘Y history, ⇧⌘C copy link). Swallowed
        // only while a browser tab is active, so lxapp views keep their keys.
        if shortcutMonitor == nil {
            shortcutMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                var handled = false
                MainActor.assumeIsolated {
                    guard let self,
                          let window = self.webContainer.window,
                          event.window === window else { return }
                    handled = self.handleShortcut(event)
                }
                return handled ? nil : event
            }
        }

        // First click inside the web area marks the active tab as interacted
        // (observe-only; the event passes through untouched).
        if interactionMonitor == nil {
            interactionMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
                MainActor.assumeIsolated {
                    guard let self,
                          let window = self.webContainer.window,
                          event.window === window else { return }
                    let point = self.webContainer.convert(event.locationInWindow, from: nil)
                    if self.webContainer.bounds.contains(point) {
                        self.markActiveTabInteracted()
                    }
                }
                return event
            }
        }

        let bv = NSView()
        bv.translatesAutoresizingMaskIntoConstraints = false
        bv.wantsLayer = true

        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.wantsLayer = true
        toolbar.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        bv.addSubview(toolbar)

        configureButton(backButton, iconName: "icon_back", action: #selector(backClicked))
        NavButtonState.apply(backButton, enabled: false)
        toolbar.addSubview(backButton)

        configureButton(forwardButton, iconName: "icon_forward", action: #selector(forwardClicked))
        NavButtonState.apply(forwardButton, enabled: false)
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
        addressField.placeholderString = L10n.string("lx_browser_address_placeholder")
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

        starButton.translatesAutoresizingMaskIntoConstraints = false
        starButton.title = ""
        starButton.isBordered = false
        starButton.bezelStyle = .regularSquare
        starButton.imagePosition = .imageOnly
        starButton.imageScaling = .scaleProportionallyDown
        starButton.target = self
        starButton.action = #selector(starClicked)
        starButton.toolTip = L10n.string("lx_browser_add_bookmark")
        starButton.isHidden = true
        addressBarContainer.addSubview(starButton)

        pinButton.translatesAutoresizingMaskIntoConstraints = false
        pinButton.title = ""
        pinButton.isBordered = false
        pinButton.bezelStyle = .regularSquare
        pinButton.imagePosition = .imageOnly
        pinButton.imageScaling = .scaleProportionallyDown
        pinButton.target = self
        pinButton.action = #selector(pinClicked)
        pinButton.toolTip = L10n.string("lx_browser_pin_to_sidebar")
        pinButton.isHidden = true
        addressBarContainer.addSubview(pinButton)

        configureButton(bookmarksButton, iconName: "icon_bookmarks", action: #selector(bookmarksClicked))
        bookmarksButton.toolTip = L10n.string("lx_browser_manage_bookmarks")
        bookmarksButton.setAccessibilityLabel(bookmarksButton.toolTip ?? "")
        toolbar.addSubview(bookmarksButton)
        bookmarksButton.isHidden = !pageActionsVisible

        configureButton(menuButton, iconName: "icon_page_menu", action: #selector(menuClicked))
        menuButton.toolTip = L10n.string("lx_browser_page_menu")
        menuButton.setAccessibilityLabel(menuButton.toolTip ?? "")
        toolbar.addSubview(menuButton)
        menuButton.isHidden = !pageActionsVisible

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
        let bookmarksCenterY = bookmarksButton.centerYAnchor.constraint(equalTo: toolbar.topAnchor, constant: Layout.toolbarCenterY)
        let menuCenterY = menuButton.centerYAnchor.constraint(equalTo: toolbar.topAnchor, constant: Layout.toolbarCenterY)
        toolbarCenterYConstraints = [
            backCenterY, forwardCenterY, refreshCenterY, addressCenterY,
            bookmarksCenterY, menuCenterY,
        ]

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
            addressBarContainer.trailingAnchor.constraint(
                equalTo: bookmarksButton.leadingAnchor,
                constant: pageActionsVisible ? -8 : 0
            ),
            addressCenterY,
            addressBarContainer.heightAnchor.constraint(equalToConstant: Layout.addressBarHeight),

            bookmarksButton.trailingAnchor.constraint(
                equalTo: menuButton.leadingAnchor,
                constant: pageActionsVisible ? -4 : 0
            ),
            bookmarksCenterY,
            bookmarksButton.widthAnchor.constraint(
                equalToConstant: pageActionsVisible ? Layout.buttonSize : 0
            ),
            bookmarksButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            menuButton.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor, constant: -8),
            menuCenterY,
            menuButton.widthAnchor.constraint(
                equalToConstant: pageActionsVisible ? Layout.buttonSize : 0
            ),
            menuButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            addressField.leadingAnchor.constraint(equalTo: addressBarContainer.leadingAnchor, constant: 8),
            addressField.trailingAnchor.constraint(equalTo: starButton.leadingAnchor, constant: -4),
            addressField.centerYAnchor.constraint(equalTo: addressBarContainer.centerYAnchor),

            starButton.trailingAnchor.constraint(
                equalTo: pinButton.leadingAnchor,
                constant: pageActionsVisible ? -2 : 0
            ),
            starButton.centerYAnchor.constraint(equalTo: addressBarContainer.centerYAnchor),
            starButton.widthAnchor.constraint(equalToConstant: pageActionsVisible ? 20 : 0),
            starButton.heightAnchor.constraint(equalToConstant: 20),

            pinButton.trailingAnchor.constraint(equalTo: addressBarContainer.trailingAnchor, constant: -5),
            pinButton.centerYAnchor.constraint(equalTo: addressBarContainer.centerYAnchor),
            pinButton.widthAnchor.constraint(equalToConstant: pageActionsVisible ? 20 : 0),
            pinButton.heightAnchor.constraint(equalToConstant: 20),

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
        NavButtonState.apply(backButton, enabled: canGoBack && activeTabInteracted())
    }

    private func updateForwardButtonState(canGoForward: Bool) {
        NavButtonState.apply(forwardButton, enabled: canGoForward && activeTabInteracted())
    }

    /// Chrome-style history intervention: until the user interacts with a
    /// tab (click in its page or an address-bar navigation), auto-created
    /// history (SPA pushState redirects) must not light back/forward.
    private func activeTabInteracted() -> Bool {
        guard let id = activeTabId else { return false }
        return interactedTabs.contains(id)
    }

    func markActiveTabInteracted() {
        guard let id = activeTabId, !interactedTabs.contains(id) else { return }
        interactedTabs.insert(id)
        if let webView = activeWebView {
            updateBackButtonState(canGoBack: webView.canGoBack)
            updateForwardButtonState(canGoForward: webView.canGoForward)
        }
    }

    private func configureButton(_ button: NSButton, iconName: String, action: Selector) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.title = ""
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
        Layout.buttonLeading
    }

    // MARK: - Data Helpers

    private func tabIdString(_ id: String) -> String {
        id.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func displayableURL(_ raw: String?) -> String {
        guard let raw else { return "" }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        return browserUrlIsHidden(trimmed) ? "" : trimmed
    }

    private func syncAddressField(_ rawURL: String?, force: Bool = false) {
        guard force || addressField.currentEditor() == nil else { return }
        addressField.stringValue = displayableURL(rawURL)
    }

    private func syncAddressFieldSoon(for webView: WKWebView) {
        syncAddressField(webView.url?.absoluteString, force: true)
        DispatchQueue.main.async { [weak self, weak webView] in
            guard let self, let webView, webView === self.activeWebView else { return }
            self.syncAddressField(webView.url?.absoluteString, force: true)
        }
    }

    private func sidebarItems() -> [(id: String, title: String, url: String, favicon: NSImage?)] {
        // Favicons are normally kicked by the URL observation — which never
        // fires for tabs RESTORED across a restart. Kick any icon-less tab
        // here (idempotent: the request-origin map dedupes in-flight work).
        for id in tabIds where tabFavicons[id] == nil {
            guard let raw = lastObservedURLs[id], let url = URL(string: raw),
                  let origin = faviconRequestOrigin(for: url),
                  tabFaviconRequestOrigins[id] != origin,
                  let webView = findWebView(for: id) else { continue }
            tabFaviconRequestOrigins[id] = origin
            fetchFavicon(for: origin, tabId: id, webView: webView)
        }
        return tabIds.map { id in
            (id, tabTitles[id] ?? L10n.string("lx_browser_new_tab"), lastObservedURLs[id] ?? "", tabFavicons[id])
        }
    }

    private func handleTitleChanged(id: String, title: String) {
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
        let bundle = Bundle.lingxiaResources
        #else
        let bundle = Bundle(for: BrowserTabCoordinator.self)
        #endif
        guard let faviconURL = bundle.url(forResource: "favicon", withExtension: "ico") else {
            return nil
        }
        return NSImage(contentsOf: faviconURL)
    }

    private func fetchFavicon(for origin: String, tabId: String, webView: WKWebView) {
        if origin.hasPrefix("lingxia://") {
            guard let image = bundledFavicon() else { return }
            tabFavicons[tabId] = image
            host?.updateSidebarBrowserItems(sidebarItems(), activeId: activeTabId)
            return
        }

        // Page-declared icon first, /favicon.ico fallback (FaviconLoader waits
        // for the load to settle — the old fetch ran at navigation time and
        // only tried the default path, so most sites never got an icon).
        Task { @MainActor [weak self] in
            guard let image = await FaviconLoader.resolve(webView: webView) else { return }
            guard let self,
                  self.tabIds.contains(tabId),
                  self.tabFaviconRequestOrigins[tabId] == origin else { return }
            self.tabFavicons[tabId] = image
            self.host?.updateSidebarBrowserItems(self.sidebarItems(), activeId: self.activeTabId)
        }
    }
}

#endif
