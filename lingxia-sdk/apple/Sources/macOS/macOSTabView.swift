#if os(macOS)
import Cocoa
import Foundation

/// Tab-style tab view for macOS - manages application-level tabs
@MainActor
public class macOSTabView: NSView {
    private var tabManager: LxAppTabManager
    private var tabViews: [String: NSView] = [:]  // appId -> NSView
    private var windowControlsView: NSView?

    // Callbacks - simplified to use appId
    public var onTabSelected: ((String) -> Void)?
    public var onTabClosed: ((String) -> Void)?

    public init(tabManager: LxAppTabManager) {
        self.tabManager = tabManager
        super.init(frame: .zero)
        setupView()
        setupObservers()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupView() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor

        setupWindowControls()
        refreshTabs()
    }

    private func setupWindowControls() {
        windowControlsView = NSView()
        guard let controlsView = windowControlsView else { return }

        controlsView.translatesAutoresizingMaskIntoConstraints = false
        controlsView.wantsLayer = true
        controlsView.layer?.backgroundColor = NSColor.clear.cgColor
        addSubview(controlsView)

        NSLayoutConstraint.activate([
            controlsView.leadingAnchor.constraint(equalTo: leadingAnchor),
            controlsView.topAnchor.constraint(equalTo: topAnchor),
            controlsView.bottomAnchor.constraint(equalTo: bottomAnchor),
            controlsView.widthAnchor.constraint(equalToConstant: 70)
        ])
    }

    private func setupObservers() {
        // Listen to tab manager changes directly
        let originalCallback = tabManager.onTabChanged
        tabManager.onTabChanged = { [weak self] tab in
            // Call original callback first
            originalCallback?(tab)
            // Then refresh our UI
            self?.refreshTabs()
        }

        // Initial refresh
        refreshTabs()
    }

    // Tab Management
    private func refreshTabs() {
        // Remove existing tab views
        subviews.forEach { view in
            if view != windowControlsView {
                view.removeFromSuperview()
            }
        }
        tabViews.removeAll()

        let tabs = tabManager.tabs
        guard !tabs.isEmpty else { return }

        var currentX: CGFloat = 70
        let homeLxAppId = LxAppCore.getHomeLxAppId()

        // Separate home and regular tabs
        let homeTabs = tabs.filter { $0.appId == homeLxAppId }
        let regularTabs = tabs.filter { $0.appId != homeLxAppId }

        // Layout home tabs first (fixed width)
        for tab in homeTabs {
            let tabView = createTabView(for: tab, width: 40)
            tabView.frame = NSRect(x: currentX, y: 0, width: 40, height: 32)
            addSubview(tabView)
            tabViews[tab.appId] = tabView
            currentX += 40
        }

        // Add separator if we have both home and regular tabs
        if !homeTabs.isEmpty && !regularTabs.isEmpty {
            let separator = createSeparator()
            separator.frame = NSRect(x: currentX, y: 4, width: 1, height: 24)
            addSubview(separator)
            currentX += 9
        }

        // Layout regular tabs (optimized width)
        if !regularTabs.isEmpty {
            let availableWidth = bounds.width - currentX
            // Much smaller tab width: min 80pt, max 160pt for better space usage
            let tabWidth = min(160, max(80, availableWidth / CGFloat(regularTabs.count)))

            for tab in regularTabs {
                let tabView = createTabView(for: tab, width: tabWidth)
                tabView.frame = NSRect(x: currentX, y: 0, width: tabWidth, height: 32)
                addSubview(tabView)
                tabViews[tab.appId] = tabView
                currentX += tabWidth
            }
        }
    }

