import AppKit
import WebKit
import SwiftUI
import os.log
@_spi(Runner) import lingxia

/// View controller for Runner Simulator mode - mirrors macOSLxAppViewController functionality
@MainActor
public class SimulatorViewController: NSViewController, WKNavigationDelegate {
    
    private static let log = OSLog(subsystem: "LingXiaRunner", category: "SimulatorViewController")
    
    // MARK: - Properties
    
    public let appId: String
    public private(set) var currentPath: String
    /// The page instance shown at `currentPath`; nil follows the app's current page.
    private var currentPageInstanceId: String?
    private var displayScale: CGFloat
    
    private var webViewContainer: NSView!
    /// The GPU page on screen, spanning the phone screen under the bars; nil for a WebView page.
    private var activeGpuView: NSView?
    /// A dialog of the GPU page is open: the window controller hides the bars it draws.
    var onTopLayer: ((Bool) -> Void)?
    private var topLayerOpen = false
    internal var tabBarView: NSView?
    internal var tabBarConfig: RunnerTabBarConfig?
    internal var selectedTabIndex: Int = 0
    public var isDestroyed: Bool = false
    
    private var currentTopMargin: CGFloat = 0
    private var tabBarConstraints: [NSLayoutConstraint] = []
    private var tabBarTopConstraint: NSLayoutConstraint?
    private var tabBarBottomConstraint: NSLayoutConstraint?
    /// The system bar along the bottom edge of an Android or HarmonyOS phone: a plain strip
    /// with the gesture handle, over the page.
    private var systemBar: SystemBarView?
    private var systemBarHeight: NSLayoutConstraint?
    private var webViewEdgeConstraints: [NSLayoutConstraint] = []
    
    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var tabBarObserver: NSObjectProtocol?
    
    // MARK: - Initialization
    
    public init(appId: String, path: String, displayScale: CGFloat = 1) {
        self.appId = appId
        self.currentPath = path
        self.displayScale = min(1, max(0.4, displayScale))
        super.init(nibName: nil, bundle: nil)
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
        tabBarObserver.map(NotificationCenter.default.removeObserver)
    }
    
    // MARK: - Layout

    private var webViewTopConstraint: NSLayoutConstraint?

    internal func updateTopMargin(_ newMargin: CGFloat) {
        currentTopMargin = newMargin
        webViewTopConstraint?.constant = newMargin
        tabBarTopConstraint?.constant = newMargin
        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
        simulateGpuPhone()
    }
    
    // MARK: - View Lifecycle
    
    public override func loadView() {
        view = NSView()
        view.wantsLayer = true
        // Clear, not windowBackgroundColor: this host fills the square phone area
        // behind the rounded device shape, so an opaque bg leaks (cream in Light
        // Mode) outside the rounded corners. The lxapp paints its own background.
        view.layer?.backgroundColor = NSColor.clear.cgColor
    }
    
    public override func viewDidLoad() {
        super.viewDidLoad()

        setupLayout()
        setupNotificationObservers()
        // The phone's side panel starts below the status bar, whatever renders the page.
        LingxiaRunnerSPI.PhoneAside.setStatusInset {
            SimulatorWindowController.Layout.systemStatusBarHeight
        }
        loadWebViewContent()
    }

    public override func viewDidLayout() {
        super.viewDidLayout()
        placeSystemBar()
        reportSurfaceMetrics()
        simulateGpuPhone()
    }

    /// The oriented device, as the page is told it.
    private var simulatedDevice: MobileDeviceSize {
        let app = RunnerApp.shared
        return app.selectedDeviceSize
            .withPosture(app.activePosture)
            .oriented(app.deviceOrientation)
    }

    /// Window points between the screen's bottom edge and the host's tab bar.
    private func tabBarLift() -> CGFloat {
        simulatedDevice.tabBarLift * displayScale
    }

