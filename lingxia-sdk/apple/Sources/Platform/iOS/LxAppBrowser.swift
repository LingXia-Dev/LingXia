#if os(iOS)
import UIKit
import WebKit
import OSLog
import CLingXiaRustAPI

/// Tab manager for the in-app browser. Owns a single persistent view
/// controller and the ordered set of open browser tabs; switching tabs swaps
/// the displayed managed webview instead of pushing a new screen.
@MainActor
final class LxAppBrowser: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "Browser")
    private static var currentController: LxAppBrowserViewController?

    private(set) static var openTabIds: [String] = []
    private(set) static var activeTabId: String?
    /// Tabs the user has interacted with (page tap or address navigation).
    /// Until then, auto-created history (SPA pushState redirects) must not
    /// light back/forward — mirroring Chrome's history intervention.
    static var interactedTabIds: Set<String> = []

    /// Show the browser displaying `tabId`. Creates and pushes the controller on
    /// first use; afterwards it only registers and activates the tab in place.
    @discardableResult
    static func show(tabId: String) -> Bool {
        let normalizedTabId = normalizeTabId(tabId)
        guard !normalizedTabId.isEmpty else {
            os_log("show failed: empty tab id", log: log, type: .error)
            return false
        }

        register(tabId: normalizedTabId)
        activeTabId = normalizedTabId

        if let controller = currentController,
           controller.navigationController?.topViewController === controller {
            browserTabActivate(normalizedTabId)
            controller.displayActiveTab()
            return true
        }

        guard let manager = iOSLxApp.getInstance().currentLxAppManager,
              let navController = manager.navigationController else {
            os_log("show failed: no active navigation controller", log: log, type: .error)
            return false
        }

        let controller = LxAppBrowserViewController()
        navController.pushViewController(controller, animated: true)
        currentController = controller
        browserTabActivate(normalizedTabId)
        os_log("Browser view controller pushed for tab=%{public}@", log: log, type: .info, normalizedTabId)
        return true
    }

    /// Open a fresh built-in start-page tab and switch to it.
    @discardableResult
    static func openNewTab() -> Bool {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else {
            os_log("openNewTab failed: no session for builtin browser", log: log, type: .error)
            return false
        }
        guard let newId = openBrowserTab(appId, sessionId, "lingxia://newtab")?.toString(),
              !normalizeTabId(newId).isEmpty else {
            os_log("openNewTab failed: runtime returned no tab id", log: log, type: .error)
            return false
        }
        let normalizedTabId = normalizeTabId(newId)
        register(tabId: normalizedTabId)
        activate(tabId: normalizedTabId)
        return true
    }

    /// Make an already-open tab the active, displayed one.
    static func activate(tabId: String) {
        let normalizedTabId = normalizeTabId(tabId)
        guard openTabIds.contains(normalizedTabId) else { return }
        activeTabId = normalizedTabId
        browserTabActivate(normalizedTabId)
        currentController?.displayActiveTab()
    }

    /// Close a single tab and move focus to a neighbor; exiting the browser when
    /// the last tab goes away.
    static func closeTab(tabId: String) {
        let normalizedTabId = normalizeTabId(tabId)
        guard let index = openTabIds.firstIndex(of: normalizedTabId) else { return }

        _ = browserTabClose(normalizedTabId)
        currentController?.releaseWebView(forTabId: normalizedTabId)
        openTabIds.remove(at: index)
        interactedTabIds.remove(normalizedTabId)

        let wasActive = activeTabId == normalizedTabId
        if !wasActive { return }

        if openTabIds.isEmpty {
            activeTabId = nil
            exitBrowser()
            return
        }

        let neighborIndex = index > 0 ? index - 1 : 0
        activate(tabId: openTabIds[neighborIndex])
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

    private static func register(tabId: String) {
        if !openTabIds.contains(tabId) {
            openTabIds.append(tabId)
        }
    }

    private static func normalizeTabId(_ tabId: String) -> String {
        tabId.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Pop the browser controller, tearing the whole session down.
    private static func exitBrowser() {
        guard let controller = currentController else { return }
        if controller.navigationController?.topViewController === controller {
            controller.navigationController?.popViewController(animated: true)
        } else {
            controller.closeManagedTabIfNeeded()
        }
    }

    fileprivate static func clearState() {
        openTabIds.removeAll()
        interactedTabIds.removeAll()
        activeTabId = nil
    }

    fileprivate static func browserControllerDidClose(_ controller: LxAppBrowserViewController) {
        if currentController === controller {
            currentController = nil
        }
    }
}

@MainActor
private final class LxAppBrowserViewController: UIViewController, UIGestureRecognizerDelegate, UITextFieldDelegate {
    private static let log = OSLog(subsystem: "LingXia", category: "BrowserViewController")
    private static let attachRetryDelay: TimeInterval = 0.1
    private static let maxAttachRetries = 8

    // Address row
    private let addressPill = UIView()
    private let addressIcon = UIImageView()
    private let addressField = UITextField()
    private let refreshButton = UIButton(type: .system)

    // Action row
    private let backButton = UIButton(type: .system)
    private let forwardButton = UIButton(type: .system)
    /// Row refresh for aside tabs (self tabs refresh from the address pill).
    private let asideRefreshButton = UIButton(type: .system)
    private let newTabButton = UIButton(type: .system)
    private let tabsButton = UIButton(type: .system)
    private let tabsBadge = UILabel()
    private let menuButton = UIButton(type: .system)
    private let closeButton = UIButton(type: .system)

    private let contentContainer = UIView()
    private let statusBarChrome = UIVisualEffectView(effect: UIBlurEffect(style: .systemThinMaterial))
    private let bottomBar = UIView()
    private let bottomBarBackground = UIVisualEffectView(effect: UIBlurEffect(style: .systemThinMaterial))

    private var bottomBarBottomConstraint: NSLayoutConstraint?
    // Action-row top: below the address pill normally, at the bar top for an
    // aside tab (address row hidden).
    private var actionRowTopWithAddress: NSLayoutConstraint?
    private var actionRowTopWithoutAddress: NSLayoutConstraint?

    // The webview displayed for the active tab, plus the tab id it belongs to.
    private var activeBrowserWebView: WKWebView?
    private var activeWebViewTabId: String?

    private var urlObservation: NSKeyValueObservation?
    private var canGoBackObservation: NSKeyValueObservation?
    private var canGoForwardObservation: NSKeyValueObservation?
    private var attachRetryWorkItem: DispatchWorkItem?
    private var didCloseManagedTab = false
    private var backEdgePanGesture: UIScreenEdgePanGestureRecognizer?
    private var interactionTapGesture: UITapGestureRecognizer?

    init() {
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
        observeKeyboard()
        updateTabsBadge()
        attachManagedWebViewIfNeeded()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        navigationController?.setNavigationBarHidden(true, animated: false)
        navigationController?.interactivePopGestureRecognizer?.isEnabled = true
        navigationController?.interactivePopGestureRecognizer?.delegate = nil
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
            NotificationCenter.default.removeObserver(self)
        }
    }

    /// Tear down every managed tab and reset the manager when the browser exits.
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
        activeWebViewTabId = nil
        backEdgePanGesture?.isEnabled = false

        for tabId in LxAppBrowser.openTabIds {
            _ = browserTabClose(tabId)
        }
        LxAppBrowser.clearState()
        LxAppBrowser.browserControllerDidClose(self)
        os_log("Closed all managed browser tabs", log: Self.log, type: .info)
    }

    // MARK: - Tab display

    /// Swap the attached webview to the manager's currently active tab.
    fileprivate func displayActiveTab() {
        updateTabsBadge()
        // Blank the address and back/forward until the new tab's webview
        // attaches — never show the previous tab's state (they are per-tab).
        if activeWebViewTabId != LxAppBrowser.activeTabId {
            addressField.text = ""
            addressIcon.isHidden = true
            NavButtonState.apply(backButton, enabled: false)
            NavButtonState.apply(forwardButton, enabled: false)
        }
        attachManagedWebViewIfNeeded()
    }

    /// Detach (without closing) the webview if it belongs to a tab being removed.
    fileprivate func releaseWebView(forTabId tabId: String) {
        guard activeWebViewTabId == tabId else { return }
        invalidateObservations()
        activeBrowserWebView?.removeFromSuperview()
        activeBrowserWebView = nil
        activeWebViewTabId = nil
    }

    private func setupUI() {
        // Content container — fills from the top safe area down to the bar. Page
        // content stays below the status bar: websites can't inset for the notch
        // the way our own lxapp pages can, so the system status area must stay clear.
        contentContainer.translatesAutoresizingMaskIntoConstraints = false
        contentContainer.backgroundColor = .white
        contentContainer.clipsToBounds = true
        view.addSubview(contentContainer)

        // Status-bar chrome: a translucent strip behind the system status bar so it
        // reads cleanly (matching the bottom bar) instead of a bare white gap.
        statusBarChrome.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(statusBarChrome)

        // Bottom bar — full-width material panel pinned to the bottom safe area,
        // carrying the address row over the action row.
        bottomBar.translatesAutoresizingMaskIntoConstraints = false
        bottomBar.backgroundColor = .clear
        view.addSubview(bottomBar)

        bottomBarBackground.translatesAutoresizingMaskIntoConstraints = false
        bottomBarBackground.clipsToBounds = true
        bottomBar.addSubview(bottomBarBackground)

        let borderLine = UIView()
        borderLine.translatesAutoresizingMaskIntoConstraints = false
        borderLine.backgroundColor = UIColor(red: 0.88, green: 0.88, blue: 0.88, alpha: 1.0)
        bottomBarBackground.contentView.addSubview(borderLine)

        let barContent = bottomBarBackground.contentView

        // Address row: the editable URL pill.
        addressPill.translatesAutoresizingMaskIntoConstraints = false
        addressPill.backgroundColor = UIColor(red: 0.94, green: 0.94, blue: 0.94, alpha: 1.0)
        addressPill.layer.cornerRadius = 18
        addressPill.clipsToBounds = true
        barContent.addSubview(addressPill)

        addressIcon.translatesAutoresizingMaskIntoConstraints = false
        addressIcon.contentMode = .scaleAspectFit
        addressIcon.tintColor = UIColor(white: 0.4, alpha: 1.0)
        addressPill.addSubview(addressIcon)

        addressField.translatesAutoresizingMaskIntoConstraints = false
        addressField.font = UIFont.systemFont(ofSize: 13)
        addressField.textColor = UIColor(red: 0.2, green: 0.2, blue: 0.2, alpha: 1.0)
        addressField.borderStyle = .none
        addressField.delegate = self
        addressField.keyboardType = .URL
        addressField.autocapitalizationType = .none
        addressField.autocorrectionType = .no
        addressField.returnKeyType = .go
        addressField.clearButtonMode = .whileEditing
        addressField.placeholder = "Search or enter address"
        addressPill.addSubview(addressField)

        configureIconButton(refreshButton, iconName: "icon_browser_refresh", iconSize: 16, tintColor: UIColor(white: 0.4, alpha: 1.0), action: #selector(refreshTapped))
        refreshButton.widthAnchor.constraint(equalToConstant: 32).isActive = true
        refreshButton.heightAnchor.constraint(equalToConstant: 32).isActive = true
        addressPill.addSubview(refreshButton)

        // Action row: back / forward — spacer — tabs / menu / close.
        let actionRow = UIStackView()
        actionRow.translatesAutoresizingMaskIntoConstraints = false
        actionRow.axis = .horizontal
        actionRow.alignment = .center
        actionRow.spacing = 8
        barContent.addSubview(actionRow)

        configureIconButton(backButton, iconName: "icon_back", iconSize: 20, tintColor: UIColor(white: 0.2, alpha: 1.0), action: #selector(backTapped))
        NavButtonState.apply(backButton, enabled: false)
        actionRow.addArrangedSubview(backButton)

        configureIconButton(forwardButton, iconName: "icon_forward", iconSize: 20, tintColor: UIColor(white: 0.2, alpha: 1.0), action: #selector(forwardTapped))
        NavButtonState.apply(forwardButton, enabled: false)
        actionRow.addArrangedSubview(forwardButton)

        configureIconButton(asideRefreshButton, iconName: "icon_browser_refresh", iconSize: 18, tintColor: UIColor(white: 0.2, alpha: 1.0), action: #selector(refreshTapped))
        asideRefreshButton.isHidden = true
        actionRow.addArrangedSubview(asideRefreshButton)

        let spacer = UIView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        actionRow.addArrangedSubview(spacer)

        // New-tab button — its own affordance, not buried in the tab switcher.
        newTabButton.translatesAutoresizingMaskIntoConstraints = false
        newTabButton.tintColor = UIColor(white: 0.2, alpha: 1.0)
        newTabButton.setImage(iconImage(named: "icon_plus", size: 20)?.withRenderingMode(.alwaysTemplate), for: .normal)
        newTabButton.addTarget(self, action: #selector(newTabTapped), for: .touchUpInside)
        NSLayoutConstraint.activate([
            newTabButton.widthAnchor.constraint(equalToConstant: 40),
            newTabButton.heightAnchor.constraint(equalToConstant: 36),
        ])
        actionRow.addArrangedSubview(newTabButton)

        // Tabs button with an overlaid open-tab count.
        setupTabsButton()
        actionRow.addArrangedSubview(tabsButton)

        // Menu (hamburger) with downloads + settings.
        menuButton.translatesAutoresizingMaskIntoConstraints = false
        menuButton.tintColor = UIColor(white: 0.2, alpha: 1.0)
        menuButton.setImage(iconImage(named: "icon_menu", size: 20)?.withRenderingMode(.alwaysTemplate), for: .normal)
        menuButton.showsMenuAsPrimaryAction = true
        menuButton.menu = makeOverflowMenu()
        NSLayoutConstraint.activate([
            menuButton.widthAnchor.constraint(equalToConstant: 40),
            menuButton.heightAnchor.constraint(equalToConstant: 36),
        ])
        actionRow.addArrangedSubview(menuButton)

        configureIconButton(closeButton, iconName: "icon_close_x", iconSize: 20, tintColor: UIColor(white: 0.2, alpha: 1.0), action: #selector(closeTapped))
        actionRow.addArrangedSubview(closeButton)

        // Anchor to the true bottom; the controls keep only a small home-indicator
        // clearance (below), so the bar hugs the bottom instead of leaving the full
        // safe-area gap. The blur fills down to the edge.
        let barBottom = bottomBar.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        bottomBarBottomConstraint = barBottom

        NSLayoutConstraint.activate([
            // Bottom bar
            bottomBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bottomBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            barBottom,

            bottomBarBackground.leadingAnchor.constraint(equalTo: bottomBar.leadingAnchor),
            bottomBarBackground.trailingAnchor.constraint(equalTo: bottomBar.trailingAnchor),
            bottomBarBackground.topAnchor.constraint(equalTo: bottomBar.topAnchor),
            // Extend the bar material behind the home indicator so the bottom edge
            // blends instead of exposing a strip of page content.
            bottomBarBackground.bottomAnchor.constraint(equalTo: view.bottomAnchor),

            borderLine.leadingAnchor.constraint(equalTo: barContent.leadingAnchor),
            borderLine.trailingAnchor.constraint(equalTo: barContent.trailingAnchor),
            borderLine.topAnchor.constraint(equalTo: barContent.topAnchor),
            borderLine.heightAnchor.constraint(equalToConstant: 0.5),

            // Address row
            addressPill.leadingAnchor.constraint(equalTo: barContent.leadingAnchor, constant: 12),
            addressPill.trailingAnchor.constraint(equalTo: barContent.trailingAnchor, constant: -12),
            addressPill.topAnchor.constraint(equalTo: barContent.topAnchor, constant: 6),
            addressPill.heightAnchor.constraint(equalToConstant: 34),

            addressIcon.leadingAnchor.constraint(equalTo: addressPill.leadingAnchor, constant: 12),
            addressIcon.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),
            addressIcon.widthAnchor.constraint(equalToConstant: 16),
            addressIcon.heightAnchor.constraint(equalToConstant: 16),

            addressField.leadingAnchor.constraint(equalTo: addressIcon.trailingAnchor, constant: 6),
            addressField.trailingAnchor.constraint(equalTo: refreshButton.leadingAnchor, constant: -4),
            addressField.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),

            refreshButton.trailingAnchor.constraint(equalTo: addressPill.trailingAnchor, constant: -4),
            refreshButton.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),

            // Action row
            actionRow.leadingAnchor.constraint(equalTo: barContent.leadingAnchor, constant: 8),
            actionRow.trailingAnchor.constraint(equalTo: barContent.trailingAnchor, constant: -8),
            // Sit close to the home indicator; the blur extends below it.
            actionRow.bottomAnchor.constraint(equalTo: bottomBar.bottomAnchor, constant: -10),

            // Content container — top safe area down to the bar's top.
            contentContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            contentContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            statusBarChrome.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            statusBarChrome.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            statusBarChrome.topAnchor.constraint(equalTo: view.topAnchor),
            statusBarChrome.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),

            contentContainer.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            contentContainer.bottomAnchor.constraint(equalTo: bottomBar.topAnchor),
        ])

        let withAddress = actionRow.topAnchor.constraint(equalTo: addressPill.bottomAnchor, constant: 4)
        let withoutAddress = actionRow.topAnchor.constraint(equalTo: barContent.topAnchor, constant: 6)
        actionRowTopWithAddress = withAddress
        actionRowTopWithoutAddress = withoutAddress
        withAddress.isActive = true
    }

    /// Aside chrome: refresh, smart back/forward, tabs, close — no address
    /// row, no new-tab, no menu (asides are API-opened; user-created tabs
    /// are self mode).
    private func updateAddressRowVisibility() {
        let aside = LxAppBrowser.activeTabId.map { browserTabIsAside($0) } ?? false
        newTabButton.isHidden = aside
        menuButton.isHidden = aside
        asideRefreshButton.isHidden = !aside
        guard addressPill.isHidden != aside else { return }
        addressPill.isHidden = aside
        if aside {
            actionRowTopWithAddress?.isActive = false
            actionRowTopWithoutAddress?.isActive = true
        } else {
            actionRowTopWithoutAddress?.isActive = false
            actionRowTopWithAddress?.isActive = true
        }
    }

    private func setupTabsButton() {
        tabsButton.translatesAutoresizingMaskIntoConstraints = false
        tabsButton.tintColor = UIColor(white: 0.2, alpha: 1.0)
        tabsButton.setImage(iconImage(named: "icon_tabs", size: 20)?.withRenderingMode(.alwaysTemplate), for: .normal)
        tabsButton.addTarget(self, action: #selector(tabsTapped), for: .touchUpInside)
        NSLayoutConstraint.activate([
            tabsButton.widthAnchor.constraint(equalToConstant: 40),
            tabsButton.heightAnchor.constraint(equalToConstant: 36),
        ])

        tabsBadge.translatesAutoresizingMaskIntoConstraints = false
        tabsBadge.font = UIFont.systemFont(ofSize: 10, weight: .semibold)
        tabsBadge.textColor = UIColor(white: 0.2, alpha: 1.0)
        tabsBadge.textAlignment = .center
        tabsButton.addSubview(tabsBadge)
        NSLayoutConstraint.activate([
            tabsBadge.centerXAnchor.constraint(equalTo: tabsButton.centerXAnchor, constant: 2),
            tabsBadge.centerYAnchor.constraint(equalTo: tabsButton.centerYAnchor, constant: -2),
        ])
    }

    private func updateTabsBadge() {
        tabsBadge.text = String(LxAppBrowser.openTabIds.count)
    }

    private func makeOverflowMenu() -> UIMenu {
        let downloads = UIAction(
            title: "Downloads",
            image: iconImage(named: "icon_download", size: 18)?.withRenderingMode(.alwaysTemplate)
        ) { [weak self] _ in
            self?.navigateToInternalPage("lingxia://downloads")
        }
        let settings = UIAction(
            title: "Settings",
            image: iconImage(named: "icon_settings", size: 18)?.withRenderingMode(.alwaysTemplate)
        ) { [weak self] _ in
            self?.navigateToInternalPage("lingxia://settings")
        }
        return UIMenu(title: "", children: [downloads, settings])
    }

    /// Open one of the browser's own `lingxia://` pages in the active tab.
    private func navigateToInternalPage(_ url: String) {
        guard let tabId = LxAppBrowser.activeTabId else { return }
        _ = browserTabNavigate(tabId, url)
    }

    private func setupBackGestureRecognizer() {
        let edgePan = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(handleBackEdgePan(_:)))
        edgePan.edges = .left
        edgePan.delegate = self
        edgePan.requiresExclusiveTouchType = false
        edgePan.name = "LxAppBrowserBackEdgePan"
        view.addGestureRecognizer(edgePan)
        backEdgePanGesture = edgePan

        // First tap on the page marks the active tab as interacted
        // (observe-only; touches pass through to the webview).
        let tap = UITapGestureRecognizer(target: self, action: #selector(handleInteractionTap))
        tap.cancelsTouchesInView = false
        tap.delegate = self
        contentContainer.addGestureRecognizer(tap)
        interactionTapGesture = tap
    }

    @objc private func handleInteractionTap() {
        guard let tabId = LxAppBrowser.activeTabId,
              !LxAppBrowser.interactedTabIds.contains(tabId) else { return }
        LxAppBrowser.interactedTabIds.insert(tabId)
        updateNavigationButtons()
    }

    // MARK: - Keyboard avoidance

    private func observeKeyboard() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(keyboardWillShow(_:)),
            name: UIResponder.keyboardWillShowNotification,
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(keyboardWillHide(_:)),
            name: UIResponder.keyboardWillHideNotification,
            object: nil
        )
    }

    @objc private func keyboardWillShow(_ notification: Notification) {
        guard let frame = (notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue else { return }
        // The bar rests at the true bottom, so lift it by the full keyboard height.
        animateBar(offset: -frame.height, notification: notification)
    }

    @objc private func keyboardWillHide(_ notification: Notification) {
        animateBar(offset: 0, notification: notification)
    }

    private func animateBar(offset: CGFloat, notification: Notification) {
        bottomBarBottomConstraint?.constant = offset
        let duration = (notification.userInfo?[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double) ?? 0.25
        UIView.animate(withDuration: duration) {
            self.view.layoutIfNeeded()
        }
    }

    // MARK: - Webview attach

    private func attachManagedWebViewIfNeeded(attempt: Int = 0) {
        attachRetryWorkItem?.cancel()
        attachRetryWorkItem = nil

        guard let activeTabId = LxAppBrowser.activeTabId else { return }
        updateAddressRowVisibility()

        if let webView = findManagedBrowserWebView(tabId: activeTabId) {
            if activeBrowserWebView !== webView || activeWebViewTabId != activeTabId || webView.superview !== contentContainer {
                invalidateObservations()
                // Detach the previous tab's webview without closing it.
                if activeBrowserWebView !== webView {
                    activeBrowserWebView?.removeFromSuperview()
                }
                activeBrowserWebView = webView
                activeWebViewTabId = activeTabId
                WebViewManager.configureWebViewTransparency(webView, transparent: false)
                WebViewManager.attachWebViewToContainer(webView, container: contentContainer)
                observeManagedWebView(webView)
            }

            updateAddressBar(url: webView.url)
            updateNavigationButtons()
            return
        }

        guard attempt < Self.maxAttachRetries else {
            os_log("Failed to attach browser webview for tab=%{public}@", log: Self.log, type: .error, activeTabId)
            return
        }

        let workItem = DispatchWorkItem { [weak self] in
            self?.attachManagedWebViewIfNeeded(attempt: attempt + 1)
        }
        attachRetryWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.attachRetryDelay, execute: workItem)
    }

    private func findManagedBrowserWebView(tabId: String) -> WKWebView? {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else {
            return nil
        }
        return WebViewManager.resolveWebView(
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
        // A new/blank tab (lingxia://newtab) shows just the placeholder, like a
        // fresh browser tab — never the raw internal URL.
        if let url, browserUrlIsHidden(url.absoluteString) {
            addressField.text = ""
            addressIcon.isHidden = true
            return
        }
        addressIcon.isHidden = false
        let display = browserUrlDisplay(url: url)
        addressField.text = display.text
        addressIcon.image = iconImage(named: display.iconName, size: 16)?.withRenderingMode(.alwaysTemplate)
        addressIcon.tintColor = display.tintColor
    }

    private struct BrowserUrlDisplay {
        let text: String
        let iconName: String
        let tintColor: UIColor
    }

    private func browserUrlDisplay(url: URL?) -> BrowserUrlDisplay {
        guard let url else {
            return BrowserUrlDisplay(text: "", iconName: "icon_lock", tintColor: UIColor(white: 0.4, alpha: 1.0))
        }
        switch url.scheme?.lowercased() {
        case "lingxia":
            // The browser's own internal pages (newtab / settings / downloads):
            // show the full lingxia:// address, no security chrome.
            return BrowserUrlDisplay(
                text: url.absoluteString,
                iconName: "icon_lock",
                tintColor: UIColor(white: 0.4, alpha: 1.0)
            )
        case "https":
            return BrowserUrlDisplay(
                text: url.host?.isEmpty == false ? url.host! : "Web page",
                iconName: "icon_lock",
                tintColor: UIColor(white: 0.4, alpha: 1.0)
            )
        case "http":
            return BrowserUrlDisplay(
                text: url.host?.isEmpty == false ? url.host! : "Web page",
                iconName: "icon_warning",
                tintColor: UIColor(red: 0.63, green: 0.36, blue: 0.0, alpha: 1.0)
            )
        default:
            return BrowserUrlDisplay(
                text: "Web page",
                iconName: "icon_warning",
                tintColor: UIColor(red: 0.63, green: 0.36, blue: 0.0, alpha: 1.0)
            )
        }
    }

    private func iconImage(named iconName: String, size: CGFloat) -> UIImage? {
        lxAppBrowserIconImage(named: iconName, size: size)
    }

    private func updateNavigationButtons() {
        // Pre-interaction history is auto-created (redirects/pushState) and
        // must not light the affordances.
        let interacted = LxAppBrowser.activeTabId.map { LxAppBrowser.interactedTabIds.contains($0) } ?? false
        NavButtonState.apply(backButton, enabled: (activeBrowserWebView?.canGoBack ?? false) && interacted)
        NavButtonState.apply(forwardButton, enabled: (activeBrowserWebView?.canGoForward ?? false) && interacted)
    }

    private func configureIconButton(
        _ button: UIButton,
        iconName: String,
        iconSize: CGFloat,
        tintColor: UIColor,
        action: Selector?
    ) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.tintColor = tintColor
        if let action {
            button.addTarget(self, action: action, for: .touchUpInside)
        }

        if let image = iconImage(named: iconName, size: iconSize) {
            button.setImage(image.withRenderingMode(.alwaysTemplate), for: .normal)
        }

        // Default button size (can be overridden after calling this method)
        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: 40),
            button.heightAnchor.constraint(equalToConstant: 36),
        ])
    }

    // MARK: - Actions

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

    @objc private func tabsTapped() {
        let switcher = LxAppBrowserTabSwitcherViewController(host: self)
        if let sheet = switcher.sheetPresentationController {
            sheet.detents = [.medium(), .large()]
            sheet.prefersGrabberVisible = true
        }
        present(switcher, animated: true)
    }

    @objc private func newTabTapped() {
        _ = LxAppBrowser.openNewTab()
    }

    /// Resolve a tab's display label from its managed webview, falling back to a
    /// generic label so the switcher always has something to show.
    fileprivate func tabLabel(forTabId tabId: String) -> String {
        if let webView = findManagedBrowserWebView(tabId: tabId) {
            if let title = webView.title, !title.isEmpty {
                return title
            }
            if let host = webView.url?.host, !host.isEmpty {
                return host
            }
        }
        return "New Tab"
    }

    // MARK: - Address bar navigation

    func textFieldDidBeginEditing(_ textField: UITextField) {
        // Reveal the full URL for editing and select it for quick replacement,
        // except on a blank new tab, where there is nothing to reveal.
        if let full = activeBrowserWebView?.url?.absoluteString, !browserUrlIsHidden(full) {
            textField.text = full
        } else {
            textField.text = ""
        }
        addressIcon.isHidden = true
        DispatchQueue.main.async { textField.selectAll(nil) }
    }

    func textFieldDidEndEditing(_ textField: UITextField) {
        addressIcon.isHidden = false
        updateAddressBar(url: activeBrowserWebView?.url)
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        // Capture the input BEFORE resigning: resignFirstResponder fires
        // textFieldDidEndEditing synchronously, which rewrites the field to the
        // current page URL (empty for a hidden newtab) — reading text after that
        // would lose what the user typed.
        let input = textField.text ?? ""
        textField.resignFirstResponder()
        navigate(toInput: input)
        return true
    }

    /// Resolve a typed address: a full URL loads as-is, a bare domain gets https.
    /// Free-text search belongs to a configurable search provider or the start page,
    /// not native browser chrome.
    private func navigate(toInput input: String) {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let tabId = LxAppBrowser.activeTabId else { return }
        let target: URL?
        if let url = URL(string: trimmed),
           let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" || scheme == "lingxia" {
            target = url
        } else if !trimmed.contains(" "), trimmed.contains("."), let url = URL(string: "https://\(trimmed)") {
            target = url
        } else {
            target = nil
        }
        guard let target else {
            updateAddressBar(url: activeBrowserWebView?.url)
            return
        }
        if browserTabNavigate(tabId, target.absoluteString) {
            // An address-bar navigation is a user interaction.
            LxAppBrowser.interactedTabIds.insert(tabId)
            updateAddressBar(url: target)
            attachManagedWebViewIfNeeded()
        } else {
            updateAddressBar(url: activeBrowserWebView?.url)
        }
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

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        gestureRecognizer === backEdgePanGesture || gestureRecognizer === interactionTapGesture
    }
}

