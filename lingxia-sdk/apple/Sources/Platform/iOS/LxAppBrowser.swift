#if os(iOS)
import UIKit
import WebKit
import OSLog
import CLingXiaRustAPI

/// Tab manager for the in-app browser. Self and aside tabs stay in separate
/// presentation groups even though they share one reusable controller.
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
            LXLog.error("show failed: empty tab id", category: "Browser")
            return false
        }

        register(tabId: normalizedTabId)
        activeTabId = normalizedTabId

        guard let manager = iOSLxApp.getInstance().currentLxAppManager,
              let navController = manager.navigationController else {
            LXLog.error("show failed: no active navigation controller", category: "Browser")
            return false
        }

        if let controller = currentController {
            if controller.navigationController?.topViewController !== controller {
                navController.pushViewController(controller, animated: true)
            }
            browserTabActivate(normalizedTabId)
            controller.displayActiveTab()
            return true
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
        if activeTabId.map({ browserTabIsAside($0) }) ?? false {
            return false
        }
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else {
            LXLog.error("openNewTab failed: no session for builtin browser", category: "Browser")
            return false
        }
        guard let newId = openBrowserTab(appId, sessionId, "lingxia://newtab")?.toString(),
              !normalizeTabId(newId).isEmpty else {
            LXLog.error("openNewTab failed: runtime returned no tab id", category: "Browser")
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
        guard openTabIds.contains(normalizedTabId),
              isAside(tabId: normalizedTabId) == activeTabIsAside else { return }
        activeTabId = normalizedTabId
        browserTabActivate(normalizedTabId)
        currentController?.displayActiveTab()
    }

    /// Close a tab and select a neighbor in its group; exit when that group is
    /// empty.
    static func closeTab(tabId: String) {
        let normalizedTabId = normalizeTabId(tabId)
        guard let index = openTabIds.firstIndex(of: normalizedTabId) else { return }
        let closingAside = isAside(tabId: normalizedTabId)
        let groupIndex = tabIds(aside: closingAside).firstIndex(of: normalizedTabId) ?? 0

        _ = browserTabClose(normalizedTabId)
        currentController?.releaseWebView(forTabId: normalizedTabId)
        openTabIds.remove(at: index)
        interactedTabIds.remove(normalizedTabId)

        let wasActive = activeTabId == normalizedTabId
        if !wasActive { return }

        let remaining = tabIds(aside: closingAside)
        if remaining.isEmpty {
            activeTabId = nil
            exitBrowser()
            return
        }

        let neighborIndex = min(groupIndex, remaining.count - 1)
        let neighbor = remaining[neighborIndex]
        activeTabId = neighbor
        browserTabActivate(neighbor)
        currentController?.displayActiveTab()
    }

    @objc public static func dismiss() {
        guard let controller = currentController else { return }

        if controller.navigationController?.topViewController === controller {
            controller.navigationController?.popViewController(animated: true)
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

    private static func isAside(tabId: String) -> Bool {
        browserTabIsAside(tabId)
    }

    fileprivate static var activeTabIsAside: Bool {
        activeTabId.map { isAside(tabId: $0) } ?? false
    }

    fileprivate static var visibleTabIds: [String] {
        guard activeTabId != nil else { return [] }
        return tabIds(aside: activeTabIsAside)
    }

    private static func tabIds(aside: Bool) -> [String] {
        openTabIds.filter { isAside(tabId: $0) == aside }
    }

    private static func normalizeTabId(_ tabId: String) -> String {
        tabId.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Pop the browser controller while keeping both tab groups alive.
    private static func exitBrowser() {
        guard let controller = currentController else { return }
        if controller.navigationController?.topViewController === controller {
            controller.navigationController?.popViewController(animated: true)
        }
    }

}

@MainActor
private final class LxAppBrowserViewController: UIViewController, UIGestureRecognizerDelegate, UITextFieldDelegate {
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
    private var actionRowTopWithAddress: NSLayoutConstraint?
    private var actionRowTopWithoutAddress: NSLayoutConstraint?

    // The webview displayed for the active tab, plus the tab id it belongs to.
    private var activeBrowserWebView: WKWebView?
    private var activeWebViewTabId: String?

    private var urlObservation: NSKeyValueObservation?
    private var canGoBackObservation: NSKeyValueObservation?
    private var canGoForwardObservation: NSKeyValueObservation?
    private var attachRetryWorkItem: DispatchWorkItem?
    private var pendingReloadTabId: String?
    private var backEdgePanGesture: UIScreenEdgePanGestureRecognizer?
    private var interactionTapGesture: UITapGestureRecognizer?

    // In-view overlays shown/hidden instantly — no system sheet/menu animation.
    private var tabSwitcherOverlay: LxAppBrowserTabSwitcherView?
    private var menuOverlay: UIView?

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
            suspendManagedWebView()
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

    private func suspendManagedWebView() {
        attachRetryWorkItem?.cancel()
        attachRetryWorkItem = nil
        pendingReloadTabId = nil
        invalidateObservations()
        activeBrowserWebView?.removeFromSuperview()
        activeBrowserWebView?.pauseWebView()
        activeBrowserWebView = nil
        activeWebViewTabId = nil
    }

    // MARK: - Tab display

    /// Swap the attached webview to the manager's currently active tab.
    fileprivate func displayActiveTab() {
        updateTabsBadge()
        if pendingReloadTabId != LxAppBrowser.activeTabId {
            pendingReloadTabId = nil
        }
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
        if pendingReloadTabId == tabId {
            pendingReloadTabId = nil
        }
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

        configureIconButton(
            refreshButton,
            iconName: "icon_browser_refresh",
            iconSize: 16,
            tintColor: UIColor(white: 0.4, alpha: 1.0),
            action: #selector(refreshTapped),
            buttonSize: CGSize(width: 32, height: 32)
        )
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

        // Menu (hamburger) with downloads + settings — a custom instant popup,
        // not UIMenu (its highlight flash + presentation animation).
        menuButton.translatesAutoresizingMaskIntoConstraints = false
        menuButton.tintColor = UIColor(white: 0.2, alpha: 1.0)
        menuButton.setImage(iconImage(named: "icon_menu", size: 20)?.withRenderingMode(.alwaysTemplate), for: .normal)
        menuButton.addTarget(self, action: #selector(menuTapped), for: .touchUpInside)
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

    /// Compact aside chrome is one row: history, refresh, tabs, and dismiss.
    /// Desktop's read-only aside address is a separate projection.
    private func applyActiveModeChrome() {
        let aside = LxAppBrowser.activeTabIsAside
        newTabButton.isHidden = aside
        menuButton.isHidden = aside
        asideRefreshButton.isHidden = !aside
        if aside, addressField.isFirstResponder {
            addressField.resignFirstResponder()
        }
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
        tabsBadge.text = String(LxAppBrowser.visibleTabIds.count)
    }

    @objc private func menuTapped() {
        if menuOverlay != nil {
            dismissOverflowMenu()
            return
        }
        dismissTabSwitcher()

        let scrim = UIControl()
        scrim.translatesAutoresizingMaskIntoConstraints = false
        scrim.addTarget(self, action: #selector(overflowScrimTapped), for: .touchUpInside)
        view.addSubview(scrim)

        let card = UIView()
        card.translatesAutoresizingMaskIntoConstraints = false
        card.backgroundColor = .white
        card.layer.cornerRadius = 12
        card.layer.shadowColor = UIColor.black.cgColor
        card.layer.shadowOpacity = 0.18
        card.layer.shadowRadius = 12
        card.layer.shadowOffset = CGSize(width: 0, height: 4)
        scrim.addSubview(card)

        let rows = UIStackView(arrangedSubviews: [
            overflowMenuRow(title: "Downloads", iconName: "icon_download", action: #selector(downloadsTapped)),
            overflowMenuRow(title: "Settings", iconName: "icon_settings", action: #selector(settingsTapped)),
        ])
        rows.translatesAutoresizingMaskIntoConstraints = false
        rows.axis = .vertical
        card.addSubview(rows)

        NSLayoutConstraint.activate([
            scrim.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scrim.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            scrim.topAnchor.constraint(equalTo: view.topAnchor),
            scrim.bottomAnchor.constraint(equalTo: view.bottomAnchor),

            card.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -12),
            card.bottomAnchor.constraint(equalTo: bottomBar.topAnchor, constant: -8),
            card.widthAnchor.constraint(equalToConstant: 200),

            rows.leadingAnchor.constraint(equalTo: card.leadingAnchor),
            rows.trailingAnchor.constraint(equalTo: card.trailingAnchor),
            rows.topAnchor.constraint(equalTo: card.topAnchor, constant: 6),
            rows.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -6),
        ])

        menuOverlay = scrim
    }

    private func overflowMenuRow(title: String, iconName: String, action: Selector) -> UIButton {
        var config = UIButton.Configuration.plain()
        config.title = title
        config.image = iconImage(named: iconName, size: 18)?.withRenderingMode(.alwaysTemplate)
        config.imagePadding = 12
        config.baseForegroundColor = UIColor(white: 0.2, alpha: 1.0)
        config.contentInsets = NSDirectionalEdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16)
        let button = UIButton(configuration: config)
        button.contentHorizontalAlignment = .leading
        button.addTarget(self, action: action, for: .touchUpInside)
        return button
    }

    @objc private func overflowScrimTapped() {
        dismissOverflowMenu()
    }

    @objc private func downloadsTapped() {
        dismissOverflowMenu()
        navigateToInternalPage("lingxia://downloads")
    }

    @objc private func settingsTapped() {
        dismissOverflowMenu()
        navigateToInternalPage("lingxia://settings")
    }

    private func dismissOverflowMenu() {
        menuOverlay?.removeFromSuperview()
        menuOverlay = nil
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
        applyActiveModeChrome()

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
            if pendingReloadTabId == activeTabId {
                pendingReloadTabId = nil
                webView.reload()
            }
            return
        }

        guard attempt < Self.maxAttachRetries else {
            if pendingReloadTabId == activeTabId {
                pendingReloadTabId = nil
            }
            LXLog.error("Failed to attach browser webview for tab=\(activeTabId)", category: "BrowserViewController")
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
        // URL KVO can fire repeatedly while a page redirects or a new tab is
        // attaching. Never replace text that the user is actively editing.
        guard !addressField.isFirstResponder else { return }
        // A new/blank tab (lingxia://newtab) shows just the placeholder, like a
        // fresh browser tab — never the raw internal URL.
        if let url, browserUrlIsHidden(url.absoluteString) {
            addressField.text = ""
            addressIcon.isHidden = true
            return
        }
        let display = browserUrlDisplay(url: url)
        addressField.text = display.text
        if let iconName = display.iconName {
            addressIcon.isHidden = false
            addressIcon.image = iconImage(named: iconName, size: 16)?.withRenderingMode(.alwaysTemplate)
            addressIcon.tintColor = display.tintColor
        } else {
            addressIcon.isHidden = true
        }
    }

    private struct BrowserUrlDisplay {
        let text: String
        // Secure is the norm — no padlock (misread as "locked"); only insecure
        // pages get an icon.
        let iconName: String?
        let tintColor: UIColor
    }

    private func browserUrlDisplay(url: URL?) -> BrowserUrlDisplay {
        guard let url else {
            return BrowserUrlDisplay(text: "", iconName: nil, tintColor: UIColor(white: 0.4, alpha: 1.0))
        }
        switch url.scheme?.lowercased() {
        case "lingxia":
            // The browser's own internal pages (newtab / settings / downloads):
            // show the full lingxia:// address, no security chrome.
            return BrowserUrlDisplay(
                text: url.absoluteString,
                iconName: nil,
                tintColor: UIColor(white: 0.4, alpha: 1.0)
            )
        case "https":
            return BrowserUrlDisplay(
                text: url.host?.isEmpty == false ? url.host! : "Web page",
                iconName: nil,
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
        action: Selector?,
        buttonSize: CGSize = CGSize(width: 40, height: 36)
    ) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.tintColor = tintColor
        if let action {
            button.addTarget(self, action: action, for: .touchUpInside)
        }

        if let image = iconImage(named: iconName, size: iconSize) {
            button.setImage(image.withRenderingMode(.alwaysTemplate), for: .normal)
        }

        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: buttonSize.width),
            button.heightAnchor.constraint(equalToConstant: buttonSize.height),
        ])
    }

    // MARK: - Actions

    @objc private func closeTapped() {
        if let navigationController {
            navigationController.popViewController(animated: true)
        } else {
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
        guard let tabId = LxAppBrowser.activeTabId else { return }
        if activeWebViewTabId == tabId, let webView = activeBrowserWebView {
            webView.reload()
            return
        }

        // A newly activated tab can receive the tap before its managed WebView
        // has attached. Preserve the action and apply it as soon as it appears.
        pendingReloadTabId = tabId
        attachManagedWebViewIfNeeded()
    }

    @objc private func tabsTapped() {
        if tabSwitcherOverlay != nil {
            dismissTabSwitcher()
            return
        }
        dismissOverflowMenu()
        let switcher = LxAppBrowserTabSwitcherView(host: self)
        view.addSubview(switcher)
        NSLayoutConstraint.activate([
            switcher.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            switcher.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            switcher.topAnchor.constraint(equalTo: view.topAnchor),
            switcher.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        switcher.refresh()
        tabSwitcherOverlay = switcher
    }

    fileprivate func dismissTabSwitcher() {
        tabSwitcherOverlay?.removeFromSuperview()
        tabSwitcherOverlay = nil
    }

    @objc private func newTabTapped() {
        openNewTabAndFocusAddress()
    }

    fileprivate func openNewTabAndFocusAddress() {
        guard LxAppBrowser.openNewTab() else { return }
        addressField.text = ""
        addressIcon.isHidden = true
        DispatchQueue.main.async { [weak self] in
            guard let self,
                  !(LxAppBrowser.activeTabId.map { browserTabIsAside($0) } ?? false),
                  self.viewIfLoaded?.window != nil else { return }
            self.addressField.becomeFirstResponder()
        }
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

    func textFieldShouldBeginEditing(_ textField: UITextField) -> Bool {
        !(LxAppBrowser.activeTabId.map { browserTabIsAside($0) } ?? false)
    }

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
        guard !(LxAppBrowser.activeTabId.map { browserTabIsAside($0) } ?? false) else {
            textField.resignFirstResponder()
            return false
        }
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
/// In-view tab switcher: scrim + edge-to-edge bottom panel sized to the tab
/// rows (capped), shared by self and aside modes.
@MainActor
private final class LxAppBrowserTabSwitcherView: UIView, UITableViewDataSource, UITableViewDelegate {
    private weak var host: LxAppBrowserViewController?
    private let tableView = UITableView(frame: .zero, style: .plain)
    private var tableHeightConstraint: NSLayoutConstraint?
    private static let cellReuseId = "LxAppBrowserTabCell"
    private static let rowHeight: CGFloat = 56
    private static let maxListHeight: CGFloat = 360
    private var tabIds: [String] { LxAppBrowser.visibleTabIds }

    init(host: LxAppBrowserViewController) {
        self.host = host
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false

        let scrim = UIControl()
        scrim.translatesAutoresizingMaskIntoConstraints = false
        scrim.backgroundColor = UIColor(white: 0, alpha: 0.4)
        scrim.addTarget(self, action: #selector(scrimTapped), for: .touchUpInside)
        addSubview(scrim)

        let panel = UIView()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.backgroundColor = .white
        panel.layer.cornerRadius = 16
        panel.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        panel.clipsToBounds = true
        addSubview(panel)

        let titleLabel = UILabel()
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.text = "Tabs"
        titleLabel.font = UIFont.systemFont(ofSize: 17, weight: .semibold)
        panel.addSubview(titleLabel)

        let addButton = UIButton(type: .system)
        addButton.translatesAutoresizingMaskIntoConstraints = false
        addButton.tintColor = UIColor(white: 0.2, alpha: 1.0)
        addButton.setImage(lxAppBrowserIconImage(named: "icon_plus", size: 20)?.withRenderingMode(.alwaysTemplate), for: .normal)
        addButton.addTarget(self, action: #selector(newTabTapped), for: .touchUpInside)
        // New tabs are self mode; hide the affordance while an aside is active.
        addButton.isHidden = LxAppBrowser.activeTabId.map { browserTabIsAside($0) } ?? false
        panel.addSubview(addButton)

        let divider = UIView()
        divider.translatesAutoresizingMaskIntoConstraints = false
        divider.backgroundColor = UIColor(white: 0, alpha: 0.07)
        panel.addSubview(divider)

        tableView.translatesAutoresizingMaskIntoConstraints = false
        tableView.dataSource = self
        tableView.delegate = self
        tableView.register(LxAppBrowserTabCell.self, forCellReuseIdentifier: Self.cellReuseId)
        tableView.rowHeight = Self.rowHeight
        tableView.separatorStyle = .none
        panel.addSubview(tableView)

        let tableHeight = tableView.heightAnchor.constraint(equalToConstant: Self.rowHeight)
        tableHeightConstraint = tableHeight

        NSLayoutConstraint.activate([
            scrim.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrim.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrim.topAnchor.constraint(equalTo: topAnchor),
            scrim.bottomAnchor.constraint(equalTo: bottomAnchor),

            panel.leadingAnchor.constraint(equalTo: leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: trailingAnchor),
            panel.bottomAnchor.constraint(equalTo: bottomAnchor),

            titleLabel.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 20),
            titleLabel.topAnchor.constraint(equalTo: panel.topAnchor, constant: 16),

            addButton.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -12),
            addButton.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            addButton.widthAnchor.constraint(equalToConstant: 40),
            addButton.heightAnchor.constraint(equalToConstant: 36),

            divider.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            divider.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            divider.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 14),
            divider.heightAnchor.constraint(equalToConstant: 0.5),

            tableView.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            tableView.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            tableView.topAnchor.constraint(equalTo: divider.bottomAnchor),
            tableView.bottomAnchor.constraint(equalTo: safeAreaLayoutGuide.bottomAnchor),
            tableHeight,
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// Reload the rows and size the list to their count, capped for long lists.
    func refresh() {
        let rows = max(tabIds.count, 1)
        tableHeightConstraint?.constant = min(CGFloat(rows) * Self.rowHeight, Self.maxListHeight)
        tableView.reloadData()
    }

    @objc private func scrimTapped() {
        host?.dismissTabSwitcher()
    }

    @objc private func newTabTapped() {
        host?.dismissTabSwitcher()
        host?.openNewTabAndFocusAddress()
    }

    private func closeTab(at index: Int) {
        guard index < tabIds.count else { return }
        let tabId = tabIds[index]
        LxAppBrowser.closeTab(tabId: tabId)
        if tabIds.isEmpty {
            host?.dismissTabSwitcher()
        } else {
            refresh()
        }
    }

    // MARK: - UITableViewDataSource / Delegate

    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        tabIds.count
    }

    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: Self.cellReuseId, for: indexPath) as! LxAppBrowserTabCell
        let tabId = tabIds[indexPath.row]
        let label = host?.tabLabel(forTabId: tabId) ?? tabId
        let isActive = LxAppBrowser.activeTabId == tabId
        cell.configure(title: label, isActive: isActive) { [weak self] in
            self?.closeTab(at: indexPath.row)
        }
        return cell
    }

    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: false)
        let tabId = tabIds[indexPath.row]
        host?.dismissTabSwitcher()
        LxAppBrowser.activate(tabId: tabId)
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