    /// Draws the bottom system bar of a device whose host does not cover it with its own, and
    /// keeps the tab bar above it; no-op once in place.
    private func placeSystemBar() {
        let device = simulatedDevice
        let lift = tabBarLift()
        if let bottom = tabBarBottomConstraint, bottom.constant != -lift {
            bottom.constant = -lift
        }
        let height = device.drawsSystemBar ? device.safeArea.bottom * displayScale : 0
        guard height > 0 else {
            systemBar?.isHidden = true
            return
        }
        if systemBar == nil {
            let bar = SystemBarView()
            bar.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(bar, positioned: .above, relativeTo: nil)
            let constraint = bar.heightAnchor.constraint(equalToConstant: height)
            NSLayoutConstraint.activate([
                bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                bar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                constraint,
            ])
            systemBar = bar
            systemBarHeight = constraint
        }
        systemBar?.isHidden = false
        if systemBarHeight?.constant != height { systemBarHeight?.constant = height }
        if let bar = systemBar, view.subviews.last !== bar {
            view.addSubview(bar, positioned: .above, relativeTo: nil)
        }
    }

    /// The page's viewport in the device's points: the frame is drawn at `displayScale`.
    private func surfaceViewport() -> CGSize? {
        let size = LingxiaRunnerSPI.SurfaceViewport.devicePoints(
            webViewContainer.bounds.size, displayScale: displayScale)
        return size.width > 0 && size.height > 0 ? size : nil
    }

    /// Keep the adaptive surface context aligned with the selected device frame.
    private func reportSurfaceMetrics() {
        guard !appId.isEmpty, let size = surfaceViewport(),
              LingxiaRunnerSPI.SurfaceViewport.admits(appId: appId, from: view.window) else { return }
        _ = setSurfaceWidth(appId, Double(size.width))
        _ = setSurfaceViewport(appId, Double(size.width), Double(size.height))
    }

    /// The phone presents the lxapp from now on: only its viewport reaches the core, measured
    /// once the switch settles (the desktop shell it replaces can still lay out meanwhile).
    func presentSurface(in window: NSWindow?) {
        guard !appId.isEmpty, let window else { return }
        let appId = self.appId
        LingxiaRunnerSPI.SurfaceViewport.present(appId: appId, by: window) { [weak self] in
            guard let self, self.appId == appId, let size = self.surfaceViewport() else { return nil }
            _ = setSurfaceWidth(appId, Double(size.width))
            return size
        }
    }

    // MARK: - UI Setup
    
    private func setupLayout() {
        view.wantsLayer = true
        // Clear, not windowBackgroundColor: this host fills the square phone area
        // behind the rounded device shape, so an opaque bg leaks (cream in Light
        // Mode) outside the rounded corners. The lxapp paints its own background.
        view.layer?.backgroundColor = NSColor.clear.cgColor
        
        setupWebViewContainer()
        setupWebViewConstraints()
        updateTabBar()
        
        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }
    
    private func setupTabBarConstraints(tabBar: NSView, config: RunnerTabBarConfig) {
        // The device's host bar, as the page is told it, drawn at the skin's fit.
        let device = simulatedDevice
        let dimension = device.chrome == nil
            ? CGFloat(config.dimension) * displayScale
            : (device.tabBarHeight + device.tabBarGrowth) * displayScale
        NSLayoutConstraint.deactivate(tabBarConstraints)
        tabBarConstraints = createTabBarConstraints(tabBar: tabBar, position: config.position, dimension: dimension)
        NSLayoutConstraint.activate(tabBarConstraints)

        if RunnerSupport.TabBar.isTransparent(config.background_color) {
            tabBar.wantsLayer = true
            tabBar.layer?.backgroundColor = NSColor.clear.cgColor
        }
    }
    