/// Modal sheet listing the open tabs as a single-column list, with per-row close
/// and a "+" affordance for opening a new tab.
@MainActor
private final class LxAppBrowserTabSwitcherViewController: UIViewController, UITableViewDataSource, UITableViewDelegate {
    private weak var host: LxAppBrowserViewController?
    private let tableView = UITableView(frame: .zero, style: .plain)
    private static let cellReuseId = "LxAppBrowserTabCell"

    init(host: LxAppBrowserViewController) {
        self.host = host
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .pageSheet
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        let header = UIView()
        header.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(header)

        let titleLabel = UILabel()
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.text = "Tabs"
        titleLabel.font = UIFont.systemFont(ofSize: 17, weight: .semibold)
        header.addSubview(titleLabel)

        let addButton = UIButton(type: .system)
        addButton.translatesAutoresizingMaskIntoConstraints = false
        addButton.setImage(lxAppBrowserIconImage(named: "icon_plus", size: 20)?.withRenderingMode(.alwaysTemplate), for: .normal)
        addButton.addTarget(self, action: #selector(newTabTapped), for: .touchUpInside)
        // New tabs are self mode; hide the affordance while an aside is active.
        addButton.isHidden = LxAppBrowser.activeTabId.map { browserTabIsAside($0) } ?? false
        header.addSubview(addButton)

        tableView.translatesAutoresizingMaskIntoConstraints = false
        tableView.dataSource = self
        tableView.delegate = self
        tableView.register(LxAppBrowserTabCell.self, forCellReuseIdentifier: Self.cellReuseId)
        tableView.rowHeight = 56
        view.addSubview(tableView)

        NSLayoutConstraint.activate([
            header.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            header.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            header.heightAnchor.constraint(equalToConstant: 52),

            titleLabel.leadingAnchor.constraint(equalTo: header.leadingAnchor, constant: 20),
            titleLabel.centerYAnchor.constraint(equalTo: header.centerYAnchor),

            addButton.trailingAnchor.constraint(equalTo: header.trailingAnchor, constant: -20),
            addButton.centerYAnchor.constraint(equalTo: header.centerYAnchor),
            addButton.widthAnchor.constraint(equalToConstant: 40),
            addButton.heightAnchor.constraint(equalToConstant: 36),

            tableView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            tableView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            tableView.topAnchor.constraint(equalTo: header.bottomAnchor),
            tableView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    @objc private func newTabTapped() {
        dismiss(animated: true) {
            LxAppBrowser.openNewTab()
        }
    }

    private func closeTab(at index: Int) {
        guard index < LxAppBrowser.openTabIds.count else { return }
        let tabId = LxAppBrowser.openTabIds[index]
        LxAppBrowser.closeTab(tabId: tabId)
        if LxAppBrowser.openTabIds.isEmpty {
            dismiss(animated: true)
        } else {
            tableView.reloadData()
        }
    }

    // MARK: - UITableViewDataSource / Delegate

    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        LxAppBrowser.openTabIds.count
    }

    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: Self.cellReuseId, for: indexPath) as! LxAppBrowserTabCell
        let tabId = LxAppBrowser.openTabIds[indexPath.row]
        let label = host?.tabLabel(forTabId: tabId) ?? tabId
        let isActive = LxAppBrowser.activeTabId == tabId
        cell.configure(title: label, isActive: isActive) { [weak self] in
            self?.closeTab(at: indexPath.row)
        }
        return cell
    }

    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: false)
        let tabId = LxAppBrowser.openTabIds[indexPath.row]
        dismiss(animated: true) {
            LxAppBrowser.activate(tabId: tabId)
        }
    }
}

