import AppKit
import WebKit
@_spi(Runner) import lingxia

/// Runner host for pad/desktop shapes.
///
/// This deliberately delegates app chrome, browser tabs, URL asides, and
/// adaptive surface layout to the SDK desktop shell. The runner only owns device
/// selection and window sizing policy here.
@MainActor
final class RunnerSurfaceShellHost {
    let shell: LxAppShell

    private(set) var appId: String
    private(set) var currentPath: String
    private(set) var device: MobileDeviceSize
    private let headless: Bool

    // The same toolbar as the iPhone simulator, so phone and pad share one UI.
    private lazy var toolbar: SimulatorToolbar = {
        let bar = SimulatorToolbar()
        bar.onDeviceSelected = { device in RunnerApp.shared.setDeviceSize(device) }
        bar.onPostureSelected = { posture in RunnerApp.shared.setDevicePosture(posture) }
        bar.onRotateClicked = { RunnerApp.shared.toggleDeviceOrientation() }
        return bar
    }()
    private static let topAccessoryGap: CGFloat = 12
    private static var topAccessoryHeight: CGFloat {
        SimulatorToolbar.Layout.height + topAccessoryGap
    }
    private var hingeOverlay: RunnerHingeOverlayView?

    nonisolated(unsafe) private var closeObserver: NSObjectProtocol?
    private var isHiddenForHostSwitch = false
    var onClose: ((RunnerSurfaceShellHost) -> Void)?

    init(
        controller: LxAppController,
        appId: String,
        path: String,
        sessionId: UInt64,
        device: MobileDeviceSize
    ) {
        self.shell = RunnerSupport.SurfaceShell.make(controller: controller)
        self.appId = appId
        self.currentPath = path
        self.device = device
        self.headless = false
        RunnerSupport.SurfaceShell.setBrowserBlankNewTabs(shell, enabled: false)
        RunnerSupport.SurfaceShell.setBrowserPageActionsVisible(shell, visible: false)
        observeClose()
        installDeviceSelector()
        configureWindow(for: device, center: true)
        if let window = shell.window { RunnerWindowPlacement.restore(window) }
        open(appId: appId, path: path, sessionId: sessionId)
    }

    init(
        controller: LxAppController,
        webTarget: RunnerWebTarget,
        device: MobileDeviceSize,
        headless: Bool = false
    ) {
        self.shell = RunnerSupport.SurfaceShell.make(controller: controller)
        self.appId = webTarget.ownerAppId
        self.currentPath = webTarget.url.absoluteString
        self.device = device
        self.headless = headless
        RunnerSupport.SurfaceShell.setBrowserBlankNewTabs(shell, enabled: true)
        RunnerSupport.SurfaceShell.setBrowserClosesWindowOnLastTab(shell, enabled: true)
        RunnerSupport.SurfaceShell.setBrowserPageActionsVisible(shell, visible: false)
        observeClose()
        installDeviceSelector()
        configureWindow(for: device, center: true)
        if let window = shell.window { RunnerWindowPlacement.restore(window) }
        activate()
        presentBrowserTab(id: webTarget.tabId)
    }

    deinit {
        if let closeObserver {
            NotificationCenter.default.removeObserver(closeObserver)
        }
    }

    func activate() {
        isHiddenForHostSwitch = false
        RunnerSupport.SurfaceShell.activate(shell)
        if !headless {
            RunnerFront.show(shell.window)
        }
        presentSurface()
    }

    func open(appId: String, path: String, sessionId: UInt64) {
        RunnerSupport.SurfaceShell.setBrowserBlankNewTabs(shell, enabled: false)
        RunnerSupport.SurfaceShell.setBrowserClosesWindowOnLastTab(shell, enabled: false)
        self.appId = appId
        self.currentPath = path
        RunnerSupport.Runtime.setSessionId(sessionId, for: appId)
        RunnerSupport.Runtime.setCurrentApp(appId: appId, path: path)
        activate()
        RunnerSupport.SurfaceShell.open(shell, appId: appId, path: path, sessionId: sessionId)
        updateHingeOverlay(for: device)
        reportSurfaceMetricsWhenReady()
        DevToolsLogger.shared.log("Opened \(appId) in SDK shell -> \(path)", level: .nav)
    }

    func navigate(to path: String, pageInstanceId: String?, animationType: LxAppAnimation) {
        currentPath = path
        RunnerSupport.SurfaceShell.navigate(
            shell,
            appId: appId,
            path: path,
            pageInstanceId: pageInstanceId,
            animationType: animationType
        )
    }

    func presentBrowserTab(id tabId: String) {
        RunnerSupport.SurfaceShell.setBrowserBlankNewTabs(shell, enabled: true)
        RunnerSupport.SurfaceShell.presentBrowserTab(shell, tabId: tabId)
        updateHingeOverlay(for: device)
        if !headless {
            RunnerFront.show(shell.window)
        }
    }

    func applyDevice(_ newDevice: MobileDeviceSize) {
        device = newDevice
        toolbar.setCurrentDevice(newDevice)
        configureWindow(for: newDevice, center: false)
        presentSurface()
        updateHingeOverlay(for: newDevice)
        reportSurfaceMetricsWhenReady()
        DevToolsLogger.shared.log("Device -> \(newDevice.displayName) (\(newDevice.sizeDescription))", level: .debug)
    }

