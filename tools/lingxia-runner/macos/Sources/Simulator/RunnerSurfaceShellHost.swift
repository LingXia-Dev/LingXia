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

    // The same toolbar as the iPhone simulator, so phone and pad share one UI.
    private lazy var toolbar: SimulatorToolbar = {
        let bar = SimulatorToolbar()
        bar.onDeviceSelected = { device in RunnerApp.shared.setDeviceSize(device) }
        bar.onRotateClicked = { RunnerApp.shared.toggleDeviceOrientation() }
        bar.onInspectClicked = { [weak self] in self?.openInspector() }
        return bar
    }()

    /// Toggle the Safari Web Inspector for the shell's active webview (same as the
    /// phone simulator's DevTools action).
    private func openInspector() {
        guard let webView = RunnerSupport.WebView.current() else { return }
        let retained = Unmanaged.passRetained(webView)
        let ptr = UInt(bitPattern: retained.toOpaque())
        _ = toggleWebViewDevtoolsByPtr(ptr, true)
        retained.release()
    }
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
        observeClose()
        installDeviceSelector()
        configureWindow(for: device, center: true)
        open(appId: appId, path: path, sessionId: sessionId)
    }

    deinit {
        if let closeObserver {
            NotificationCenter.default.removeObserver(closeObserver)
        }
    }

    func activate() {
        isHiddenForHostSwitch = false
        RunnerSupport.SurfaceShell.activate(shell)
        shell.window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func open(appId: String, path: String, sessionId: UInt64) {
        self.appId = appId
        self.currentPath = path
        RunnerSupport.Runtime.setSessionId(sessionId, for: appId)
        RunnerSupport.Runtime.setCurrentApp(appId: appId, path: path)
        activate()
        RunnerSupport.SurfaceShell.open(shell, appId: appId, path: path, sessionId: sessionId)
        DevToolsLogger.shared.log("Opened \(appId) in SDK shell -> \(path)", level: .nav)
    }

    func navigate(to path: String, animationType: LxAppAnimation) {
        currentPath = path
        RunnerSupport.SurfaceShell.navigate(
            shell,
            appId: appId,
            path: path,
            animationType: animationType
        )
    }

    func presentBrowserTab(id tabId: String) {
        RunnerSupport.SurfaceShell.presentBrowserTab(shell, tabId: tabId)
        shell.window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func applyDevice(_ newDevice: MobileDeviceSize) {
        device = newDevice
        toolbar.setCurrentDevice(newDevice)
        configureWindow(for: newDevice, center: false)
        DevToolsLogger.shared.log("Device -> \(newDevice.displayName) (\(newDevice.sizeDescription))", level: .debug)
    }

    func hideForHostSwitch() {
        refreshCurrentPathFromRuntime()
        isHiddenForHostSwitch = true
        shell.window?.orderOut(nil)
    }

    func refreshCurrentPathFromRuntime() {
        if RunnerSupport.Runtime.currentAppId() == appId {
            currentPath = RunnerSupport.Runtime.currentPath()
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
                self.onClose?(self)
            }
        }
    }

    private func installDeviceSelector() {
        // Float the toolbar above the content with a gap (like the iPhone simulator),
        // so the rounded toolbar never sits flush against the content.
        let gap: CGFloat = 12
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
            shell, view: host, height: SimulatorToolbar.Layout.height + gap)
        toolbar.setCurrentDevice(device)
    }

    private func configureWindow(for device: MobileDeviceSize, center: Bool) {
        guard let window = shell.window else { return }

        let contentSize = NSSize(width: device.width, height: device.height)
        window.title = "LingXia Runner - \(device.orientedDisplayName)"
        // Frameless: no real macOS traffic lights. The toolbar strip above the
        // content carries its own close/minimize dots (like the iPhone simulator).
        RunnerSupport.SurfaceShell.setTrafficLightsVisible(shell, visible: false)
        window.maxSize = NSSize(
            width: CGFloat.greatestFiniteMagnitude,
            height: CGFloat.greatestFiniteMagnitude
        )

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
        }
    }
}
