#if os(iOS)
import UIKit
import WebKit
import OSLog
import CLingXiaRustAPI

@MainActor
final class LxAppBrowserOverlay: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "BrowserOverlay")
    private static var currentController: LxAppBrowserViewController?

    static func show(tabId: String) -> Bool {
        let normalizedTabId = tabId.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !normalizedTabId.isEmpty else {
            os_log("show failed: empty tab id", log: log, type: .error)
            return false
        }

        guard let manager = iOSLxApp.getInstance().currentLxAppManager,
              let navController = manager.navigationController else {
            os_log("show failed: no active navigation controller", log: log, type: .error)
            return false
        }

        // Reuse if already showing the same tab
        if let existing = currentController, existing.tabId == normalizedTabId {
            return true
        }
        if let top = navController.topViewController as? LxAppBrowserViewController,
           top.tabId == normalizedTabId {
            currentController = top
            return true
        }

        // Dismiss current browser before opening a new one
        if let existing = currentController {
            existing.closeManagedTabIfNeeded()
            if existing.navigationController?.topViewController === existing {
                existing.navigationController?.popViewController(animated: false)
            }
        } else if let top = navController.topViewController as? LxAppBrowserViewController {
            top.closeManagedTabIfNeeded()
            navController.popViewController(animated: false)
        }

        let controller = LxAppBrowserViewController(tabId: normalizedTabId)
        navController.pushViewController(controller, animated: true)
        currentController = controller
        os_log("Browser view controller pushed for tab=%{public}@", log: log, type: .info, normalizedTabId)
        return true
    }

    @objc public static func dismiss() {
        guard let controller = currentController else { return }

        if controller.navigationController?.topViewController === controller {
            controller.navigationController?.popViewController(animated: true)
        } else {
            controller.closeManagedTabIfNeeded()
        }
    }

    static func isShowing() -> Bool {
        guard let controller = currentController else { return false }
        return controller.navigationController?.topViewController === controller
    }

    fileprivate static func browserControllerDidClose(_ controller: LxAppBrowserViewController) {
        if currentController === controller {
            currentController = controller.navigationController?.topViewController as? LxAppBrowserViewController
        }
    }
}

@MainActor
private final class LxAppBrowserViewController: UIViewController, UITextFieldDelegate, UIGestureRecognizerDelegate {
    private static let log = OSLog(subsystem: "LingXia", category: "BrowserOverlayViewController")
    private static let attachRetryDelay: TimeInterval = 0.1
    private static let maxAttachRetries = 8

    let tabId: String

    private let addressPill = UIView()
    private let addressField = UITextField()
    private let refreshButton = UIButton(type: .system)
    private let contentContainer = UIView()
    private let bottomBar = UIView()
    private let bottomBarBackground = UIVisualEffectView(effect: UIBlurEffect(style: .systemThinMaterial))
    private let backButton = UIButton(type: .system)
    private let forwardButton = UIButton(type: .system)
    private let closeButton = UIButton(type: .system)

    private var activeBrowserWebView: WKWebView?
    private var urlObservation: NSKeyValueObservation?
    private var canGoBackObservation: NSKeyValueObservation?
    private var canGoForwardObservation: NSKeyValueObservation?
    private var attachRetryWorkItem: DispatchWorkItem?
    private var didCloseManagedTab = false
    private var backEdgePanGesture: UIScreenEdgePanGestureRecognizer?
    private var bottomBarBottomConstraint: NSLayoutConstraint?

    init(tabId: String) {
        self.tabId = tabId
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .fullScreen
        hidesBottomBarWhenPushed = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .white
        setupUI()
        setupBackGestureRecognizer()
        attachManagedWebViewIfNeeded()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        navigationController?.setNavigationBarHidden(true, animated: false)
        navigationController?.interactivePopGestureRecognizer?.isEnabled = true
        navigationController?.interactivePopGestureRecognizer?.delegate = nil
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardWillShow(_:)), name: UIResponder.keyboardWillShowNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardWillHide(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        NotificationCenter.default.removeObserver(self, name: UIResponder.keyboardWillShowNotification, object: nil)
        NotificationCenter.default.removeObserver(self, name: UIResponder.keyboardWillHideNotification, object: nil)
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        attachManagedWebViewIfNeeded()
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)