    /// The shell presents the lxapp from now on: only its viewport reaches the core, measured
    /// once the switch settles (a phone frame closing can still lay out meanwhile).
    private func presentSurface() {
        guard let window = shell.window else { return }
        let appId = self.appId
        LingxiaRunnerSPI.SurfaceViewport.present(appId: appId, by: window) { [weak self] in
            guard let self, RunnerSupport.Runtime.currentAppId() == appId else { return nil }
            // A GPU page has no webview to measure: its view controller does.
            guard let webView = RunnerSupport.WebView.page(appId: appId) else {
                return LingxiaRunnerSPI.SurfaceShell.viewportSize(self.shell, appId: appId)
            }
            guard webView.window != nil else { return nil }
            webView.layoutSubtreeIfNeeded()
            return webView.bounds.size
        }
    }

    func hideForHostSwitch() {
        refreshCurrentPathFromRuntime()
        isHiddenForHostSwitch = true
        // Folding back to the phone frame opens it where this window stood, not where the last phone did.
        if let window = shell.window { RunnerWindowPlacement.remember(window) }
        shell.window?.orderOut(nil)
    }

    /// Realign the host record after the shell switched tabs on its own
    /// (e.g. the active tab closed) — no navigation, bookkeeping only.
    func noteActiveLxApp(appId: String) {
        self.appId = appId
        refreshCurrentPathFromRuntime()
        RunnerSupport.Runtime.setCurrentApp(appId: appId, path: currentPath)
    }

    func refreshCurrentPathFromRuntime() {
        // Ask the runtime's navigation stack, not the Swift-side current-app
        // cache — the cache goes stale when navigation happens through the
        // shell (tab switches, in-shell navigations).
        let current = getCurrentLxApp()
        let path = current.path.toString()
        if current.appid.toString() == appId, !path.isEmpty {
            currentPath = path
        }
    }

    private func observeClose() {
        guard let window = shell.window else { return }
        closeObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in
                guard !self.isHiddenForHostSwitch else { return }
                if let window = self.shell.window { RunnerWindowPlacement.remember(window) }
                self.onClose?(self)
            }
        }
    }

    private func installDeviceSelector() {
        // Float the toolbar above the content with a gap (like the iPhone simulator),
        // so the rounded toolbar never sits flush against the content.
        let host = NSView()
        host.translatesAutoresizingMaskIntoConstraints = false
        host.wantsLayer = true
        host.layer?.backgroundColor = NSColor(white: 0.11, alpha: 1.0).cgColor
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        host.addSubview(toolbar)
        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: host.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: SimulatorToolbar.Layout.height),
        ])
        RunnerSupport.SurfaceShell.setTopAccessory(
            shell, view: host, height: Self.topAccessoryHeight)
        LingxiaRunnerSPI.GpuPage.setScreenTopInset(Self.topAccessoryHeight)
        toolbar.setCurrentDevice(device)
        updateHingeOverlay(for: device)
    }

    private func updateHingeOverlay(for device: MobileDeviceSize) {
        guard let contentView = shell.window?.contentView else { return }
        let overlay: RunnerHingeOverlayView
        if let existing = hingeOverlay {
            overlay = existing
        } else {
            let created = RunnerHingeOverlayView()
            created.translatesAutoresizingMaskIntoConstraints = false
            contentView.addSubview(created, positioned: .above, relativeTo: nil)
            NSLayoutConstraint.activate([
                created.topAnchor.constraint(equalTo: contentView.topAnchor, constant: Self.topAccessoryHeight),
                created.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
                created.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
                created.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
            ])
            hingeOverlay = created
            overlay = created
        }
        overlay.setHinge(device.activeHinge)
        if contentView.subviews.last !== overlay {
            contentView.addSubview(overlay, positioned: .above, relativeTo: nil)
        }
    }

    private func configureWindow(for device: MobileDeviceSize, center: Bool) {
        guard let window = shell.window else { return }
        // A device switch resizes in place: keep the top-left where the user
        // left it, so a taller device grows downward instead of under the menu bar.
        let topLeft = NSPoint(x: window.frame.minX, y: window.frame.maxY)

        let contentSize = NSSize(width: device.width, height: device.height)
        window.title = "LingXia Runner - \(device.orientedDisplayName)"
        // Frameless: no real macOS traffic lights. The toolbar strip above the
        // content carries its own close/minimize dots (like the iPhone simulator).
        RunnerSupport.SurfaceShell.setTrafficLightsVisible(shell, visible: false)
        RunnerSupport.SurfaceShell.setDeviceCompactProjectionEnabled(
            shell,
            enabled: !device.isResizableDesktop && !device.isRegularWidth
        )
        window.maxSize = NSSize(
            width: CGFloat.greatestFiniteMagnitude,
            height: CGFloat.greatestFiniteMagnitude
        )
        // The device's size, not the Mac screen's: a page measures the same on any screen.
        RunnerSupport.SurfaceShell.setKeepsSizeOffScreen(shell, keeps: true)

        if device.isResizableDesktop {
            window.styleMask.insert(.resizable)
            window.contentMinSize = NSSize(width: 720, height: 480)
            window.minSize = NSSize(width: 720, height: 480)
            window.setContentSize(contentSize)
        } else {
            window.styleMask.remove(.resizable)
            window.contentMinSize = contentSize
            window.setContentSize(contentSize)
            let fixedFrame = window.frameRect(forContentRect: NSRect(origin: .zero, size: contentSize)).size
            window.minSize = fixedFrame
            window.maxSize = fixedFrame
        }

        if center {
            window.center()
        } else {
            let drifted = abs(window.frame.minX - topLeft.x) > 0.5
                || abs(window.frame.maxY - topLeft.y) > 0.5
            if drifted {
                window.setFrameTopLeftPoint(topLeft)
            }
        }
    }
}