    private func createTabView(for tab: LxAppTab, width: CGFloat) -> NSView {
        let tabView = NSView()
        tabView.wantsLayer = true

        let isHomeLxApp = tab.appId == LxAppCore.getHomeLxAppId()

        // Only apply tab-style to non-home tabs
        if !isHomeLxApp {
            let isActive = (tabManager.activeTab?.appId == tab.appId)
            setupTabStyle(tabView, isActive: isActive)
        }

        // Store appId for identification
        tabView.identifier = NSUserInterfaceItemIdentifier(tab.appId)

        // Add click handling using a custom gesture recognizer that doesn't interfere with buttons
        let clickGesture = TabClickGestureRecognizer(target: self, action: #selector(tabClicked(_:)))
        tabView.addGestureRecognizer(clickGesture)

        // Add tracking area for hover effects
        let trackingArea = NSTrackingArea(
            rect: tabView.bounds,
            options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: ["appId": tab.appId]
        )
        tabView.addTrackingArea(trackingArea)

        if isHomeLxApp {
            // Home tab: show house icon
            let homeIcon = createHomeIcon()
            homeIcon.translatesAutoresizingMaskIntoConstraints = false
            tabView.addSubview(homeIcon)

            // Center the home icon
            NSLayoutConstraint.activate([
                homeIcon.centerXAnchor.constraint(equalTo: tabView.centerXAnchor),
                homeIcon.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),
                homeIcon.widthAnchor.constraint(equalToConstant: 16),
                homeIcon.heightAnchor.constraint(equalToConstant: 16)
            ])
        } else {
            // Regular tab: show title text and close button
            // Truncate title for better UI - max 10 characters for smaller tabs
            let maxLength = 10
            let truncatedTitle = tab.title.count > maxLength ? String(tab.title.prefix(maxLength - 1)) + "…" : tab.title
            let titleLabel = NSTextField(labelWithString: truncatedTitle)
            let isActive = (tabManager.activeTab?.appId == tab.appId)

            // Strong font and color contrast for clear visibility
            titleLabel.font = isActive ? NSFont.systemFont(ofSize: 13, weight: .semibold) : NSFont.systemFont(ofSize: 13)
            titleLabel.textColor = isActive ? NSColor.labelColor : NSColor.secondaryLabelColor
            titleLabel.lineBreakMode = NSLineBreakMode.byTruncatingTail
            titleLabel.translatesAutoresizingMaskIntoConstraints = false

            // Add tooltip with full title
            titleLabel.toolTip = tab.title
            tabView.addSubview(titleLabel)

            // Create close button if closable
            if tab.isClosable {
                let closeButton = createCloseButton(for: tab)
                closeButton.isHidden = !isActive

                // Add close button and ensure it's on top
                tabView.addSubview(closeButton)

                // Make sure the button is clickable
                closeButton.wantsLayer = true

                // Setup constraints with close button
                NSLayoutConstraint.activate([
                    // Title label
                    titleLabel.leadingAnchor.constraint(equalTo: tabView.leadingAnchor, constant: 12),
                    titleLabel.trailingAnchor.constraint(equalTo: closeButton.leadingAnchor, constant: -8),
                    titleLabel.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),

                    // Close button
                    closeButton.trailingAnchor.constraint(equalTo: tabView.trailingAnchor, constant: -8),
                    closeButton.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),
                    closeButton.widthAnchor.constraint(equalToConstant: 16),
                    closeButton.heightAnchor.constraint(equalToConstant: 16)
                ])
            } else {
                // No close button, title takes full width
                NSLayoutConstraint.activate([
                    titleLabel.leadingAnchor.constraint(equalTo: tabView.leadingAnchor, constant: 12),
                    titleLabel.trailingAnchor.constraint(equalTo: tabView.trailingAnchor, constant: -12),
                    titleLabel.centerYAnchor.constraint(equalTo: tabView.centerYAnchor)
                ])
            }
        }

        return tabView
    }

    private func createCloseButton(for tab: LxAppTab) -> NSButton {
        let button = NSButton()
        button.image = createCloseButtonImage()
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.translatesAutoresizingMaskIntoConstraints = false
        button.target = self
        button.action = #selector(closeButtonClicked(_:))
        button.identifier = NSUserInterfaceItemIdentifier("close_\(tab.appId)")

        // Simple close button appearance
        button.wantsLayer = true
        button.layer?.zPosition = 100

        return button
    }

    private func setupTabStyle(_ tabView: NSView, isActive: Bool) {
        guard let layer = tabView.layer else { return }

        // Simple, clean rounded corners
        layer.cornerRadius = 6

        if isActive {
            // Active tab: clean bright background for clear distinction
            layer.backgroundColor = NSColor.controlBackgroundColor.cgColor

            // No border for clean look
            layer.borderWidth = 0

            // Strong shadow for depth and definition
            layer.shadowColor = NSColor.black.cgColor
            layer.shadowOpacity = 0.2
            layer.shadowOffset = CGSize(width: 0, height: 2)
            layer.shadowRadius = 4
        } else {
            // Inactive tab: very subtle background
            layer.backgroundColor = NSColor.controlBackgroundColor.withAlphaComponent(0.1).cgColor
            layer.borderWidth = 0
            layer.shadowOpacity = 0
        }
    }

    private func createHomeIcon() -> NSImageView {
        let imageView = NSImageView()

        // Use system home icon - will be replaced with lxapp icon when configuration supports it
        if #available(macOS 11.0, *) {
            imageView.image = NSImage(systemSymbolName: "house.fill", accessibilityDescription: "Home")
        } else {
            // Fallback for older macOS versions
            imageView.image = NSImage(named: NSImage.homeTemplateName)
        }

        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
        return imageView
    }

    private func createSeparator() -> NSView {
        let separator = NSView()
        separator.wantsLayer = true
        separator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        return separator
    }

    private func createCloseButtonImage() -> NSImage {
        let size = CGSize(width: 16, height: 16)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setLineWidth(1.8)  // Good visibility but not too thick
            context.setStrokeColor(NSColor.labelColor.withAlphaComponent(0.6).cgColor)
            context.setLineCap(.round)
            context.setLineJoin(.round)

            // Clean X with good proportions
            let margin: CGFloat = 4
            context.move(to: CGPoint(x: margin, y: margin))
            context.addLine(to: CGPoint(x: size.width - margin, y: size.height - margin))
            context.move(to: CGPoint(x: size.width - margin, y: margin))
            context.addLine(to: CGPoint(x: margin, y: size.height - margin))
            context.strokePath()
        }

        image.unlockFocus()
        return image
    }

    // Actions
    @objc private func tabClicked(_ sender: NSClickGestureRecognizer) {
        guard let tabView = sender.view,
              let identifier = tabView.identifier else {
            return
        }

        let appId = identifier.rawValue
        tabManager.selectTab(appId: appId)
    }

    @objc private func closeButtonClicked(_ sender: NSButton) {
        guard let identifier = sender.identifier?.rawValue,
              identifier.hasPrefix("close_"),
              let appId = identifier.components(separatedBy: "_").last else {
            return
        }

        onTabClosed?(appId)
    }

    // Layout
    public override func layout() {
        super.layout()
        refreshTabs()
    }

    // Mouse Tracking for hover effects
    public override func mouseEntered(with event: NSEvent) {
        super.mouseEntered(with: event)

        if let userInfo = event.trackingArea?.userInfo,
           let appId = userInfo["appId"] as? String,
           let tabView = tabViews[appId] {

            // Add hover effect to tab
            if let layer = tabView.layer {
                let isActive = (tabManager.activeTab?.appId == appId)
                if isActive {
                    // Active tab hover: slightly brighter with accent color
                    layer.backgroundColor = NSColor.controlBackgroundColor.blended(withFraction: 0.1, of: NSColor.controlAccentColor)?.cgColor ?? NSColor.controlBackgroundColor.cgColor
                } else {
                    // Inactive tab hover: more visible
                    layer.backgroundColor = NSColor.controlBackgroundColor.withAlphaComponent(0.5).cgColor
                }
            }
        }
    }

    public override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)

        if let userInfo = event.trackingArea?.userInfo,
           let appId = userInfo["appId"] as? String,
           let tabView = tabViews[appId] {

            // Remove hover effect
            if let layer = tabView.layer {
                let isActive = (tabManager.activeTab?.appId == appId)
                if isActive {
                    layer.backgroundColor = NSColor.controlBackgroundColor.cgColor
                } else {
                    layer.backgroundColor = NSColor.controlBackgroundColor.withAlphaComponent(0.1).cgColor
                }
            }
        }
    }
}

/// Custom click gesture recognizer that doesn't interfere with button clicks
@MainActor
private class TabClickGestureRecognizer: NSClickGestureRecognizer {

    override func mouseDown(with event: NSEvent) {
        // Check if the click is on a button
        guard let view = self.view else {
            super.mouseDown(with: event)
            return
        }

        let locationInView = view.convert(event.locationInWindow, from: nil)

        // Check all subviews to see if click is on a button
        for subview in view.subviews {
            if let button = subview as? NSButton {
                if button.frame.contains(locationInView) {
                    // Don't handle this click, let the button handle it
                    return
                }
            }
        }
        super.mouseDown(with: event)
    }
}
#endif
