import AppKit
import WebKit
import os.log

@MainActor
final class RunnerBrowserOverlay {
    private static let log = OSLog(subsystem: "LingXiaRunner", category: "BrowserOverlay")

    private var overlayView: NSView?
    private var webContainer: NSView?
    private var activeTabId: String?
    private weak var hostWindow: NSWindow?

    private var addressField: NSTextField?
    private var backButton: NSButton?
    private var forwardButton: NSButton?
    private var refreshButton: NSButton?
    private var urlObservation: NSKeyValueObservation?
    private var canGoBackObservation: NSKeyValueObservation?
    private var canGoForwardObservation: NSKeyValueObservation?

    var activeWebView: WKWebView? {
        guard activeTabId != nil, overlayView?.isHidden == false else { return nil }
        return webContainer?.subviews.compactMap { $0 as? WKWebView }.first
    }

    func present(tabId: String, in phoneContent: NSView, window: NSWindow?) {
        let normalized = tabId.lowercased()
        guard !normalized.isEmpty else { return }

        if let activeTabId, activeTabId != normalized {
            clearWebViewAttachment()
            _ = RunnerSupport.Browser.closeTab(tabId: activeTabId)
        }

        activeTabId = normalized
        hostWindow = window
        show(in: phoneContent)
        attachWebView(tabId: normalized, attempt: 0)
    }

    func dismiss(closeTab: Bool) {
        let tabId = activeTabId
        activeTabId = nil
        clearWebViewAttachment()
        overlayView?.isHidden = true

        if closeTab, let tabId {
            _ = RunnerSupport.Browser.closeTab(tabId: tabId)
        }
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
        barBackground.layer?.cornerRadius = 16
        barBackground.layer?.masksToBounds = true
        bottomBar.addSubview(barBackground)

        let controlRow = NSStackView()
        controlRow.translatesAutoresizingMaskIntoConstraints = false
        controlRow.orientation = .horizontal
        controlRow.alignment = .centerY
        controlRow.spacing = 6
        barBackground.addSubview(controlRow)

        let backButton = makeIconButton(named: "icon_back", action: #selector(backClicked))
        let forwardButton = makeIconButton(named: "icon_forward", action: #selector(forwardClicked))
        let refreshButton = makeIconButton(named: "icon_browser_refresh", action: #selector(refreshClicked))
        let closeButton = makeIconButton(named: "icon_close_x", action: #selector(closeClicked))

        let addressPill = NSView()
        addressPill.translatesAutoresizingMaskIntoConstraints = false
        addressPill.wantsLayer = true
        addressPill.layer?.backgroundColor = NSColor.textBackgroundColor.withAlphaComponent(0.74).cgColor
        addressPill.layer?.cornerRadius = 18
        addressPill.layer?.masksToBounds = true

        let addressField = NSTextField()
        addressField.translatesAutoresizingMaskIntoConstraints = false
        addressField.isBordered = false
        addressField.drawsBackground = false
        addressField.focusRingType = .none
        addressField.font = NSFont.systemFont(ofSize: 13)
        addressField.lineBreakMode = .byTruncatingMiddle
        addressField.usesSingleLineMode = true
        addressField.target = self
        addressField.action = #selector(addressSubmitted)
        addressPill.addSubview(addressField)

        controlRow.addArrangedSubview(backButton)
        controlRow.addArrangedSubview(forwardButton)
        controlRow.addArrangedSubview(addressPill)
        controlRow.addArrangedSubview(refreshButton)
        controlRow.addArrangedSubview(closeButton)

        self.backButton = backButton
        self.forwardButton = forwardButton
        self.refreshButton = refreshButton
        self.addressField = addressField

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
            bottomBar.bottomAnchor.constraint(equalTo: overlay.bottomAnchor, constant: -4),
            bottomBar.heightAnchor.constraint(equalToConstant: 52),

            barBackground.leadingAnchor.constraint(equalTo: bottomBar.leadingAnchor, constant: 12),
            barBackground.trailingAnchor.constraint(equalTo: bottomBar.trailingAnchor, constant: -12),
            barBackground.topAnchor.constraint(equalTo: bottomBar.topAnchor),
            barBackground.bottomAnchor.constraint(equalTo: bottomBar.bottomAnchor),

            controlRow.leadingAnchor.constraint(equalTo: barBackground.leadingAnchor, constant: 8),
            controlRow.trailingAnchor.constraint(equalTo: barBackground.trailingAnchor, constant: -8),
            controlRow.centerYAnchor.constraint(equalTo: barBackground.centerYAnchor),

            addressPill.heightAnchor.constraint(equalToConstant: 36),
            addressPill.widthAnchor.constraint(greaterThanOrEqualToConstant: 120),
            addressField.leadingAnchor.constraint(equalTo: addressPill.leadingAnchor, constant: 12),
            addressField.trailingAnchor.constraint(equalTo: addressPill.trailingAnchor, constant: -12),
            addressField.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),
        ])
        addressPill.setContentHuggingPriority(.defaultLow, for: .horizontal)
        addressPill.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        overlayView = overlay
        self.webContainer = webContainer
        updateNavigationButtons()
    }

    private func show(in phoneContent: NSView) {
        setupIfNeeded(in: phoneContent)
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
            dismiss(closeTab: true)
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
        addressField?.stringValue = url?.absoluteString ?? ""
    }

    private func updateNavigationButtons() {
        let webView = activeWebView
        backButton?.isEnabled = webView?.canGoBack ?? false
        backButton?.alphaValue = (webView?.canGoBack ?? false) ? 1.0 : 0.35
        forwardButton?.isEnabled = webView?.canGoForward ?? false
        forwardButton?.alphaValue = (webView?.canGoForward ?? false) ? 1.0 : 0.35
        refreshButton?.isEnabled = webView != nil
        refreshButton?.alphaValue = webView == nil ? 0.35 : 1.0
    }

    private func makeIconButton(named iconName: String, action: Selector) -> NSButton {
        let button = NSButton()
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.image = RunnerSupport.Assets.image(named: iconName, size: CGSize(width: 20, height: 20))
        button.imageScaling = .scaleProportionallyDown
        button.target = self
        button.action = action
        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: 40),
            button.heightAnchor.constraint(equalToConstant: 40),
        ])
        return button
    }

    @objc private func closeClicked() {
        dismiss(closeTab: true)
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
        guard let tabId = activeTabId,
              let result = RunnerSupport.Browser.handleAddressSubmission(
                rawInput: addressField?.stringValue ?? "",
                currentURL: activeWebView?.url?.absoluteString,
                tabId: tabId
              ),
              let url = URL(string: result.url) else {
            return
        }
        addressField?.stringValue = result.displayText
        hostWindow?.makeFirstResponder(nil)
        activeWebView?.load(URLRequest(url: url))
    }
}
