#if os(macOS)
import AppKit
import WebKit

// MARK: - BrowserViewController

/// Browser content view controller with toolbar (back, refresh, address bar) and WKWebView.
/// Used for opening standard HTTPS websites in sidebar browser tabs.
@MainActor
public class BrowserViewController: NSViewController, WKNavigationDelegate, WKUIDelegate {

    struct Layout {
        static let toolbarHeight: CGFloat = 38
        static let buttonSize: CGFloat = 28
        static let leadingPadding: CGFloat = 8
        static let addressBarHeight: CGFloat = 26
        static let addressBarCornerRadius: CGFloat = 6
        static let buttonSpacing: CGFloat = 4
    }

    let id: UUID
    private(set) var url: URL?
    private(set) var pageTitle: String = "New Tab"

    var onTitleChanged: ((UUID, String) -> Void)?
    var onURLChanged: ((UUID, URL?) -> Void)?

    private let toolbar = NSView()
    private let toolbarSeparator = NSView()
    private let backButton = NSButton()
    private let refreshButton = NSButton()
    private let addressBarContainer = NSView()
    private let addressBar = NSTextField()
    private var webView: WKWebView!
    private var titleObservation: NSKeyValueObservation?
    private var urlObservation: NSKeyValueObservation?

    init(id: UUID) {
        self.id = id
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        titleObservation?.invalidate()
        urlObservation?.invalidate()
    }

    public override func loadView() {
        let root = NSView()
        root.wantsLayer = true
        self.view = root

        setupToolbar()
        setupWebView()
    }

    public override func viewDidAppear() {
        super.viewDidAppear()
        // Auto-focus address bar when appearing
        view.window?.makeFirstResponder(addressBar)
    }

    // MARK: - Setup

    private func setupToolbar() {
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.wantsLayer = true
        toolbar.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        view.addSubview(toolbar)

        // Back button
        backButton.translatesAutoresizingMaskIntoConstraints = false
        backButton.image = NSImage(systemSymbolName: "chevron.left", accessibilityDescription: "Back")
        backButton.isBordered = false
        backButton.bezelStyle = .regularSquare
        backButton.imagePosition = .imageOnly
        backButton.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
        backButton.target = self
        backButton.action = #selector(backClicked)
        toolbar.addSubview(backButton)

        // Refresh button
        refreshButton.translatesAutoresizingMaskIntoConstraints = false
        refreshButton.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: "Refresh")
        refreshButton.isBordered = false
        refreshButton.bezelStyle = .regularSquare
        refreshButton.imagePosition = .imageOnly
        refreshButton.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
        refreshButton.target = self
        refreshButton.action = #selector(refreshClicked)
        toolbar.addSubview(refreshButton)

        // Address bar container (rounded-rect background)
        addressBarContainer.translatesAutoresizingMaskIntoConstraints = false
        addressBarContainer.wantsLayer = true
        addressBarContainer.layer?.cornerRadius = Layout.addressBarCornerRadius
        addressBarContainer.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        toolbar.addSubview(addressBarContainer)

        // Address bar text field (inside container, padded by constraints)
        addressBar.translatesAutoresizingMaskIntoConstraints = false
        addressBar.font = NSFont.systemFont(ofSize: 13)
        addressBar.placeholderString = "Enter URL"
        addressBar.stringValue = ""
        addressBar.isBordered = false
        addressBar.drawsBackground = false
        addressBar.focusRingType = .none
        addressBar.usesSingleLineMode = true
        addressBar.cell?.wraps = false
        addressBar.cell?.isScrollable = true
        addressBar.cell?.lineBreakMode = .byTruncatingTail
        addressBar.delegate = self
        addressBarContainer.addSubview(addressBar)

