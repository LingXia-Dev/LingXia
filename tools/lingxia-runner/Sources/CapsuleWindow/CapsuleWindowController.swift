import AppKit
import SwiftUI
import WebKit
import os.log
@_spi(Runner) import lingxia

// MARK: - Notification Names

extension Notification.Name {
    static let capsuleNavigationBarStateChanged = Notification.Name("CapsuleNavigationBarStateChanged")
}

/// Window controller for Runner Simulator mode
/// Provides Xcode-like simulator interface with toolbar and device frame
@MainActor
public class CapsuleWindowController: NSWindowController, NSWindowDelegate {

    private static let log = OSLog(subsystem: "LingXiaRunner", category: "CapsuleWindowController")

    // MARK: - Layout Constants

    public struct Layout {
        @MainActor public static var currentNotchSpec: iPhoneNotchSpec = .iPhoneSE

        @MainActor public static var systemStatusBarHeight: CGFloat {
            return currentNotchSpec.statusBarHeight
        }
        public static let navBarHeight: CGFloat = 40
        public static let capsuleContainerWidth: CGFloat = 88
        public static let capsuleContainerHeight: CGFloat = 26
        public static let capsuleTrailingMargin: CGFloat = 12
        public static let statusBarSideMargin: CGFloat = 12

        // Simulator layout - borderless window
        public static let toolbarToDeviceGap: CGFloat = 12  // Gap between toolbar and device
    }

    // MARK: - Device Configuration

    private static var currentDeviceSize: MobileDeviceSize = .iPhoneSE

    // MARK: - UI Components - Simulator Shell

    private var toolbar: SimulatorToolbar?
    private var deviceFrame: DeviceFrame?
    private var deviceFrameWidthConstraint: NSLayoutConstraint?
    private var deviceFrameHeightConstraint: NSLayoutConstraint?
    private var phoneContentView: NSView?  // The view that contains the phone screen content

    // MARK: - DevTools

    private var devToolsPanel: DevToolsPanel?
    private var isDevToolsVisible: Bool = false
    static let devToolsPanelWidth: CGFloat = DevToolsPanel.panelWidth

    // MARK: - UI Components - Phone Content

    private var viewController: CapsuleViewController?
    private var systemStatusBar: NSView?
    private var statusBarHeightConstraint: NSLayoutConstraint?
    private var navigationBar: NSView?
    private var floatingCapsuleContainer: NSView?
    private let browserOverlay = RunnerBrowserOverlay()

    // Status bar components
    private var timeLabel: NSTextField?
    private var batteryView: NSView?
    private var notchView: NSView?

    // MARK: - State

    public private(set) var appId: String
    public private(set) var currentPath: String

    // Observers
    nonisolated(unsafe) private var navigationBarObserver: NSObjectProtocol?
    private var suppressRuntimeCloseNotification = false
    
    // MARK: - Initialization
    
