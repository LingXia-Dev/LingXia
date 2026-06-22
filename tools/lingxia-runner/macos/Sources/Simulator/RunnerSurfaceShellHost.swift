import AppKit
@_spi(Runner) import lingxia

/// Runner host for pad/desktop shapes.
///
/// This deliberately delegates app chrome, browser tabs, URL asides, and
/// adaptive surface layout to the SDK desktop shell. The runner only owns device
/// selection and window sizing policy here.
@MainActor
private final class RunnerSurfaceDeviceSelector: NSView {
    private let button = NSButton()
    private var selectedDevice: MobileDeviceSize?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setup()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func updateDevice(_ device: MobileDeviceSize) {
        selectedDevice = device
        button.image = Self.symbol(for: device)
        button.toolTip = "Device: \(device.displayName)"
    }

    private func setup() {
        translatesAutoresizingMaskIntoConstraints = false

        button.translatesAutoresizingMaskIntoConstraints = false
        button.title = ""
        button.imagePosition = .imageOnly
        button.bezelStyle = .regularSquare
        button.isBordered = false
        button.setButtonType(.momentaryChange)
        button.target = self
        button.action = #selector(showDeviceMenu)
        button.contentTintColor = .secondaryLabelColor

        addSubview(button)
        NSLayoutConstraint.activate([
            button.leadingAnchor.constraint(equalTo: leadingAnchor),
            button.trailingAnchor.constraint(equalTo: trailingAnchor),
            button.topAnchor.constraint(equalTo: topAnchor),
            button.bottomAnchor.constraint(equalTo: bottomAnchor),
            widthAnchor.constraint(equalToConstant: 30),
            heightAnchor.constraint(equalToConstant: 24),
        ])
    }

    @objc private func showDeviceMenu() {
        let menu = NSMenu(title: "Device")
        var selectedItem: NSMenuItem?

        let clean = NSMenuItem(
            title: "Clean Cache and Restart LxApp",
            action: #selector(cleanCacheAndRestartLxApp(_:)),
            keyEquivalent: ""
        )
        clean.target = self
        clean.image = NSImage(systemSymbolName: "trash", accessibilityDescription: nil)
        menu.addItem(clean)

        let restart = NSMenuItem(
            title: "Restart LxApp",
            action: #selector(restartLxApp(_:)),
            keyEquivalent: ""
        )
        restart.target = self
        restart.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: nil)
        menu.addItem(restart)
        menu.addItem(.separator())

        var previousShape: RunnerDeviceShape?
        for device in MobileDeviceSize.allCases {
            if let previousShape, previousShape != device.shape {
                menu.addItem(.separator())
            }
            let item = NSMenuItem(
                title: device.displayName,
                action: #selector(deviceSelectionChanged(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = device
            item.state = device == selectedDevice ? .on : .off
            menu.addItem(item)
            if item.state == .on {
                selectedItem = item
            }
            previousShape = device.shape
        }
        menu.popUp(positioning: selectedItem, at: NSPoint(x: 0, y: bounds.maxY + 2), in: self)
    }

    @objc private func deviceSelectionChanged(_ sender: NSMenuItem) {
        guard let device = sender.representedObject as? MobileDeviceSize else {
            return
        }
        RunnerApp.shared.setDeviceSize(device)
    }

    @objc private func cleanCacheAndRestartLxApp(_ sender: NSMenuItem) {
        RunnerApp.shared.cleanCacheAndRestartCurrentLxApp()
    }

    @objc private func restartLxApp(_ sender: NSMenuItem) {
        RunnerApp.shared.restartCurrentLxApp()
    }

    private static func symbol(for device: MobileDeviceSize) -> NSImage? {
        let name: String
        switch device.shape {
        case .phone:
            name = "iphone"
        case .pad:
            name = "ipad"
        case .desktop:
            name = "desktopcomputer"
        }
        return NSImage(systemSymbolName: name, accessibilityDescription: "Device")
    }
}

@MainActor
final class RunnerSurfaceShellHost {
    let shell: LxAppShell

    private(set) var appId: String
    private(set) var currentPath: String
    private(set) var device: MobileDeviceSize

    private let deviceSelector = RunnerSurfaceDeviceSelector()
    private var deviceAccessoryController: NSTitlebarAccessoryViewController?
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
        deviceSelector.updateDevice(newDevice)
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
        guard deviceAccessoryController == nil, let window = shell.window else { return }
        let controller = NSTitlebarAccessoryViewController()
        controller.layoutAttribute = .left
        controller.view = deviceSelector
        deviceSelector.updateDevice(device)
        window.addTitlebarAccessoryViewController(controller)
        deviceAccessoryController = controller
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
