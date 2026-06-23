import AppKit
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

    // The same selector the iPhone toolbar uses. The pad/desktop strip has no
    // rotate button or capsule, so orientation + lxapp lifecycle ride along as
    // extras below the device list.
    private lazy var deviceSelector: RunnerDeviceSelectorControl = {
        let control = RunnerDeviceSelectorControl(extras: [
            RunnerDeviceSelectorControl.ExtraItem(
                title: "Rotate", systemImage: "rotate.right", separatorBefore: true
            ) { RunnerApp.shared.toggleDeviceOrientation() },
            RunnerDeviceSelectorControl.ExtraItem(
                title: "Restart LxApp", systemImage: "arrow.clockwise", separatorBefore: true
            ) { RunnerApp.shared.restartCurrentLxApp() },
            RunnerDeviceSelectorControl.ExtraItem(
                title: "Clean Cache and Restart LxApp", systemImage: "trash"
            ) { RunnerApp.shared.cleanCacheAndRestartCurrentLxApp() },
        ])
        control.onDeviceSelected = { device in
            RunnerApp.shared.setDeviceSize(device)
        }
        return control
    }()
    private lazy var toolbar: RunnerSurfaceToolbar = {
        let bar = RunnerSurfaceToolbar(selector: deviceSelector)
        bar.onClose = { [weak self] in self?.shell.window?.performClose(nil) }
        bar.onMinimize = { [weak self] in self?.shell.window?.miniaturize(nil) }
        return bar
    }()
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
        deviceSelector.setCurrentDevice(newDevice)
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
        // Mount the toolbar as a content-level strip ABOVE the shell content (the
        // shell lays its sidebar/content out beneath it). This is what the iPhone
        // simulator does — the selector lives in its own chrome bar, never over the
        // app UI, and the bar's own dots replace the (hidden) traffic lights.
        RunnerSupport.SurfaceShell.setTopAccessory(shell, view: toolbar, height: RunnerSurfaceToolbar.height)
        deviceSelector.setCurrentDevice(device)
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