    public init(appId: String, path: String) {
        self.appId = appId
        self.currentPath = path
        
        let window = Self.createSimulatorWindow()
        super.init(window: window)
        
        setupSimulatorMode()
        setupNotificationObservers()
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    deinit {
        navigationBarObserver.map(NotificationCenter.default.removeObserver)
    }
    
    private func setupNotificationObservers() {
        navigationBarObserver = NotificationCenter.default.addObserver(
            forName: .capsuleNavigationBarStateChanged,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let appId = notification.userInfo?["appId"] as? String,
                  let path = notification.userInfo?["path"] as? String else { return }

            Task { @MainActor [weak self] in
                guard let self = self, appId == self.appId else { return }
                let navState = RunnerSupport.Navigation.state(appId: appId, path: path)
                self.updateNavigationBar(with: navState)
            }
        }
    }
    
    // MARK: - Configuration
    
    public static func setWindowSize(_ deviceSize: MobileDeviceSize) {
        currentDeviceSize = deviceSize
        Layout.currentNotchSpec = deviceSize.notchSpec
    }
    
    // MARK: - Window Creation
    
    private static func createSimulatorWindow() -> CapsuleWindow {
        let windowSize = calculateWindowSize(for: currentDeviceSize, devToolsWidth: 0)
        
        let window = CapsuleWindow(
            contentRect: NSRect(x: 0, y: 0, width: windowSize.width, height: windowSize.height),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.center()
        window.isReleasedWhenClosed = false
        window.title = "LingXia Simulator"
        return window
    }
    
    /// Calculate total window size including toolbar, device frame, and optional DevTools panel
    private static func calculateWindowSize(for device: MobileDeviceSize, devToolsWidth: CGFloat = 0) -> CGSize {
        let frameSize = DeviceFrame.frameSize(for: device)

        let width = frameSize.width + devToolsWidth

        let height = SimulatorToolbar.Layout.height
            + Layout.toolbarToDeviceGap
            + frameSize.height

        return CGSize(width: width, height: height)
    }
    
    // MARK: - Setup
    
    private func setupSimulatorMode() {
        self.window?.delegate = self

        guard let window = self.window, let contentView = window.contentView else { return }

        // Transparent background - borderless window
        contentView.wantsLayer = true
        contentView.layer?.backgroundColor = NSColor.clear.cgColor

        // Setup simulator UI structure
        setupToolbar(in: contentView)
        setupDeviceFrame(in: contentView)
        setupDevToolsPanel(in: contentView)
        setupPhoneContent(appId: appId, path: currentPath)

        // Apply initial navigation configuration
        applyInitialNavigationConfiguration()

        DevToolsLogger.shared.log("Opened \(appId) → \(currentPath)", level: .nav)
    }
    
    // MARK: - Toolbar Setup
    
    private func setupToolbar(in contentView: NSView) {
        let toolbar = SimulatorToolbar()
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.setCurrentDevice(Self.currentDeviceSize)
        
        toolbar.onDeviceSelected = { [weak self] device in
            self?.handleDeviceChange(device)
        }

        toolbar.onInspectClicked = { [weak self] in
            self?.openInspector()
        }
        
        contentView.addSubview(toolbar)
        
        // Toolbar spans full width, same as device
        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: contentView.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: SimulatorToolbar.Layout.height)
        ])
        
