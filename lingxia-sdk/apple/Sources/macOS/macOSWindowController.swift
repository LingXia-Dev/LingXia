#if os(macOS)
import AppKit
import WebKit
import os.log

/// Unified window controller for macOS - supports both capsule and tab modes
class macOSWindowController: NSWindowController, NSWindowDelegate {

    // Static Configuration
    private static var windowWidth: CGFloat = 800
    private static var windowHeight: CGFloat = 600
    private static var windowStyle: LxAppWindowStyle = .tabStyle

    /// Sets the window size for all new windows
    public static func setWindowSize(width: CGFloat, height: CGFloat) {
        windowWidth = width
        windowHeight = height
    }

    /// Sets the window style for all new windows
    public static func setWindowStyle(_ style: LxAppWindowStyle) {
        windowStyle = style
    }

    /// Gets the current window style
    public static func getWindowStyle() -> LxAppWindowStyle {
        return windowStyle
    }

    /// Gets the top margin for current window style
    public static func getTopMarginForCurrentStyle() -> CGFloat {
        return macOSWindowSupport.getTopMarginForStyle(windowStyle)
    }

    /// Gets the current window width
    public static func getWindowWidth() -> CGFloat {
        return windowWidth
    }

    // Single LxApp mode
    var appId: String?
    var path: String?
    private var navigationBar: macOSNavigationBar?

    // Tab mode
    private let tabManager = LxAppTabManager()
    private var tabView: macOSTabView?
    private var currentViewController: macOSLxAppViewController?
    private var viewControllers: [String: macOSLxAppViewController] = [:]

    // Common
    private var switchPageObserver: NSObjectProtocol?

    /// Initialize for single LxApp mode
    init(appId: String, path: String) {
        self.appId = appId
        self.path = path

        let window = Self.createWindow()
        super.init(window: window)

        setupSingleAppMode()
    }

    /// Initialize for tab mode
    init() {
        super.init(window: Self.createWindow(width: 1200, height: 800, style: .tabStyle))
        setupTabMode()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private static func createWindow(width: CGFloat? = nil, height: CGFloat? = nil, style: LxAppWindowStyle? = nil) -> macOSLxAppWindow {
        let finalWidth = width ?? windowWidth
        let finalHeight = height ?? windowHeight
        let finalStyle = style ?? windowStyle

        let styleMask: NSWindow.StyleMask
        switch finalStyle {
        case .capsuleStyle:
            styleMask = [.titled, .closable, .miniaturizable]
        case .tabStyle:
            styleMask = [.titled, .closable, .miniaturizable, .resizable]
        }

        let window = macOSLxAppWindow(
            contentRect: NSRect(x: 0, y: 0, width: finalWidth, height: finalHeight),
            styleMask: styleMask,
            backing: .buffered,
            defer: false
        )

        window.configureForStyle(finalStyle)
        window.center()
        window.isReleasedWhenClosed = false

        return window
    }

    private func setupSingleAppMode() {
        guard let appId = appId, let path = path else { return }

        self.window?.delegate = self

        let viewController = macOSLxAppViewController(appId: appId, path: path)
        self.window?.contentViewController = viewController

        setupNotificationObservers()

        if Self.windowStyle == .capsuleStyle {
            DispatchQueue.main.async { [weak self] in
                self?.setupTitleBar()
            }
        }
    }

    private func setupTabMode() {
        self.window?.delegate = self

        if let window = self.window as? macOSLxAppWindow {
            window.standardWindowButton(.zoomButton)?.isHidden = false
        }

        tabManager.onTabChanged = { [weak self] tab in
            self?.switchToTab(tab.appId)
        }

        setupTabInterface()
        setupInitialTab()
    }

    func windowWillClose(_ notification: Notification) {
        if let appId = appId {
            // Single app mode
            macOSLxApp.handleAppClosing(appId: appId)
            removeNotificationObservers()
            macOSLxApp.removeWindowController(self)
        } else {
            // Tab mode
            for tab in tabManager.tabs {
                let _ = onLxappClosed(tab.appId)
            }
            macOSLxApp.removeTabWindowController(self)
        }
    }

    private func setupNotificationObservers() {
        guard let appId = appId else { return }

        switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let notificationAppId = notification.userInfo?["appId"] as? String,
                  let path = notification.userInfo?["path"] as? String,
                  notificationAppId == appId else { return }

            Task { @MainActor in
                self.updateWindowTitle(for: path)
            }
        }
    }

