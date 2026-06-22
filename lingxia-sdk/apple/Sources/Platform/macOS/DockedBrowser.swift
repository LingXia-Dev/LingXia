#if os(macOS)
import AppKit
import WebKit
import OSLog
import CLingXiaRustAPI

/// A self-contained browser docked beside the main content as an aside.
///
/// Unlike a bare `WKWebView`, the webview here is created through the Rust
/// browser path (`openBrowserTab`) exactly like a main browser tab, so it is a
/// real LingXia browser webview that receives native mouse/keyboard input and
/// drives back/forward state. It carries its own chrome (back / forward /
/// refresh / address bar / close) and its own KVO, fully independent of the
/// singleton `BrowserTabCoordinator`: the docked tab is never registered as a
/// switchable main tab or a sidebar item — it lives and dies with the aside.
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
        static let addressHeight: CGFloat = 26
        static let edge: CGFloat = 8
    }

    /// Root view handed to the aside panel slot.
    let containerView = NSView()

    private let tabId: String
    private let onClose: () -> Void

    private let toolbar = NSView()
    private let backButton = NSButton()
    private let forwardButton = NSButton()
    private let refreshButton = NSButton()
    private let closeButton = NSButton()
    private let addressBarContainer = NSView()
    private let addressField = NSTextField()
    private let separator = NSView()
    private let webContainer = NSView()

    private var webView: WKWebView?
    private var torn = false

    /// Registry of live docked browsers by their runtime tab id. Lets the tab
    /// coordinator route native-input prep to the aside in place instead of
    /// relocating the tab into the main browser area (which would empty the
    /// aside). Weakly held so teardown is the single owner of lifetime.
    private final class WeakRef {
        weak var value: DockedBrowser?
        init(_ value: DockedBrowser) { self.value = value }
    }
    private static var registry: [String: WeakRef] = [:]

    /// The live docked browser hosting `tabId`, if any.
    static func forTab(_ tabId: String) -> DockedBrowser? {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return nil }
        return registry[normalized]?.value
    }

    nonisolated(unsafe) private var urlObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var backObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var forwardObservation: NSKeyValueObservation?

    /// Create a browser tab for `url` owned by `owner` and start wiring it into a
    /// docked container. Returns nil if the tab could not be created. The webview
    /// itself is resolved asynchronously (browser webview creation is async), so
    /// it is attached on a short retry once Rust reports it ready.
    init?(owner: (appId: String, sessionId: UInt64), url: String, onClose: @escaping () -> Void) {
        guard let opened = openStandaloneBrowserTab(owner.appId, owner.sessionId, url) else {
            os_log("openStandaloneBrowserTab failed for docked browser url=%{public}@", log: Self.log, type: .error, url)
            return nil
        }
        let resolvedTabId = opened.toString().trimmingCharacters(in: .whitespacesAndNewlines)
        guard !resolvedTabId.isEmpty else {
            os_log("openBrowserTab returned empty tab id for docked browser", log: Self.log, type: .error)
            return nil
        }
        self.tabId = resolvedTabId
        self.onClose = onClose
        super.init()
        Self.registry[resolvedTabId] = WeakRef(self)
        buildChrome(initialURL: url)
        attachWhenReady(attempt: 0)
    }

    /// Make this aside's own webview first responder for native input prep,
    /// in place — so input-prep (e.g. browser automation) never relocates the
    /// docked tab into the main browser area. Returns false until the webview
    /// has been resolved/attached.
    func focusForInput() -> Bool {
        guard let webView, let window = webView.window else { return false }
        window.makeFirstResponder(webView)
        return true
    }

    /// Tear down the docked browser: stop KVO, detach the webview, and destroy
    /// the underlying Rust tab. Idempotent — the surface close path and the close
    /// button can both reach here. The tab is intentionally closed (not merely
    /// discarded): it has no sidebar entry to reactivate from.
    func tearDown() {
        guard !torn else { return }
        torn = true
        Self.registry.removeValue(forKey: tabId)
        urlObservation?.invalidate()
        backObservation?.invalidate()
        forwardObservation?.invalidate()
        urlObservation = nil
        backObservation = nil
        forwardObservation = nil
        if let webView {
            webView.stopLoading()
            webView.removeFromSuperview()
        }
        webView = nil
        _ = browserTabClose(tabId)
    }

    // MARK: - Chrome

    private func buildChrome(initialURL: String) {
        containerView.wantsLayer = true
        containerView.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor

        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.wantsLayer = true
        containerView.addSubview(toolbar)

        configureToolButton(backButton, iconName: "icon_back", action: #selector(backClicked))
        configureToolButton(forwardButton, iconName: "icon_forward", action: #selector(forwardClicked))
        configureToolButton(refreshButton, iconName: "icon_browser_refresh", action: #selector(refreshClicked))
        toolbar.addSubview(backButton)
        toolbar.addSubview(forwardButton)
        toolbar.addSubview(refreshButton)

        addressBarContainer.translatesAutoresizingMaskIntoConstraints = false
        addressBarContainer.wantsLayer = true
        addressBarContainer.layer?.cornerRadius = 6
        addressBarContainer.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        toolbar.addSubview(addressBarContainer)

        addressField.translatesAutoresizingMaskIntoConstraints = false
        addressField.font = NSFont.systemFont(ofSize: 13)
        addressField.placeholderString = "Enter address"
        addressField.isBordered = false
        addressField.drawsBackground = false
        addressField.focusRingType = .none
        addressField.usesSingleLineMode = true
        addressField.cell?.wraps = false
        addressField.cell?.isScrollable = true
        addressField.cell?.lineBreakMode = .byTruncatingTail
        addressField.stringValue = initialURL
        addressField.target = self
        addressField.action = #selector(addressSubmitted(_:))
        addressBarContainer.addSubview(addressField)

        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.isBordered = false
        closeButton.bezelStyle = .regularSquare
        closeButton.imagePosition = .imageOnly
        closeButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close")
        closeButton.contentTintColor = NSColor.secondaryLabelColor
        closeButton.toolTip = "Close"
        closeButton.target = self
        closeButton.action = #selector(closeClicked)
        toolbar.addSubview(closeButton)

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

            closeButton.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor, constant: -Layout.edge),
            closeButton.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            closeButton.widthAnchor.constraint(equalToConstant: Layout.closeSize),
            closeButton.heightAnchor.constraint(equalToConstant: Layout.closeSize),

            addressBarContainer.leadingAnchor.constraint(equalTo: refreshButton.trailingAnchor, constant: Layout.edge),
            addressBarContainer.trailingAnchor.constraint(equalTo: closeButton.leadingAnchor, constant: -Layout.edge),
            addressBarContainer.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            addressBarContainer.heightAnchor.constraint(equalToConstant: Layout.addressHeight),

            addressField.leadingAnchor.constraint(equalTo: addressBarContainer.leadingAnchor, constant: Layout.edge),
            addressField.trailingAnchor.constraint(equalTo: addressBarContainer.trailingAnchor, constant: -Layout.edge),
            addressField.centerYAnchor.constraint(equalTo: addressBarContainer.centerYAnchor),

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

    // MARK: - WebView wiring

    private func attachWhenReady(attempt: Int) {
        guard !torn else { return }
        if let webView = resolveWebView() {
            attach(webView)
            return
        }
        guard attempt < Self.maxAttachRetry else {
            os_log("docked browser webview never became ready tab=%{public}@", log: Self.log, type: .error, tabId)
            onClose()
            return
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.attachRetryDelay) { [weak self] in
            self?.attachWhenReady(attempt: attempt + 1)
        }
    }

    private func resolveWebView() -> WKWebView? {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else { return nil }
        let path = browserTabPathForId(tabId).toString()
        return WebViewManager.resolveWebView(appId: appId, path: path, sessionId: sessionId)
    }

    private func attach(_ webView: WKWebView) {
        if #available(macOS 13.3, *) {
            webView.isInspectable = true
        }
        self.webView = webView
        WebViewManager.attachWebViewToContainer(webView, container: webContainer)
        addressField.stringValue = displayURL(webView.url?.absoluteString)
        updateBackForward(canGoBack: webView.canGoBack, canGoForward: webView.canGoForward)
        observe(webView)
        webView.window?.makeFirstResponder(webView)
    }

    private func observe(_ webView: WKWebView) {
        urlObservation = webView.observe(\.url, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, !self.torn else { return }
                if self.addressField.currentEditor() == nil {
                    self.addressField.stringValue = self.displayURL(webView.url?.absoluteString)
                }
                _ = updateBrowserTabInfo(self.tabId, webView.url?.absoluteString ?? "", webView.title ?? "")
            }
        }
        backObservation = webView.observe(\.canGoBack, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, !self.torn else { return }
                self.updateBackForward(canGoBack: webView.canGoBack, canGoForward: webView.canGoForward)
            }
        }
        forwardObservation = webView.observe(\.canGoForward, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, !self.torn else { return }
                self.updateBackForward(canGoBack: webView.canGoBack, canGoForward: webView.canGoForward)
            }
        }
    }

    private func updateBackForward(canGoBack: Bool, canGoForward: Bool) {
        backButton.isEnabled = canGoBack
        backButton.alphaValue = canGoBack ? 1.0 : 0.4
        forwardButton.isEnabled = canGoForward
        forwardButton.alphaValue = canGoForward ? 1.0 : 0.4
    }

    private func displayURL(_ raw: String?) -> String {
        guard let raw else { return "" }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        return browserUrlIsHidden(trimmed) ? "" : trimmed
    }

    // MARK: - Actions

    @objc private func backClicked() {
        guard let webView, webView.canGoBack else { return }
        webView.goBack()
    }

    @objc private func forwardClicked() {
        guard let webView, webView.canGoForward else { return }
        webView.goForward()
    }

    @objc private func refreshClicked() {
        webView?.reload()
    }

    @objc private func closeClicked() {
        onClose()
    }

    @objc private func addressSubmitted(_ sender: NSTextField) {
        guard let url = Self.resolveAddress(sender.stringValue) else { return }
        addressField.stringValue = url.absoluteString
        webView?.load(URLRequest(url: url))
    }

    /// Turn raw address-bar text into a URL: honor an explicit scheme, treat a
    /// dotted token as a host (https), otherwise leave free text to the start
    /// page/search-provider flow instead of hardcoding a native provider.
    static func resolveAddress(_ raw: String) -> URL? {
        let text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.isEmpty { return nil }
        if let url = URL(string: text), url.scheme != nil { return url }
        if text.contains(".") && !text.contains(" ") {
            return URL(string: "https://\(text)")
        }
        return nil
    }
}
#endif