        self.toolbar = toolbar
    }
    
    // MARK: - Device Frame Setup

    private func setupDeviceFrame(in contentView: NSView) {
        let frame = DeviceFrame()
        frame.translatesAutoresizingMaskIntoConstraints = false
        frame.setDeviceSize(Self.currentDeviceSize)

        contentView.addSubview(frame)

        let frameSize = DeviceFrame.frameSize(for: Self.currentDeviceSize)
        let widthConstraint = frame.widthAnchor.constraint(equalToConstant: frameSize.width)
        let heightConstraint = frame.heightAnchor.constraint(equalToConstant: frameSize.height)

        // Left-aligned so devtools panel can appear on the right without shifting the phone
        NSLayoutConstraint.activate([
            frame.topAnchor.constraint(equalTo: toolbar!.bottomAnchor, constant: Layout.toolbarToDeviceGap),
            frame.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            widthConstraint,
            heightConstraint,
        ])

        self.deviceFrame = frame
        self.deviceFrameWidthConstraint = widthConstraint
        self.deviceFrameHeightConstraint = heightConstraint
    }

    // MARK: - DevTools Panel Setup

    private func setupDevToolsPanel(in contentView: NSView) {
        guard let toolbar = toolbar, let deviceFrame = deviceFrame else { return }

        let panel = DevToolsPanel()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.isHidden = true
        contentView.addSubview(panel)

        NSLayoutConstraint.activate([
            panel.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            panel.leadingAnchor.constraint(equalTo: deviceFrame.trailingAnchor),
            panel.widthAnchor.constraint(equalToConstant: Self.devToolsPanelWidth),
            panel.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
        ])

        self.devToolsPanel = panel
        panel.updateInfo(device: Self.currentDeviceSize, path: currentPath)
    }

    // MARK: - DevTools Toggle

    private func toggleDevTools(show: Bool) {
        isDevToolsVisible = show
        devToolsPanel?.isHidden = !show

        let newSize = Self.calculateWindowSize(
            for: Self.currentDeviceSize,
            devToolsWidth: show ? Self.devToolsPanelWidth : 0
        )
        guard let window = self.window else { return }

        let cur = window.frame
        let newOrigin = NSPoint(x: cur.midX - newSize.width / 2, y: cur.midY - newSize.height / 2)
        let newFrame = NSRect(origin: newOrigin, size: newSize)

        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.25
            ctx.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            window.animator().setFrame(newFrame, display: true)
        }

        if show {
            devToolsPanel?.updateInfo(device: Self.currentDeviceSize, path: currentPath)
            DevToolsLogger.shared.log("DevTools opened", level: .debug)
        }
    }

    private func openInspector() {
        guard let webView = currentInspectableWebView() else {
            os_log("Inspect requested but no active webview was found", log: Self.log, type: .error)
            return
        }

        // Use passRetained to prevent deallocation during the FFI call, then
        // immediately release the extra retain count afterwards.
        let retained = Unmanaged.passRetained(webView)
        let ptr = UInt(bitPattern: retained.toOpaque())
        let ok = toggleWebViewDevtoolsByPtr(ptr, true)
        retained.release()
        if !ok {
            os_log("Failed to toggle web inspector for current webview", log: Self.log, type: .error)
        }
    }

    private func currentInspectableWebView() -> WKWebView? {
        browserOverlay.activeWebView
            ?? RunnerSupport.WebView.current()
            ?? RunnerSupport.WebView.find(appId: appId, path: currentPath)
    }
    
    // MARK: - Phone Content Setup
    
    private func setupPhoneContent(appId: String, path: String) {
        guard let deviceFrame = deviceFrame, let screenContainer = deviceFrame.contentView else { return }
        
        // Create phone content view
        let phoneContent = NSView()
        phoneContent.wantsLayer = true
        phoneContent.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        phoneContent.translatesAutoresizingMaskIntoConstraints = false
        screenContainer.addSubview(phoneContent)
        
        NSLayoutConstraint.activate([
            phoneContent.topAnchor.constraint(equalTo: screenContainer.topAnchor),
            phoneContent.leadingAnchor.constraint(equalTo: screenContainer.leadingAnchor),
            phoneContent.trailingAnchor.constraint(equalTo: screenContainer.trailingAnchor),
            phoneContent.bottomAnchor.constraint(equalTo: screenContainer.bottomAnchor)
        ])
        
        self.phoneContentView = phoneContent
        
        // Create view controller for WebView content
        let vc = CapsuleViewController(appId: appId, path: path)
        viewController = vc
        
        // Add view controller's view to phone content
        vc.view.translatesAutoresizingMaskIntoConstraints = false
        phoneContent.addSubview(vc.view)
        
        NSLayoutConstraint.activate([
            vc.view.topAnchor.constraint(equalTo: phoneContent.topAnchor),
            vc.view.leadingAnchor.constraint(equalTo: phoneContent.leadingAnchor),
            vc.view.trailingAnchor.constraint(equalTo: phoneContent.trailingAnchor),
            vc.view.bottomAnchor.constraint(equalTo: phoneContent.bottomAnchor)
        ])
        
        // Phone UI overlay (status bar, nav bar, floating buttons) — phone only
        if !Self.currentDeviceSize.isDesktop {
            Task { @MainActor [weak self] in
                self?.setupPhoneUIOverlay()
            }
        }
    }
    
    // MARK: - Phone UI Overlay (Status Bar, Nav Bar, Floating Buttons)
    
    private func setupPhoneUIOverlay() {
        guard let phoneContent = phoneContentView else { return }
        
        // Create system status bar and navigation bar
        let statusBar = createSystemStatusBar()
        let navBar = createNavigationBar()
        
        // Setup drag behavior
        setupDragBehavior(statusBar)
        
        phoneContent.addSubview(statusBar)
        phoneContent.addSubview(navBar)
        
        // Setup constraints
        let statusBarHeight = statusBar.heightAnchor.constraint(equalToConstant: Layout.systemStatusBarHeight)
        NSLayoutConstraint.activate([
            statusBar.topAnchor.constraint(equalTo: phoneContent.topAnchor),
            statusBar.leadingAnchor.constraint(equalTo: phoneContent.leadingAnchor),
            statusBar.trailingAnchor.constraint(equalTo: phoneContent.trailingAnchor),
            statusBarHeight,

            navBar.topAnchor.constraint(equalTo: statusBar.bottomAnchor),
            navBar.leadingAnchor.constraint(equalTo: phoneContent.leadingAnchor),
            navBar.trailingAnchor.constraint(equalTo: phoneContent.trailingAnchor),
            navBar.heightAnchor.constraint(equalToConstant: Layout.navBarHeight)
        ])

        self.systemStatusBar = statusBar
        self.statusBarHeightConstraint = statusBarHeight
        self.navigationBar = navBar
        
        // Setup floating capsule buttons
        setupFloatingCapsuleButtons(in: phoneContent)
    }
    
    private func createSystemStatusBar() -> NSView {
        let statusBar = NSView()
        statusBar.wantsLayer = true
        statusBar.layer?.backgroundColor = NSColor.clear.cgColor
        statusBar.translatesAutoresizingMaskIntoConstraints = false
        
        // Time label
        let time = createTimeLabel()
        statusBar.addSubview(time)
        self.timeLabel = time
        
        // Battery view
        let battery = createBatteryView()
        statusBar.addSubview(battery)
        self.batteryView = battery
        
        // Notch view
        let notch = createNotchView()
        statusBar.addSubview(notch)
        self.notchView = notch
        
        // Constraints
        NSLayoutConstraint.activate([
            time.leadingAnchor.constraint(equalTo: statusBar.leadingAnchor, constant: Layout.statusBarSideMargin),
            time.centerYAnchor.constraint(equalTo: statusBar.centerYAnchor),
            
            battery.trailingAnchor.constraint(equalTo: statusBar.trailingAnchor, constant: -Layout.statusBarSideMargin),
            battery.centerYAnchor.constraint(equalTo: statusBar.centerYAnchor),
            
            notch.leadingAnchor.constraint(equalTo: statusBar.leadingAnchor),
            notch.trailingAnchor.constraint(equalTo: statusBar.trailingAnchor),
            notch.topAnchor.constraint(equalTo: statusBar.topAnchor),
            notch.bottomAnchor.constraint(equalTo: statusBar.bottomAnchor)
        ])
        
        return statusBar
    }
    
    private func createNavigationBar() -> NSView {
        let navBar = NSView()
        navBar.wantsLayer = true
        navBar.layer?.backgroundColor = NSColor.clear.cgColor
        navBar.translatesAutoresizingMaskIntoConstraints = false
        return navBar
    }
    
    private func createTimeLabel() -> NSTextField {
        let label = NSTextField()
        label.isEditable = false
        label.isBordered = false
        label.backgroundColor = NSColor.clear
        label.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        label.textColor = NSColor.labelColor
        label.alignment = .left
        label.translatesAutoresizingMaskIntoConstraints = false
        label.stringValue = "9:41"
        return label
    }
    
    private func createBatteryView() -> NSView {
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        
        let outline = NSView()
        outline.wantsLayer = true
        outline.layer?.borderWidth = 1.0
        outline.layer?.borderColor = NSColor.labelColor.cgColor
        outline.layer?.cornerRadius = 2.0
        outline.translatesAutoresizingMaskIntoConstraints = false
        
        let fill = NSView()
        fill.wantsLayer = true
        fill.layer?.backgroundColor = NSColor.systemGreen.cgColor
        fill.layer?.cornerRadius = 1.0
        fill.translatesAutoresizingMaskIntoConstraints = false
        
        let tip = NSView()
        tip.wantsLayer = true
        tip.layer?.backgroundColor = NSColor.labelColor.cgColor
        tip.layer?.cornerRadius = 1.0
        tip.translatesAutoresizingMaskIntoConstraints = false
        
        container.addSubview(outline)
        container.addSubview(fill)
        container.addSubview(tip)
        
        NSLayoutConstraint.activate([
            outline.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            outline.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            outline.widthAnchor.constraint(equalToConstant: 22),
            outline.heightAnchor.constraint(equalToConstant: 11),
            
            fill.leadingAnchor.constraint(equalTo: outline.leadingAnchor, constant: 1),
            fill.trailingAnchor.constraint(equalTo: outline.trailingAnchor, constant: -1),
            fill.topAnchor.constraint(equalTo: outline.topAnchor, constant: 1),
            fill.bottomAnchor.constraint(equalTo: outline.bottomAnchor, constant: -1),
            
            tip.leadingAnchor.constraint(equalTo: outline.trailingAnchor, constant: 1),
            tip.centerYAnchor.constraint(equalTo: outline.centerYAnchor),
            tip.widthAnchor.constraint(equalToConstant: 2),
            tip.heightAnchor.constraint(equalToConstant: 6),
            
            container.widthAnchor.constraint(equalToConstant: 26),
            container.heightAnchor.constraint(equalToConstant: 11)
        ])
        
        return container
    }
    
    private func createNotchView() -> NSView {
        let container = NSView()
        container.wantsLayer = true
        container.translatesAutoresizingMaskIntoConstraints = false
        
        let spec = Layout.currentNotchSpec
        if spec.width > 0 && spec.height > 0 {
            let notch = NSView()
            notch.wantsLayer = true
            notch.layer?.backgroundColor = NSColor.black.cgColor
            notch.layer?.cornerRadius = spec.cornerRadius
            notch.translatesAutoresizingMaskIntoConstraints = false
            
            container.addSubview(notch)
            NSLayoutConstraint.activate([
                notch.centerXAnchor.constraint(equalTo: container.centerXAnchor),
                notch.topAnchor.constraint(equalTo: container.topAnchor),
                notch.widthAnchor.constraint(equalToConstant: spec.width),
                notch.heightAnchor.constraint(equalToConstant: spec.height)
            ])
        }
        
        return container
    }
    
    private func setupDragBehavior(_ statusBar: NSView) {
        let dragView = DraggableView()
        dragView.targetWindow = self.window
        dragView.frame = statusBar.bounds
        dragView.autoresizingMask = [.width, .height]
        statusBar.addSubview(dragView, positioned: .below, relativeTo: nil)
        
        [timeLabel, batteryView, notchView].compactMap { $0 }.forEach {
            statusBar.addSubview($0, positioned: .above, relativeTo: dragView)
        }
    }
    
    private func setupFloatingCapsuleButtons(in contentView: NSView) {
        guard floatingCapsuleContainer == nil else { return }
        
        let capsuleContainer = NSView()
        capsuleContainer.wantsLayer = true
        capsuleContainer.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.92).cgColor
        capsuleContainer.layer?.cornerRadius = Layout.capsuleContainerHeight / 2
        capsuleContainer.translatesAutoresizingMaskIntoConstraints = false
        capsuleContainer.shadow = NSShadow()
        capsuleContainer.layer?.shadowColor = NSColor.black.cgColor
        capsuleContainer.layer?.shadowOpacity = 0.12
        capsuleContainer.layer?.shadowOffset = CGSize(width: 0, height: 1)
        capsuleContainer.layer?.shadowRadius = 4
        
        contentView.addSubview(capsuleContainer)
        
        let buttons = [
            makeButton(image: CapsuleButtonImages.createThreeDotsImage(), action: #selector(moreButtonClicked)),
            makeButton(image: CapsuleButtonImages.createMinimizeButtonImage(), action: #selector(minimizeButtonClicked)),
            makeButton(image: CapsuleButtonImages.createCloseButtonImage(), action: #selector(closeButtonClicked))
        ]
        
        buttons.forEach { capsuleContainer.addSubview($0) }
        
        let navBarCenterOffset = Layout.systemStatusBarHeight + (Layout.navBarHeight - Layout.capsuleContainerHeight) / 2
        NSLayoutConstraint.activate([
            capsuleContainer.topAnchor.constraint(equalTo: contentView.topAnchor, constant: navBarCenterOffset),
            capsuleContainer.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -Layout.capsuleTrailingMargin),
            capsuleContainer.widthAnchor.constraint(equalToConstant: Layout.capsuleContainerWidth),
            capsuleContainer.heightAnchor.constraint(equalToConstant: Layout.capsuleContainerHeight)
        ])
        
        // Layout buttons
        let buttonWidth: CGFloat = 20
        let buttonHeight: CGFloat = 24
        let edgeSpacing: CGFloat = 8
        let buttonSpacing: CGFloat = 8
        let buttonY = (Layout.capsuleContainerHeight - buttonHeight) / 2
        
        for (index, button) in buttons.enumerated() {
            let buttonX = edgeSpacing + CGFloat(index) * (buttonWidth + buttonSpacing)
            button.frame = NSRect(x: buttonX, y: buttonY, width: buttonWidth, height: buttonHeight)
        }
        
        self.floatingCapsuleContainer = capsuleContainer
    }
    
    private func makeButton(image: NSImage?, action: Selector) -> NSButton {
        let button = NSButton()
        button.image = image
        button.imageScaling = .scaleProportionallyDown
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.target = self
        button.action = action
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor
        return button
    }
    
    // MARK: - Device Change Handling
    
    private func handleDeviceChange(_ newDevice: MobileDeviceSize) {
        Self.currentDeviceSize = newDevice
        Layout.currentNotchSpec = newDevice.notchSpec

        DevToolsLogger.shared.log("Device → \(newDevice.displayName) (\(newDevice.sizeDescription))", level: .debug)

        // Resize window (preserve devtools panel if open)
        let newWindowSize = Self.calculateWindowSize(
            for: newDevice,
            devToolsWidth: isDevToolsVisible ? Self.devToolsPanelWidth : 0
        )
        
        guard let window = self.window else { return }
        
        // Calculate new frame centered on current position
        let currentFrame = window.frame
        let newOrigin = NSPoint(
            x: currentFrame.midX - newWindowSize.width / 2,
            y: currentFrame.midY - newWindowSize.height / 2
        )
        let newFrame = NSRect(origin: newOrigin, size: newWindowSize)
        
        // Animate window resize
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.3
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            window.animator().setFrame(newFrame, display: true)
        }
        
        // Update device frame size
        updateDeviceFrameSize(for: newDevice)
        
        // Update notch view
        updateNotchView()
        
        // Update status bar height constraint
        updateStatusBarConstraints()
        
        // Notify RunnerApp about device change
        RunnerApp.shared.setDeviceSize(newDevice)
    }
    
    private func updateDeviceFrameSize(for device: MobileDeviceSize) {
        guard let deviceFrame = deviceFrame else { return }

        deviceFrame.setDeviceSize(device)

        let frameSize = DeviceFrame.frameSize(for: device)
        deviceFrameWidthConstraint?.constant = frameSize.width
        deviceFrameHeightConstraint?.constant = frameSize.height
        deviceFrame.needsLayout = true

        // Refresh devtools info panel
        devToolsPanel?.updateInfo(device: device, path: currentPath)

        // Phone UI overlay: show for phones, hide for desktop
        if device.isDesktop {
            systemStatusBar?.isHidden = true
            navigationBar?.isHidden   = true
            floatingCapsuleContainer?.isHidden = true
        } else {
            systemStatusBar?.isHidden = false
            // navigationBar visibility is managed by updateNavigationBar
            floatingCapsuleContainer?.isHidden = false
            if systemStatusBar == nil, phoneContentView != nil {
                setupPhoneUIOverlay()
            }
        }
    }
    
    private func updateNotchView() {
        guard let systemStatusBar = systemStatusBar else { return }
        
        // Remove old notch view
        notchView?.removeFromSuperview()
        
        // Create new notch view with updated spec
        let newNotch = createNotchView()
        systemStatusBar.addSubview(newNotch)
        
        NSLayoutConstraint.activate([
            newNotch.leadingAnchor.constraint(equalTo: systemStatusBar.leadingAnchor),
            newNotch.trailingAnchor.constraint(equalTo: systemStatusBar.trailingAnchor),
            newNotch.topAnchor.constraint(equalTo: systemStatusBar.topAnchor),
            newNotch.bottomAnchor.constraint(equalTo: systemStatusBar.bottomAnchor)
        ])
        
        self.notchView = newNotch
    }
    
    private func updateStatusBarConstraints() {
        statusBarHeightConstraint?.constant = Layout.systemStatusBarHeight
        systemStatusBar?.needsLayout = true
    }
    
    private func applyInitialNavigationConfiguration() {
        guard let navBar = navigationBar else { return }

        if !appId.isEmpty, !currentPath.isEmpty {
            let navState = RunnerSupport.Navigation.state(appId: appId, path: currentPath)
            updateNavigationBar(with: navState)
        } else {
            navBar.layer?.backgroundColor = NSColor.systemBlue.cgColor
        }
    }
    
    // MARK: - Navigation Bar Update
    
    public func updateNavigationBar(with config: NavigationBarState?) {
        guard let navBar = navigationBar else { return }
        
        navBar.wantsLayer = true
        
        if let config = config {
            let textStyle = config.text_style.toString()
            let bgColor = Self.colorFromARGB(config.background_color)
            let isTransparent = RunnerSupport.TabBar.isTransparent(config.background_color)
            
            if config.show_navbar {
                if isTransparent {
                    navBar.layer?.backgroundColor = NSColor.clear.cgColor
                    systemStatusBar?.layer?.backgroundColor = NSColor.clear.cgColor
                    viewController?.updateTopMargin(0)
                } else {
                    navBar.layer?.backgroundColor = bgColor.cgColor
                    systemStatusBar?.layer?.backgroundColor = bgColor.cgColor
                    viewController?.updateTopMargin(Layout.systemStatusBarHeight + Layout.navBarHeight)
                }
                
                updateStatusBarTextColors(textStyle: textStyle)
                setupNavigationBarTitle(in: navBar, title: config.title_text.toString(), textStyle: textStyle)
                updateNavigationButtons(config, textStyle: textStyle)
                navBar.isHidden = false
            } else {
                navBar.layer?.backgroundColor = NSColor.clear.cgColor
                systemStatusBar?.layer?.backgroundColor = NSColor.clear.cgColor
                updateStatusBarTextColors(textStyle: textStyle)
                navBar.subviews.forEach { $0.removeFromSuperview() }
                navBar.isHidden = false
                viewController?.updateTopMargin(0)
            }
        } else {
            navBar.layer?.backgroundColor = NSColor.clear.cgColor
            systemStatusBar?.layer?.backgroundColor = NSColor.clear.cgColor
            navBar.subviews.forEach { $0.removeFromSuperview() }
            navBar.isHidden = false
            viewController?.updateTopMargin(0)
        }
    }
    
    private static func colorFromARGB(_ argb: UInt32) -> NSColor {
        let alpha = CGFloat((argb >> 24) & 0xFF) / 255.0
        let red = CGFloat((argb >> 16) & 0xFF) / 255.0
        let green = CGFloat((argb >> 8) & 0xFF) / 255.0
        let blue = CGFloat(argb & 0xFF) / 255.0
        return NSColor(red: red, green: green, blue: blue, alpha: alpha)
    }
    
    private func setupNavigationBarTitle(in navBar: NSView, title: String, textStyle: String) {
        navBar.subviews.filter { $0 is NSTextField }.forEach { $0.removeFromSuperview() }
        
        let titleLabel = NSTextField(labelWithString: title)
        titleLabel.font = NSFont.systemFont(ofSize: 16, weight: .medium)
        titleLabel.textColor = textStyle == "white" ? NSColor.white : NSColor.black
        titleLabel.alignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        
        navBar.addSubview(titleLabel)
        
        NSLayoutConstraint.activate([
            titleLabel.centerXAnchor.constraint(equalTo: navBar.centerXAnchor),
            titleLabel.centerYAnchor.constraint(equalTo: navBar.centerYAnchor)
        ])
    }
    
    private func updateNavigationButtons(_ config: NavigationBarState, textStyle: String) {
        guard let navBar = navigationBar else { return }
        
        navBar.subviews.filter { !($0 is NSTextField) }.forEach { $0.removeFromSuperview() }
        
        let buttonSize: CGFloat = 24
        let buttonY: CGFloat = (Layout.navBarHeight - buttonSize) / 2
        let buttonX: CGFloat = 16
        let iconColor = textStyle == "white" ? NSColor.white : NSColor.black
        
        if config.show_back_button, let image = CapsuleButtonImages.createBackButtonImage(color: iconColor) {
            let backButton = makeButton(image: image, action: #selector(backButtonClicked))
            backButton.frame = NSRect(x: buttonX, y: buttonY, width: buttonSize, height: buttonSize)
            navBar.addSubview(backButton)
        } else if config.show_home_button, let image = CapsuleButtonImages.createHomeButtonImage(color: iconColor) {
            let homeButton = makeButton(image: image, action: #selector(homeButtonClicked))
            homeButton.frame = NSRect(x: buttonX, y: buttonY, width: buttonSize, height: buttonSize)
            navBar.addSubview(homeButton)
        }
    }
    
    private func updateStatusBarTextColors(textStyle: String) {
        let textColor = textStyle == "white" ? NSColor.white : NSColor.black
        
        timeLabel?.textColor = textColor
        
        batteryView?.subviews.forEach { subview in
            if subview.layer?.borderColor != nil {
                subview.layer?.borderColor = textColor.cgColor
            }
        }
    }
    
    // MARK: - Button Actions
    
    @objc private func backButtonClicked() {
        let _ = onLxappEvent(appId, LxAppUiEventType.NavigationClick, "back")
    }

    @objc private func homeButtonClicked() {
        let _ = onLxappEvent(appId, LxAppUiEventType.NavigationClick, "home")
    }

    @objc private func moreButtonClicked() {
        RunnerSupport.CapsuleMenu.show(appId: appId)
    }
    
    @objc private func minimizeButtonClicked() {
        window?.miniaturize(nil)
    }
    
    @objc private func closeButtonClicked() {
        let _ = onLxappEvent(appId, LxAppUiEventType.CapsuleClick, "close")
        window?.close()
    }
    
    // MARK: - NSWindowDelegate
    
    public func windowWillClose(_ notification: Notification) {
        browserOverlay.dismiss(closeTab: true)
        if let sessionId = RunnerSupport.Runtime.sessionId(for: appId), sessionId > 0 {
            if !suppressRuntimeCloseNotification {
                let _ = onLxappClosed(appId, sessionId)
                RunnerApp.shared.discardSession(appId: appId, sessionId: sessionId)
            }
            RunnerSupport.Runtime.removeSessionId(for: appId)
        }
        RunnerApp.shared.handleWindowClosed(self)
    }

    func closeFromRuntime() {
        suppressRuntimeCloseNotification = true
        window?.close()
    }

    func presentBrowserTab(id tabId: String) {
        guard let phoneContentView else { return }
        browserOverlay.present(tabId: tabId, in: phoneContentView, window: window)
        window?.makeKeyAndOrderFront(nil)
    }
    
    // MARK: - Navigation
    
    public func navigate(to path: String) {
        navigate(to: path, animationType: .none)
    }
    
    public func navigate(to path: String, animationType: LxAppAnimation) {
        self.currentPath = path

        DevToolsLogger.shared.log("Navigate → \(path)", level: .nav)
        devToolsPanel?.updateInfo(device: Self.currentDeviceSize, path: path)

        let navState = RunnerSupport.Navigation.state(appId: appId, path: path)
        updateNavigationBar(with: navState)

        viewController?.navigate(to: path, animationType: animationType)
    }
}