    private func removeNotificationObservers() {
        if let observer = switchPageObserver {
            NotificationCenter.default.removeObserver(observer)
            switchPageObserver = nil
        }
    }

    public func updateWindowTitle(for path: String) {
        guard let appId = appId else { return }
        self.path = path

        guard let navigationBar = self.navigationBar else { return }

        let pageConfig: NavigationBarConfig? = macOSPageNavigation.getNavigationBarConfig(appId: appId, path: path)
        _ = navigationBar.updateWithConfig(
            pageConfig: pageConfig,
            isBackNavigation: false,
            disableAnimation: true,
            onBackClickListener: {},
            onAnimationEnd: { }
        )
    }

    private func setupTitleBar() {
        guard let window = self.window, let contentView = window.contentView else { return }

        window.standardWindowButton(.closeButton)?.isHidden = true
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true

        guard let navBar = macOSNavigationBar.createForWindow(window) else { return }
        self.navigationBar = navBar
        contentView.addSubview(navBar)

        if let path = path {
            updateWindowTitle(for: path)
        }

        if Self.windowStyle == .capsuleStyle {
            setupCapsuleButtons(on: navBar)
        }
    }

    private func setupCapsuleButtons(on titleBarView: NSView) {
        // Use the dedicated macOSCapsuleButton class for proper visual effects
        macOSCapsuleButton.addCapsuleButtons(
            to: titleBarView,
            windowWidth: Self.windowWidth,
            target: self,
            moreAction: #selector(moreButtonClicked),
            minimizeAction: #selector(minimizeButtonClicked),
            closeAction: #selector(closeButtonClicked)
        )
    }

    @objc private func moreButtonClicked() {
        // More button action
    }

    @objc private func minimizeButtonClicked() {
        window?.miniaturize(nil)
    }

    @objc private func closeButtonClicked() {
        window?.close()
    }

    private func setupTabInterface() {
        guard let window = self.window, let contentView = window.contentView else { return }

        tabView = macOSTabView(tabManager: tabManager)
        guard let tabBar = tabView else { return }

        tabBar.translatesAutoresizingMaskIntoConstraints = false
        tabBar.onTabSelected = { [weak self] appId in
            self?.switchToTab(appId)
        }
        tabBar.onTabClosed = { [weak self] appId in
            self?.closeTab(appId)
        }

        contentView.addSubview(tabBar)

        NSLayoutConstraint.activate([
            tabBar.topAnchor.constraint(equalTo: contentView.topAnchor),
            tabBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            tabBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            tabBar.heightAnchor.constraint(equalToConstant: 32)
        ])
    }

    private func setupInitialTab() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else { return }

        let initialRoute = LxAppCore.getHomeLxAppInitialRoute()
        LxAppCore.setLastActivePath(initialRoute, for: homeLxAppId)
        tabManager.addTab(appId: homeLxAppId)
    }

    public func openLxApp(appId: String, path: String) {
        LxAppCore.setLastActivePath(path, for: appId)
        tabManager.addTab(appId: appId)
    }

    private func switchToTab(_ appId: String) {
        let viewController = viewControllers[appId] ?? {
            let currentPath = LxAppCore.getLastActivePath(for: appId, defaultPath: "/")
            let vc = macOSLxAppViewController(appId: appId, path: currentPath)
            viewControllers[appId] = vc
            let _ = onLxappOpened(appId, currentPath)
            return vc
        }()

        updateContentView(with: viewController)
    }

    private func updateContentView(with viewController: macOSLxAppViewController) {
        currentViewController?.view.removeFromSuperview()
        currentViewController = viewController

        guard let window = self.window, let contentView = window.contentView else { return }

        viewController.view.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(viewController.view)

        NSLayoutConstraint.activate([
            viewController.view.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 32),
            viewController.view.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            viewController.view.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            viewController.view.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])
    }

    private func closeTab(_ appId: String) {
        if let viewController = viewControllers[appId] {
            viewController.view.removeFromSuperview()
            viewControllers.removeValue(forKey: appId)
        }

        tabManager.closeTab(appId: appId)
        let _ = onLxappClosed(appId)

        if !tabManager.hasTabs {
            window?.close()
        }
    }
}

#endif