    private func createTabBarConstraints(tabBar: NSView, position: Int32, dimension: CGFloat) -> [NSLayoutConstraint] {
        switch position {
        case 0: // bottom
            tabBarTopConstraint = nil
            let bottom = tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -tabBarLift())
            tabBarBottomConstraint = bottom
            return [
                tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                bottom,
                tabBar.heightAnchor.constraint(equalToConstant: dimension)
            ]
        case 1: // left
            tabBarBottomConstraint = nil
            let top = tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
            tabBarTopConstraint = top
            return [
                tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                top,
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.widthAnchor.constraint(equalToConstant: dimension)
            ]
        case 2: // right
            tabBarBottomConstraint = nil
            let top = tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
            tabBarTopConstraint = top
            return [
                tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                top,
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.widthAnchor.constraint(equalToConstant: dimension)
            ]
        default: // fallback to bottom
            return createTabBarConstraints(tabBar: tabBar, position: 0, dimension: dimension)
        }
    }
    
    private func makeWebViewTopConstraint() -> NSLayoutConstraint {
        let c = webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
        webViewTopConstraint = c
        return c
    }

    private func setupWebViewConstraints() {
        makeWebViewTopConstraint().isActive = true
        updateWebViewEdgeConstraints(tabBar: nil, config: nil)
    }

    private func updateWebViewEdgeConstraints(tabBar: NSView?, config: RunnerTabBarConfig?) {
        NSLayoutConstraint.deactivate(webViewEdgeConstraints)

        var leading = webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor)
        var trailing = webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor)
        var bottom = webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)

        if let tabBar,
           let config,
           config.is_visible,
           !RunnerSupport.TabBar.isTransparent(config.background_color) {
            switch config.position {
            case 0: // bottom
                bottom = webViewContainer.bottomAnchor.constraint(equalTo: tabBar.topAnchor)
            case 1: // left
                leading = webViewContainer.leadingAnchor.constraint(equalTo: tabBar.trailingAnchor)
            case 2: // right
                trailing = webViewContainer.trailingAnchor.constraint(equalTo: tabBar.leadingAnchor)
            default:
                bottom = webViewContainer.bottomAnchor.constraint(equalTo: tabBar.topAnchor)
            }
        }

        webViewEdgeConstraints = [leading, trailing, bottom]
        NSLayoutConstraint.activate(webViewEdgeConstraints)
    }
    
    private func setupWebViewContainer() {
        webViewContainer = NSView()
        webViewContainer.wantsLayer = true
        webViewContainer.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(webViewContainer)
    }
    
    private func createTabBar(config tabBarConfig: RunnerTabBarConfig) {
        self.tabBarConfig = tabBarConfig

        let tabBar = RunnerSupport.TabBar.makeView(config: tabBarConfig, appId: appId) { [weak self] index, _ in
            guard let self = self else { return }
            let _ = onLxappEvent(self.appId, LxAppUiEventType.TabBarClick, String(index))
        }
        tabBar.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(tabBar)
        self.tabBarView = tabBar
        setupTabBarConstraints(tabBar: tabBar, config: tabBarConfig)
    }

    private func updateTabBar() {
        guard let tabBarConfig = RunnerSupport.TabBar.config(for: appId) else {
            updateWebViewEdgeConstraints(tabBar: nil, config: nil)
            tabBarView?.removeFromSuperview()
            tabBarView = nil
            self.tabBarConfig = nil
            NSLayoutConstraint.deactivate(tabBarConstraints)
            tabBarConstraints = []
            tabBarTopConstraint = nil
            view.needsLayout = true
            view.layoutSubtreeIfNeeded()
            return
        }

        self.tabBarConfig = tabBarConfig

        if tabBarView == nil {
            createTabBar(config: tabBarConfig)
        } else if let tabBar = tabBarView {
            setupTabBarConstraints(tabBar: tabBar, config: tabBarConfig)
            RunnerSupport.TabBar.refresh(tabBar)
        }

        // The bottom bar goes only where the short side reaches 600: a phone on its side keeps it.
        RunnerSupport.TabBar.setCompact(
            tabBarView,
            compact: !simulatedDevice.hasRegularShortSide
        )
        RunnerSupport.TabBar.setDisplayScale(tabBarView, scale: displayScale)

        if let tabBar = tabBarView {
            view.addSubview(tabBar, positioned: .above, relativeTo: nil)
            updateWebViewEdgeConstraints(tabBar: tabBar, config: tabBarConfig)
        }

        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
        simulateGpuPhone()
    }

    func setDisplayScale(_ value: CGFloat) {
        let normalized = min(1, max(0.4, value))
        guard abs(displayScale - normalized) > 0.001 else { return }
        displayScale = normalized
        guard let tabBar = tabBarView, let config = tabBarConfig else { return }
        setupTabBarConstraints(tabBar: tabBar, config: config)
        RunnerSupport.TabBar.setDisplayScale(tabBar, scale: normalized)
        updateWebViewEdgeConstraints(tabBar: tabBar, config: config)
        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }
    
    private func loadWebViewContent() {
        if let webView = RunnerSupport.WebView.page(
            appId: appId, pageInstanceId: currentPageInstanceId) {
            showWebViewToUser(webView, pageInstanceId: currentPageInstanceId)
        } else {
            showGpuPageWhenReady(pageInstanceId: currentPageInstanceId)
        }
    }

    /// The first page can mount before its shell exists: show it once the runtime reports it.
    private func showGpuPageWhenReady(pageInstanceId: String?) {
        if showGpuPage(pageInstanceId: pageInstanceId) { return }
        let path = currentPath
        LingxiaRunnerSPI.GpuPage.whenReady(appId: appId, pageInstanceId: pageInstanceId) { [weak self] ready in
            guard let self, ready, self.currentPath == path,
                  self.currentPageInstanceId == pageInstanceId,
                  RunnerSupport.WebView.page(appId: self.appId, pageInstanceId: pageInstanceId) == nil
            else { return }
            _ = self.showGpuPage(pageInstanceId: pageInstanceId)
        }
    }

    /// Show the page's GPU view when it renders natively; false for a WebView page. It spans the
    /// phone screen, as on a phone: the bars drawn over it are chrome insets the page pads by,
    /// and an open dialog's top layer covers them.
    private func showGpuPage(pageInstanceId: String?) -> Bool {
        guard let page = LingxiaRunnerSPI.GpuPage.view(appId: appId, pageInstanceId: pageInstanceId)
        else { return false }
        if activeGpuView === page, page.superview === view { return true }
        RunnerSupport.WebView.removeCurrentFromSuperview()
        LingxiaRunnerSPI.GpuPage.detach(activeGpuView)
        setTopLayer(false)
        LingxiaRunnerSPI.GpuPage.attach(page, to: view)
        LingxiaRunnerSPI.GpuPage.onTopLayer(page) { [weak self] modal in self?.setTopLayer(modal) }
        activeGpuView = page
        if let tabBar = tabBarView {
            view.addSubview(tabBar, positioned: .above, relativeTo: nil)
        }
        simulateGpuPhone()
        LingxiaRunnerSPI.GpuPage.shown(appId: appId, path: currentPath)
        return true
    }

    /// The emulated phone's viewport, safe area and host bars, from the preset in the device's
    /// points: what that device's host gives a page, whatever screen or fit the skin is drawn at.
    private func simulateGpuPhone() {
        guard let page = activeGpuView, view.bounds.width > 0 else { return }
        let device = simulatedDevice
        let safe = device.safeArea
        // The host's navigation bar sits below the safe-area top.
        let chromeTop = currentTopMargin > 0 ? safe.top + device.navigationBarHeight : 0
        var chromeBottom: CGFloat = 0
        // A dialog hides the bar but the page keeps its padding, as an iOS page raised over its bars does.
        if tabBarView != nil, let config = tabBarConfig,
           config.is_visible, config.position == 0 {
            chromeBottom = device.tabBarReach
        }
        LingxiaRunnerSPI.GpuPage.simulate(
            page,
            viewport: device.size,
            safeArea: NSEdgeInsets(top: safe.top, left: safe.left, bottom: safe.bottom, right: safe.right),
            chromeTop: chromeTop,
            chromeBottom: chromeBottom,
            smallestWidth: min(device.width, device.height)
        )
    }

    /// While a dialog is open the bars sit under it and take no clicks, as on a phone.
    private func setTopLayer(_ modal: Bool) {
        guard modal != topLayerOpen else { return }
        topLayerOpen = modal
        tabBarView?.isHidden = modal
        onTopLayer?(modal)
    }
    
    /// Unified method to show a WebView to the user
    private func showWebViewToUser(
        _ webView: WKWebView,
        pageInstanceId: String?,
        animation: LxAppAnimation = .none
    ) {
        // Sliding a page that has not painted yet shows its content settling
        // inside a frame that is already moving. Hold the outgoing page and
        // slide once the incoming one can draw itself; nothing changes on
        // screen while we wait, so it reads as the animation starting a moment
        // later rather than as a stall. Mirrors the SDK's macOS controller.
        if RunnerPageTransition.needsPaintWait(webView, animation: animation) {
            let path = currentPath
            pageTransition.whenPagePaints(
                webView,
                stillCurrent: { [weak self] in
                    self?.currentPath == path && self?.currentPageInstanceId == pageInstanceId
                },
                swap: { [weak self] in self?.performWebViewSwap(webView, animation: animation) }
            )
            return
        }
        performWebViewSwap(webView, animation: animation)
    }

    private func performWebViewSwap(_ webView: WKWebView, animation: LxAppAnimation) {
        webViewContainer.layer?.masksToBounds = true
        pageTransition.install(animation, on: webViewContainer)
        RunnerSupport.WebView.removeCurrentFromSuperview()
        LingxiaRunnerSPI.GpuPage.detach(activeGpuView)
        activeGpuView = nil
        setTopLayer(false)
        RunnerSupport.WebView.attachLxApp(webView, to: webViewContainer)
        if let tabBar = tabBarView {
            view.addSubview(tabBar, positioned: .above, relativeTo: nil)
        }
    }

    // MARK: - Notification Observers
    
    private func setupNotificationObservers() {
        closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name("com.lingxia.CLOSE_LXAPP_ACTION"), object: nil, queue: .main
        ) { [weak self] notification in
            let appId = notification.userInfo?["appId"] as? String
            Task { @MainActor in
                guard let self = self, let targetAppId = appId, targetAppId == self.appId else { return }
                self.view.window?.close()
            }
        }
        
        // Add TabBar state change observer. Opening a second lxapp leaves the
        // first window alive but hidden, so an unfiltered observer would have
        // every suspended window tear down and rebuild its tab-bar and
        // web-view constraints on another app's notification.
        tabBarObserver = NotificationCenter.default.addObserver(
            forName: RunnerSupport.TabBar.stateChangedNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            // Posters name the app they changed; an unnamed one is broadcast.
            let changed = notification.object as? String
            Task { @MainActor in
                guard let self = self else { return }
                if let changed, changed != self.appId { return }
                self.updateTabBar()
                // Applied: resolve any awaited lx.app.tabBar.update().
                LingxiaRunnerSPI.Tabs.updateApplied(appId: self.appId)
            }
        }
    }
    
    /// Owned by the SDK so the runner cannot drift from the host's animation.
    private let pageTransition = RunnerPageTransition()

    // MARK: - Navigation

    /// `pageInstanceId` names the page to present; nil presents the app's
    /// current page.
    public func navigate(
        to path: String,
        pageInstanceId: String? = nil,
        animationType: LxAppAnimation = .none
    ) {
        self.currentPath = path
        self.currentPageInstanceId = pageInstanceId
        
        // Update UI components
        updateNavigationBar(appId: appId, path: path)
        updateTabBar()
        
        // Show the page: its WebView, or its GPU view when it renders natively.
        if let webView = RunnerSupport.WebView.page(appId: appId, pageInstanceId: pageInstanceId) {
            showWebViewToUser(webView, pageInstanceId: pageInstanceId, animation: animationType)
        } else {
            showGpuPageWhenReady(pageInstanceId: pageInstanceId)
        }

        // Update app state
        RunnerSupport.Runtime.setCurrentPath(path, pageInstanceId: pageInstanceId)
    }
    
    public func setSelectedTabIndex(_ index: Int) {
        selectedTabIndex = index
    }
    
    // Method required by WindowController
    func updateLayoutForNavigationStyle(currentPath: String) {
        self.currentPath = currentPath
        
        // Tell TabBar to refresh its state from Rust - this will handle visibility and content for the new page
        updateTabBar()
    }
    
    /// Update navigation bar state
    @MainActor
    public func updateNavigationBar(appId: String, path: String) {
        RunnerSupport.Navigation.updateState(appId: appId, path: path)
        // Simulator mode: notify window controller to update its custom navigation bar
        NotificationCenter.default.post(
            name: .capsuleNavigationBarStateChanged,
            object: nil,
            userInfo: ["appId": appId, "path": path]
        )
    }
}

/// A phone's bottom system bar: the gesture handle on a plain strip, nothing branded.
final class SystemBarView: NSView {
    private let handle = NSView()

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        handle.wantsLayer = true
        handle.translatesAutoresizingMaskIntoConstraints = false
        addSubview(handle)
        NSLayoutConstraint.activate([
            handle.centerXAnchor.constraint(equalTo: centerXAnchor),
            handle.centerYAnchor.constraint(equalTo: centerYAnchor),
            handle.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.3),
            handle.heightAnchor.constraint(equalToConstant: 4),
        ])
        updateColors()
    }

    required init?(coder: NSCoder) { nil }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateColors()
    }

    // The bar takes no input: the page and its scrolls run under it, as on the phone.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func layout() {
        super.layout()
        handle.layer?.cornerRadius = handle.bounds.height / 2
    }

    private func updateColors() {
        let dark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        handle.layer?.backgroundColor = (dark ? NSColor.white : NSColor.black).withAlphaComponent(0.6).cgColor
    }
}