@MainActor
private final class LxAppBrowserTabCell: UITableViewCell {
    private let titleLabel = UILabel()
    private let closeButton = UIButton(type: .system)
    private var onClose: (() -> Void)?

    override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
        super.init(style: style, reuseIdentifier: reuseIdentifier)

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = UIFont.systemFont(ofSize: 15)
        titleLabel.lineBreakMode = .byTruncatingTail
        contentView.addSubview(titleLabel)

        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.setImage(lxAppBrowserIconImage(named: "icon_close_x", size: 16)?.withRenderingMode(.alwaysTemplate), for: .normal)
        closeButton.tintColor = UIColor(white: 0.4, alpha: 1.0)
        closeButton.addTarget(self, action: #selector(closeTapped), for: .touchUpInside)
        contentView.addSubview(closeButton)

        NSLayoutConstraint.activate([
            titleLabel.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 20),
            titleLabel.trailingAnchor.constraint(equalTo: closeButton.leadingAnchor, constant: -12),
            titleLabel.centerYAnchor.constraint(equalTo: contentView.centerYAnchor),

            closeButton.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -16),
            closeButton.centerYAnchor.constraint(equalTo: contentView.centerYAnchor),
            closeButton.widthAnchor.constraint(equalToConstant: 36),
            closeButton.heightAnchor.constraint(equalToConstant: 36),
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func configure(title: String, isActive: Bool, onClose: @escaping () -> Void) {
        titleLabel.text = title
        titleLabel.font = UIFont.systemFont(ofSize: 15, weight: isActive ? .semibold : .regular)
        titleLabel.textColor = isActive ? UIColor(white: 0.1, alpha: 1.0) : UIColor(white: 0.3, alpha: 1.0)
        self.onClose = onClose
    }

    @objc private func closeTapped() {
        onClose?()
    }
}

/// Render a design-set PDF icon (under `Resources/icons`) at a square size.
/// Shared by the browser chrome and the tab switcher so both pull from the same
/// design icon set.
@MainActor
private func lxAppBrowserIconImage(named iconName: String, size: CGFloat) -> UIImage? {
    #if SWIFT_PACKAGE
    let bundle = Bundle.lingxiaResources
    #else
    let bundle = Bundle(for: LxAppBrowserViewController.self)
    #endif
    guard let pdfURL = bundle.url(forResource: iconName, withExtension: "pdf", subdirectory: "icons"),
          let provider = CGDataProvider(url: pdfURL as CFURL),
          let document = CGPDFDocument(provider),
          let page = document.page(at: 1) else { return nil }
    let pageRect = page.getBoxRect(.mediaBox)
    let targetSize = CGSize(width: size, height: size)
    let renderer = UIGraphicsImageRenderer(size: targetSize)
    return renderer.image { ctx in
        ctx.cgContext.translateBy(x: 0, y: targetSize.height)
        ctx.cgContext.scaleBy(x: targetSize.width / pageRect.width, y: -targetSize.height / pageRect.height)
        ctx.cgContext.drawPDFPage(page)
    }
}
#endif