        // Toolbar separator
        toolbarSeparator.translatesAutoresizingMaskIntoConstraints = false
        toolbarSeparator.wantsLayer = true
        toolbarSeparator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        view.addSubview(toolbarSeparator)

        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: view.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: Layout.toolbarHeight),

            backButton.leadingAnchor.constraint(equalTo: toolbar.leadingAnchor, constant: Layout.leadingPadding),
            backButton.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            backButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            backButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            refreshButton.leadingAnchor.constraint(equalTo: backButton.trailingAnchor, constant: Layout.buttonSpacing),
            refreshButton.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            refreshButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            refreshButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            // Address bar container
            addressBarContainer.leadingAnchor.constraint(equalTo: refreshButton.trailingAnchor, constant: 8),
            addressBarContainer.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor, constant: -Layout.leadingPadding),
            addressBarContainer.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            addressBarContainer.heightAnchor.constraint(equalToConstant: Layout.addressBarHeight),

            // Text field centered inside container with horizontal padding
            addressBar.leadingAnchor.constraint(equalTo: addressBarContainer.leadingAnchor, constant: 8),
            addressBar.trailingAnchor.constraint(equalTo: addressBarContainer.trailingAnchor, constant: -8),
            addressBar.centerYAnchor.constraint(equalTo: addressBarContainer.centerYAnchor),

            toolbarSeparator.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            toolbarSeparator.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            toolbarSeparator.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            toolbarSeparator.heightAnchor.constraint(equalToConstant: 1),
        ])
    }

    private func setupWebView() {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .default()

        webView = WKWebView(frame: .zero, configuration: config)
        webView.translatesAutoresizingMaskIntoConstraints = false
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.allowsBackForwardNavigationGestures = true
        view.addSubview(webView)

        NSLayoutConstraint.activate([
            webView.topAnchor.constraint(equalTo: toolbarSeparator.bottomAnchor),
            webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])

        // Observe title changes
        titleObservation = webView.observe(\.title, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self else { return }
                let newTitle = webView.title ?? "Untitled"
                if !newTitle.isEmpty {
                    self.pageTitle = newTitle
                    self.onTitleChanged?(self.id, newTitle)
                }
            }
        }

        // Observe URL changes
        urlObservation = webView.observe(\.url, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self else { return }
                self.url = webView.url
                self.addressBar.stringValue = webView.url?.absoluteString ?? ""
                self.onURLChanged?(self.id, webView.url)
            }
        }
    }

    // MARK: - Public API

    func loadURL(_ url: URL) {
        self.url = url
        addressBar.stringValue = url.absoluteString
        webView.load(URLRequest(url: url))
    }

    func pause() {
        // Minimal pause — nothing to do for standard web content
    }

    func resume() {
        // Minimal resume — nothing to do for standard web content
    }

    // MARK: - Actions

    @objc private func backClicked() {
        if webView.canGoBack {
            webView.goBack()
        }
    }

    @objc private func refreshClicked() {
        webView.reload()
    }

    // MARK: - WKNavigationDelegate

    public func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction,
                        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        decisionHandler(.allow)
    }

    public func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        addressBar.stringValue = webView.url?.absoluteString ?? ""
    }

    public func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        NSLog("[BrowserVC] Provisional navigation failed: \(error.localizedDescription)")
    }

    public func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        NSLog("[BrowserVC] Navigation failed: \(error.localizedDescription)")
    }

    public func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        backButton.isEnabled = webView.canGoBack
        backButton.alphaValue = webView.canGoBack ? 1.0 : 0.4
    }

    // MARK: - WKUIDelegate

    /// Handle window.open() — load in the same webview instead of opening a new window
    public func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                        for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let url = navigationAction.request.url {
            webView.load(URLRequest(url: url))
        }
        return nil
    }

    /// Handle JavaScript alert()
    public func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String,
                        initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
        guard let window = view.window else {
            completionHandler()
            return
        }
        let alert = NSAlert()
        alert.messageText = webView.url?.host ?? "Alert"
        alert.informativeText = message
        alert.addButton(withTitle: "OK")
        alert.beginSheetModal(for: window) { _ in
            completionHandler()
        }
    }

    /// Handle JavaScript confirm()
    public func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String,
                        initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (Bool) -> Void) {
        guard let window = view.window else {
            completionHandler(false)
            return
        }
        let alert = NSAlert()
        alert.messageText = webView.url?.host ?? "Confirm"
        alert.informativeText = message
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { response in
            completionHandler(response == .alertFirstButtonReturn)
        }
    }

    /// Handle JavaScript prompt()
    public func webView(_ webView: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String,
                        defaultText: String?, initiatedByFrame frame: WKFrameInfo,
                        completionHandler: @escaping (String?) -> Void) {
        guard let window = view.window else {
            completionHandler(nil)
            return
        }
        let alert = NSAlert()
        alert.messageText = webView.url?.host ?? "Prompt"
        alert.informativeText = prompt
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 260, height: 24))
        input.stringValue = defaultText ?? ""
        alert.accessoryView = input
        alert.beginSheetModal(for: window) { response in
            completionHandler(response == .alertFirstButtonReturn ? input.stringValue : nil)
        }
    }
}

// MARK: - NSTextFieldDelegate

extension BrowserViewController: NSTextFieldDelegate {

    public func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            commitAddressBar()
            return true
        }
        return false
    }

    private func commitAddressBar() {
        var text = addressBar.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        // Auto-add https:// if no scheme
        if !text.contains("://") {
            text = "https://\(text)"
        }

        guard let url = URL(string: text) else { return }
        loadURL(url)

        // Resign first responder to dismiss keyboard focus
        view.window?.makeFirstResponder(webView)
    }
}

#endif