        if isMovingFromParent || isBeingDismissed {
            closeManagedTabIfNeeded()
        }
    }

    override var preferredStatusBarStyle: UIStatusBarStyle {
        .darkContent
    }

    deinit {
        MainActor.assumeIsolated {
            invalidateObservations()
            attachRetryWorkItem?.cancel()
        }
    }

    fileprivate func closeManagedTabIfNeeded() {
        guard !didCloseManagedTab else { return }
        didCloseManagedTab = true

        attachRetryWorkItem?.cancel()
        attachRetryWorkItem = nil
        invalidateObservations()

        if let webView = activeBrowserWebView {
            webView.removeFromSuperview()
            webView.pauseWebView()
        }
        activeBrowserWebView = nil
        backEdgePanGesture?.isEnabled = false

        _ = browserTabClose(tabId)
        LxAppBrowserOverlay.browserControllerDidClose(self)
        os_log("Closed browser tab=%{public}@", log: Self.log, type: .info, tabId)
    }

    private func setupUI() {
        // Content container - full screen
        contentContainer.translatesAutoresizingMaskIntoConstraints = false
        contentContainer.backgroundColor = .white
        contentContainer.clipsToBounds = true
        view.addSubview(contentContainer)

        // Bottom bar
        bottomBar.translatesAutoresizingMaskIntoConstraints = false
        bottomBar.backgroundColor = .clear
        view.addSubview(bottomBar)

        bottomBarBackground.translatesAutoresizingMaskIntoConstraints = false
        bottomBarBackground.clipsToBounds = true
        bottomBarBackground.layer.cornerRadius = 16
        bottomBar.addSubview(bottomBarBackground)

        let borderLine = UIView()
        borderLine.translatesAutoresizingMaskIntoConstraints = false
        borderLine.backgroundColor = UIColor(red: 0.88, green: 0.88, blue: 0.88, alpha: 1.0)
        bottomBarBackground.contentView.addSubview(borderLine)

        // Main control row
        let controlRow = UIStackView()
        controlRow.translatesAutoresizingMaskIntoConstraints = false
        controlRow.axis = .horizontal
        controlRow.alignment = .center
        controlRow.spacing = 8
        bottomBarBackground.contentView.addSubview(controlRow)

        // Navigation buttons group (back + forward)
        let navButtonGroup = UIStackView()
        navButtonGroup.translatesAutoresizingMaskIntoConstraints = false
        navButtonGroup.axis = .horizontal
        navButtonGroup.alignment = .center
        navButtonGroup.spacing = -6
        controlRow.addArrangedSubview(navButtonGroup)

        // Back button
        configureIconButton(backButton, iconName: "icon_back", iconSize: 20, tintColor: UIColor(white: 0.2, alpha: 1.0), action: #selector(backTapped))
        backButton.isEnabled = false
        backButton.alpha = 0.3
        navButtonGroup.addArrangedSubview(backButton)

        // Forward button
        configureIconButton(forwardButton, iconName: "icon_forward", iconSize: 20, tintColor: UIColor(white: 0.2, alpha: 1.0), action: #selector(forwardTapped))
        forwardButton.isEnabled = false
        forwardButton.alpha = 0.3
        navButtonGroup.addArrangedSubview(forwardButton)

        // Address pill (flexible)
        addressPill.translatesAutoresizingMaskIntoConstraints = false
        addressPill.backgroundColor = UIColor(red: 0.94, green: 0.94, blue: 0.94, alpha: 1.0)
        addressPill.layer.cornerRadius = 18
        addressPill.clipsToBounds = true
        controlRow.addArrangedSubview(addressPill)

        addressField.translatesAutoresizingMaskIntoConstraints = false
        addressField.font = UIFont.systemFont(ofSize: 13)
        addressField.textColor = UIColor(red: 0.2, green: 0.2, blue: 0.2, alpha: 1.0)
        addressField.autocapitalizationType = .none
        addressField.autocorrectionType = .no
        addressField.clearButtonMode = .whileEditing
        addressField.keyboardType = .webSearch
        addressField.returnKeyType = .go
        addressField.delegate = self
        addressPill.addSubview(addressField)

        configureIconButton(refreshButton, iconName: "icon_browser_refresh", iconSize: 16, tintColor: UIColor(white: 0.4, alpha: 1.0), action: #selector(refreshTapped))
        refreshButton.widthAnchor.constraint(equalToConstant: 32).isActive = true
        refreshButton.heightAnchor.constraint(equalToConstant: 32).isActive = true
        addressPill.addSubview(refreshButton)

        // Close button
        configureIconButton(closeButton, iconName: "icon_close_x", iconSize: 20, tintColor: UIColor(white: 0.2, alpha: 1.0), action: #selector(closeTapped))
        controlRow.addArrangedSubview(closeButton)

        NSLayoutConstraint.activate([
            // Content container - from status bar bottom to bottom bar
            contentContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            contentContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            contentContainer.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            contentContainer.bottomAnchor.constraint(equalTo: bottomBar.topAnchor),

            // Bottom bar
            bottomBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bottomBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            bottomBar.heightAnchor.constraint(equalToConstant: 48),
            {
                let c = bottomBar.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -4)
                bottomBarBottomConstraint = c
                return c
            }(),

            bottomBarBackground.leadingAnchor.constraint(equalTo: bottomBar.leadingAnchor, constant: 12),
            bottomBarBackground.trailingAnchor.constraint(equalTo: bottomBar.trailingAnchor, constant: -12),
            bottomBarBackground.topAnchor.constraint(equalTo: bottomBar.topAnchor),
            bottomBarBackground.bottomAnchor.constraint(equalTo: bottomBar.bottomAnchor),

            borderLine.leadingAnchor.constraint(equalTo: bottomBarBackground.contentView.leadingAnchor),
            borderLine.trailingAnchor.constraint(equalTo: bottomBarBackground.contentView.trailingAnchor),
            borderLine.topAnchor.constraint(equalTo: bottomBarBackground.contentView.topAnchor),
            borderLine.heightAnchor.constraint(equalToConstant: 0.5),

            controlRow.leadingAnchor.constraint(equalTo: bottomBarBackground.contentView.leadingAnchor, constant: 8),
            controlRow.trailingAnchor.constraint(equalTo: bottomBarBackground.contentView.trailingAnchor, constant: -8),
            controlRow.topAnchor.constraint(equalTo: bottomBarBackground.contentView.topAnchor),
            controlRow.bottomAnchor.constraint(equalTo: bottomBarBackground.contentView.bottomAnchor),

            // Address pill (flexible width)
            addressPill.heightAnchor.constraint(equalToConstant: 36),

            addressField.leadingAnchor.constraint(equalTo: addressPill.leadingAnchor, constant: 12),
            addressField.trailingAnchor.constraint(equalTo: refreshButton.leadingAnchor, constant: -4),
            addressField.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),

            refreshButton.trailingAnchor.constraint(equalTo: addressPill.trailingAnchor, constant: -4),
            refreshButton.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),
        ])
    }

    private func setupBackGestureRecognizer() {
        let edgePan = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(handleBackEdgePan(_:)))
        edgePan.edges = .left
        edgePan.delegate = self
        edgePan.requiresExclusiveTouchType = false
        edgePan.name = "LxAppBrowserBackEdgePan"
        view.addGestureRecognizer(edgePan)
        backEdgePanGesture = edgePan
    }

    private func attachManagedWebViewIfNeeded(attempt: Int = 0) {
        attachRetryWorkItem?.cancel()
        attachRetryWorkItem = nil

        if let webView = findManagedBrowserWebView() {
            if activeBrowserWebView !== webView || webView.superview !== contentContainer {
                invalidateObservations()
                activeBrowserWebView?.removeFromSuperview()
                activeBrowserWebView = webView
                WebViewManager.configureWebViewTransparency(webView, transparent: false)
                WebViewManager.attachWebViewToContainer(webView, container: contentContainer)
                observeManagedWebView(webView)
            }

            updateAddressBar(url: webView.url)
            updateNavigationButtons()
            return
        }

        guard attempt < Self.maxAttachRetries else {
            os_log("Failed to attach browser webview for tab=%{public}@", log: Self.log, type: .error, tabId)
            return
        }

        let workItem = DispatchWorkItem { [weak self] in
            self?.attachManagedWebViewIfNeeded(attempt: attempt + 1)
        }
        attachRetryWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.attachRetryDelay, execute: workItem)
    }

    private func findManagedBrowserWebView() -> WKWebView? {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else {
            return nil
        }
        return WebViewManager.findWebView(
            appId: appId,
            path: browserTabPathForId(tabId).toString(),
            sessionId: sessionId
        )
    }

    private func observeManagedWebView(_ webView: WKWebView) {
        urlObservation = webView.observe(\.url, options: [.initial, .new]) { [weak self] webView, _ in
            Task { @MainActor in
                self?.updateAddressBar(url: webView.url)
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

    private func updateAddressBar(url: URL?) {
        guard !addressField.isFirstResponder else { return }
        addressField.text = url?.absoluteString ?? ""
    }

    private func updateNavigationButtons() {
        let canGoBack = activeBrowserWebView?.canGoBack ?? false
        backButton.isEnabled = canGoBack
        backButton.alpha = canGoBack ? 1.0 : 0.3

        let canGoForward = activeBrowserWebView?.canGoForward ?? false
        forwardButton.isEnabled = canGoForward
        forwardButton.alpha = canGoForward ? 1.0 : 0.3
    }

    private func submitAddressField() {
        guard let result = handleBrowserAddressSubmission(
            rawInput: addressField.text ?? "",
            currentURL: activeBrowserWebView?.url?.absoluteString,
            tabId: tabId
        ),
        let url = URL(string: result.url) else { return }
        addressField.text = result.displayText
        addressField.resignFirstResponder()
        activeBrowserWebView?.load(URLRequest(url: url))
    }

    private func configureIconButton(
        _ button: UIButton,
        iconName: String,
        iconSize: CGFloat,
        tintColor: UIColor,
        action: Selector
    ) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.tintColor = tintColor
        button.addTarget(self, action: action, for: .touchUpInside)

        #if SWIFT_PACKAGE
        let bundle = Bundle.module
        #else
        let bundle = Bundle(for: LxAppBrowserViewController.self)
        #endif
        if let pdfURL = bundle.url(forResource: iconName, withExtension: "pdf", subdirectory: "icons"),
           let provider = CGDataProvider(url: pdfURL as CFURL),
           let document = CGPDFDocument(provider),
           let page = document.page(at: 1) {
            let pageRect = page.getBoxRect(.mediaBox)
            let targetSize = CGSize(width: iconSize, height: iconSize)
            let renderer = UIGraphicsImageRenderer(size: targetSize)
            let image = renderer.image { ctx in
                ctx.cgContext.translateBy(x: 0, y: targetSize.height)
                ctx.cgContext.scaleBy(
                    x: targetSize.width / pageRect.width,
                    y: -targetSize.height / pageRect.height
                )
                ctx.cgContext.drawPDFPage(page)
            }
            button.setImage(image.withRenderingMode(.alwaysTemplate), for: .normal)
        }

        // Default button size (can be overridden after calling this method)
        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: 40),
            button.heightAnchor.constraint(equalToConstant: 40),
        ])
    }

    @objc private func closeTapped() {
        if let navigationController {
            navigationController.popViewController(animated: true)
        } else {
            closeManagedTabIfNeeded()
            dismiss(animated: true)
        }
    }

    @objc private func backTapped() {
        guard let webView = activeBrowserWebView, webView.canGoBack else { return }
        webView.goBack()
    }

    @objc private func forwardTapped() {
        guard let webView = activeBrowserWebView, webView.canGoForward else { return }
        webView.goForward()
    }

    @objc private func refreshTapped() {
        activeBrowserWebView?.reload()
    }

    @objc private func handleBackEdgePan(_ gesture: UIScreenEdgePanGestureRecognizer) {
        guard let containerView = gesture.view else { return }

        switch gesture.state {
        case .ended:
            let translation = gesture.translation(in: containerView).x
            let velocity = gesture.velocity(in: containerView).x
            let threshold = max(containerView.bounds.width * 0.2, 80)
            if translation > threshold || velocity > 700 {
                navigationController?.popViewController(animated: true)
            }
        default:
            break
        }
    }

    @objc private func keyboardWillShow(_ notification: Notification) {
        guard let keyboardFrame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect,
              let duration = notification.userInfo?[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double,
              let curve = notification.userInfo?[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt else { return }
        let keyboardHeight = keyboardFrame.height - view.safeAreaInsets.bottom
        bottomBarBottomConstraint?.constant = -(keyboardHeight + 4)
        UIView.animate(withDuration: duration, delay: 0, options: UIView.AnimationOptions(rawValue: curve << 16)) {
            self.view.layoutIfNeeded()
        }
    }

    @objc private func keyboardWillHide(_ notification: Notification) {
        guard let duration = notification.userInfo?[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double,
              let curve = notification.userInfo?[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt else { return }
        bottomBarBottomConstraint?.constant = -4
        UIView.animate(withDuration: duration, delay: 0, options: UIView.AnimationOptions(rawValue: curve << 16)) {
            self.view.layoutIfNeeded()
        }
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        submitAddressField()
        return false
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        gestureRecognizer === backEdgePanGesture
    }
}
#endif
