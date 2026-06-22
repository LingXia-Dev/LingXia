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
        RunnerSupport.Runtime.useDefaultOpenUrlHandling()
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

    func applyDevice(_ newDevice: MobileDeviceSize) {
        device = newDevice
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

    private func configureWindow(for device: MobileDeviceSize, center: Bool) {
        guard let window = shell.window else { return }

        let contentSize = NSSize(width: device.width, height: device.height)
        window.title = "LingXia Runner - \(device.displayName)"
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
