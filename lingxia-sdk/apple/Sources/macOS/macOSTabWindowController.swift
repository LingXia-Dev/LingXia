#if os(macOS)
import AppKit
import WebKit

/// Tab-style window controller for macOS - manages multiple LxApps in tabs
class macOSTabWindowController: NSWindowController, NSWindowDelegate {

    private let tabManager = LxAppTabManager()
    private var tabView: macOSTabView?
    private var currentViewController: macOSLxAppViewController?
    private var viewControllers: [String: macOSLxAppViewController] = [:]  // appId -> ViewController

    init() {
        let window = macOSLxAppWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        window.standardWindowButton(.zoomButton)?.isHidden = false
        window.configureForStyle(LxAppWindowStyle.tabStyle)
        window.center()
        window.isReleasedWhenClosed = false

        super.init(window: window)

        self.window?.delegate = self

        // Simple callback instead of complex delegate
        tabManager.onTabChanged = { [weak self] tab in
            self?.switchToTab(tab.appId)
        }

        setupTabInterface()
        setupInitialTab()
    }

    private func setupInitialTab() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            return
        }

        let initialRoute = LxAppCore.getHomeLxAppInitialRoute()

        // Store the initial route for this app
        LxAppCore.setLastActivePath(initialRoute, for: homeLxAppId)
        tabManager.addTab(appId: homeLxAppId)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupTabInterface() {
        guard let window = self.window, let contentView = window.contentView else { return }

        // Create tab view
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

        // Setup tab bar constraints
        NSLayoutConstraint.activate([
            tabBar.topAnchor.constraint(equalTo: contentView.topAnchor),
            tabBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            tabBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            tabBar.heightAnchor.constraint(equalToConstant: 32)
        ])
    }

    public func openLxApp(appId: String, path: String) {
        // Store the path for this app
        LxAppCore.setLastActivePath(path, for: appId)
        tabManager.addTab(appId: appId)
    }

    private func switchToTab(_ appId: String) {
        // Get or create view controller for this app
        let viewController = viewControllers[appId] ?? {
            let currentPath = LxAppCore.getLastActivePath(for: appId, defaultPath: "/")

            let vc = macOSLxAppViewController(appId: appId, path: currentPath)
            viewControllers[appId] = vc

            let _ = onLxappOpened(appId, currentPath)
            return vc
        }()

        // Switch to this view controller
        updateContentView(with: viewController)
    }

    private func updateContentView(with viewController: macOSLxAppViewController) {
        // Remove current view controller
        currentViewController?.view.removeFromSuperview()
        currentViewController = viewController

        // Add new view controller
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
        // Remove view controller from cache
        if let viewController = viewControllers[appId] {
            viewController.view.removeFromSuperview()
            viewControllers.removeValue(forKey: appId)
        }

        tabManager.closeTab(appId: appId)

        // Close LxApp
        let _ = onLxappClosed(appId)

        // If no tabs left, close window
        if !tabManager.hasTabs {
            window?.close()
        }
    }

    // Window Delegate
    func windowWillClose(_ notification: Notification) {
        // Close all LxApps in tabs
        for tab in tabManager.tabs {
            let _ = onLxappClosed(tab.appId)
        }
        macOSLxApp.removeTabWindowController(self)
    }
}

#endif
