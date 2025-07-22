#if os(macOS)
import AppKit
import WebKit

class macOSLxAppWindowController: NSWindowController, NSWindowDelegate {
    var appId: String
    var path: String
    private var navigationBar: macOSNavigationBar?

    private static var windowWidth: CGFloat = 800
    private static var windowHeight: CGFloat = 600
    private static var windowStyle: LxAppWindowStyle = .systemDefault

    // Capsule button constants
    private static let CAPSULE_BUTTON_WIDTH: CGFloat = 87
    private static let CAPSULE_BUTTON_HEIGHT: CGFloat = 28
    private static let CAPSULE_TOP_MARGIN: CGFloat = 2

    init(appId: String, path: String) {
        self.appId = appId
        self.path = path

        // Configure window based on style
        let styleMask: NSWindow.StyleMask
        switch macOSLxAppWindowController.windowStyle {
        case .systemDefault:
            styleMask = [.titled, .closable, .miniaturizable, .resizable]
        case .customCapsule:
            styleMask = [.titled, .closable, .miniaturizable] // No .resizable for custom style
        case .borderless:
            styleMask = [.titled, .closable, .miniaturizable, .resizable] // Keep .titled to show system buttons
        }

        let window = macOSLxAppWindow(
            contentRect: NSRect(x: 0, y: 0, width: macOSLxAppWindowController.windowWidth, height: macOSLxAppWindowController.windowHeight),
            styleMask: styleMask,
            backing: .buffered,
            defer: false
        )

        // Configure window appearance based on style
        window.configureForStyle(macOSLxAppWindowController.windowStyle)

        window.center()
        window.isReleasedWhenClosed = false

        super.init(window: window)

        self.window?.delegate = self

        let viewController = macOSLxAppViewController(appId: appId, path: path)
        self.window?.contentViewController = viewController

        // Setup notification observers first
        setupNotificationObservers()

        // Setup custom title bar only for customCapsule style
        if macOSLxAppWindowController.windowStyle == .customCapsule {
            DispatchQueue.main.async { [weak self] in
                self?.setupTitleBar()
            }
        }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    static func setWindowSize(width: CGFloat, height: CGFloat) {
        windowWidth = width
        windowHeight = height
    }

    static func setWindowStyle(_ style: LxAppWindowStyle) {
        windowStyle = style
    }

    static func getTopMarginForCurrentStyle() -> CGFloat {
        switch windowStyle {
        case .customCapsule:
            return 32  // Custom capsule style needs space for title bar
        case .systemDefault:
            return 0   // System default style uses system title bar
        case .borderless:
            return 0   // Content fills entire window, system buttons float on top
        }
    }

    func reapplyWindowSize() {
        guard let window = self.window else { return }

        let newSize = NSSize(width: macOSLxAppWindowController.windowWidth, height: macOSLxAppWindowController.windowHeight)
        window.setContentSize(newSize)

        // Configure resizability based on window style
        switch macOSLxAppWindowController.windowStyle {
        case .systemDefault:
            window.styleMask.update(with: .resizable)
        case .customCapsule:
            window.styleMask.remove(.resizable)
        case .borderless:
            window.styleMask.update(with: .resizable)
        }
    }

    func windowWillClose(_ notification: Notification) {
        macOSLxApp.handleAppClosing(appId: appId)
        removeNotificationObservers()
        macOSLxApp.removeWindowController(self)
    }

    // MARK: - Notification Observers

    private var switchPageObserver: NSObjectProtocol?

    private func setupNotificationObservers() {
        // Observe switch page notification to update title
        switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self else { return }
            // Extract values outside of Task to avoid data race
            let appId = notification.userInfo?["appId"] as? String
            let path = notification.userInfo?["path"] as? String

            Task { @MainActor in
                if let appId = appId,
                   let path = path,
                   appId == self.appId {
                    self.updateWindowTitle(for: path)
                }
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
        self.path = path
        guard let navigationBar = self.navigationBar else { return }

        let pageConfig: NavigationBarConfig?
        if let configJson = lingxia.getPageConfig(appId, path)?.toString() {
            pageConfig = NavigationBarConfig.fromJson(configJson)
        } else {
            pageConfig = nil
        }
        navigationBar.updateWithConfig(
            pageConfig: pageConfig,
            isBackNavigation: false,
            disableAnimation: true,
            onBackClickListener: {},
            onAnimationEnd: nil
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

        updateWindowTitle(for: path)

        if Self.windowStyle == .customCapsule {
            setupCapsuleButtons(on: navBar)
        }
    }

    private func setupCapsuleButtons(on titleBarView: NSView) {
        let buttonWidth = Self.CAPSULE_BUTTON_WIDTH / 3
        let buttonHeight = Self.CAPSULE_BUTTON_HEIGHT
        let buttonY = Self.CAPSULE_TOP_MARGIN
        let rightMargin: CGFloat = 7

        // Create buttons with proper images
        let moreButton = createCapsuleButton(
            image: createThreeDotsImage(),
            action: #selector(moreButtonTapped)
        )
        let minimizeButton = createCapsuleButton(
            image: createMinimizeButtonImage(),
            action: #selector(minimizeButtonTapped)
        )
        let closeButton = createCapsuleButton(
            image: createCloseButtonImage(),
            action: #selector(closeWindow)
        )

        // Position buttons
        let startX = Self.windowWidth - Self.CAPSULE_BUTTON_WIDTH - rightMargin
        moreButton.frame = NSRect(x: startX, y: buttonY, width: buttonWidth, height: buttonHeight)
        minimizeButton.frame = NSRect(x: startX + buttonWidth, y: buttonY, width: buttonWidth, height: buttonHeight)
        closeButton.frame = NSRect(x: startX + buttonWidth * 2, y: buttonY, width: buttonWidth, height: buttonHeight)

        // Add separators
        let separatorWidth: CGFloat = 0.5
        let separatorAlpha: CGFloat = 0.15
        let separatorHeight = buttonHeight - 12
        let separatorY = buttonY + 6

        let leftSeparator = NSView(frame: NSRect(
            x: moreButton.frame.maxX - separatorWidth/2,
            y: separatorY,
            width: separatorWidth,
            height: separatorHeight
        ))
        leftSeparator.wantsLayer = true
        leftSeparator.layer?.backgroundColor = NSColor.lightGray.withAlphaComponent(separatorAlpha).cgColor

        let rightSeparator = NSView(frame: NSRect(
            x: minimizeButton.frame.maxX - separatorWidth/2,
            y: separatorY,
            width: separatorWidth,
            height: separatorHeight
        ))
        rightSeparator.wantsLayer = true
        rightSeparator.layer?.backgroundColor = NSColor.lightGray.withAlphaComponent(separatorAlpha).cgColor

        // Add to view
        titleBarView.addSubview(leftSeparator)
        titleBarView.addSubview(rightSeparator)
        titleBarView.addSubview(moreButton)
        titleBarView.addSubview(minimizeButton)
        titleBarView.addSubview(closeButton)

        // Ensure proper layering
        moreButton.layer?.zPosition = 1000
        minimizeButton.layer?.zPosition = 1000
        closeButton.layer?.zPosition = 1000
    }

    private func createCapsuleButton(image: NSImage?, action: Selector) -> NSButton {
        let button = NSButton()
        button.image = image
        button.target = self
        button.action = action
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.translatesAutoresizingMaskIntoConstraints = true
        button.imageScaling = .scaleProportionallyDown
        button.imagePosition = .imageOnly
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor
        button.setButtonType(.momentaryPushIn)
        return button
    }

    @objc private func moreButtonTapped() {
        // More button functionality
    }

    @objc private func minimizeButtonTapped() {
        window?.miniaturize(nil)
    }

    @objc private func closeWindow() {
        guard let window = window else { return }
        window.close()

        // Check if this was the last window and quit app if needed
        DispatchQueue.main.async {
            if NSApplication.shared.windows.filter({ $0.isVisible }).isEmpty {
                NSApplication.shared.terminate(nil)
            }
        }
    }

    private func createThreeDotsImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setFillColor(NSColor.darkGray.cgColor)

            let centerY = size.height / 2
            let centerX = size.width / 2
            let centerDotRadius = size.height / 7
            let sideDotRadius = size.height / 10
            let spacing = centerDotRadius * 2.8

            // Left dot
            let leftDotRect = CGRect(
                x: centerX - spacing - sideDotRadius,
                y: centerY - sideDotRadius,
                width: sideDotRadius * 2,
                height: sideDotRadius * 2
            )
            context.fillEllipse(in: leftDotRect)

            // Right dot
            let rightDotRect = CGRect(
                x: centerX + spacing - sideDotRadius,
                y: centerY - sideDotRadius,
                width: sideDotRadius * 2,
                height: sideDotRadius * 2
            )
            context.fillEllipse(in: rightDotRect)

            // Center dot
            let centerDotRect = CGRect(
                x: centerX - centerDotRadius,
                y: centerY - centerDotRadius,
                width: centerDotRadius * 2,
                height: centerDotRadius * 2
            )
            context.fillEllipse(in: centerDotRect)
        }

        image.unlockFocus()
        return image
    }

    private func createMinimizeButtonImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setLineWidth(3.5)
            context.setLineCap(.round)
            context.setStrokeColor(NSColor.darkGray.cgColor)

            let lineWidth: CGFloat = 10
            context.move(to: CGPoint(x: (size.width - lineWidth) / 2, y: size.height / 2))
            context.addLine(to: CGPoint(x: (size.width + lineWidth) / 2, y: size.height / 2))
            context.strokePath()
        }

        image.unlockFocus()
        return image
    }

    private func createCloseButtonImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            let centerX = size.width / 2
            let centerY = size.height / 2
            let outerRadius = size.width * 0.35
            let innerRadius: CGFloat = 2.5

            context.setLineWidth(2.2)
            context.setStrokeColor(NSColor.darkGray.cgColor)
            context.setLineCap(.round)

            let outerCircle = CGRect(
                x: centerX - outerRadius,
                y: centerY - outerRadius,
                width: outerRadius * 2,
                height: outerRadius * 2
            )
            context.strokeEllipse(in: outerCircle)

            context.setFillColor(NSColor.darkGray.cgColor)
            let innerCircle = CGRect(
                x: centerX - innerRadius,
                y: centerY - innerRadius,
                width: innerRadius * 2,
                height: innerRadius * 2
            )
            context.fillEllipse(in: innerCircle)
        }

        image.unlockFocus()
        return image
    }

}

#endif
