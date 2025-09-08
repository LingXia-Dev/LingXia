#if os(macOS)
import SwiftUI
import AppKit

/// Tab-style tab view for macOS - AppKit implementation with proper layout
@MainActor
public class LxAppTabView: NSView {
    private var tabManager: LxAppTabManager
    private var tabViews: [String: NSView] = [:]
    private var windowControlsView: NSView?

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
        let originalCallback = tabManager.onTabChanged
        tabManager.onTabChanged = { [weak self] tab in
            originalCallback?(tab)
            self?.refreshTabs()
        }
        refreshTabs()
    }

    private func refreshTabs() {
        subviews.forEach { view in
            if view != windowControlsView {
                view.removeFromSuperview()
            }
        }
        tabViews.removeAll()

        let tabs = tabManager.tabs
        guard !tabs.isEmpty else { return }

        var currentX: CGFloat = 70

        let homeTabs = tabs.filter { LxAppCore.isHomeLxApp($0.appId) }
        let regularTabs = tabs.filter { !LxAppCore.isHomeLxApp($0.appId) }

        // Home tabs
        for tab in homeTabs {
            let tabView = createTabView(for: tab, width: 40)
            tabView.frame = NSRect(x: currentX, y: 0, width: 40, height: 32)
            addSubview(tabView)
            tabViews[tab.appId] = tabView
            currentX += 40
        }

        // Separator
        if !homeTabs.isEmpty && !regularTabs.isEmpty {
            let separator = createSeparator()
            separator.frame = NSRect(x: currentX, y: 4, width: 1, height: 24)
            addSubview(separator)
            currentX += 9
        }

        // Regular tabs with dynamic width
        if !regularTabs.isEmpty {
            let availableWidth = bounds.width - currentX
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

        let isHomeLxApp = LxAppCore.isHomeLxApp(tab.appId)

        if !isHomeLxApp {
            let isActive = (tabManager.activeTab?.appId == tab.appId)
            setupTabStyle(tabView, isActive: isActive)
        }

        tabView.identifier = NSUserInterfaceItemIdentifier(tab.appId)

        let clickGesture = TabClickGestureRecognizer(target: self, action: #selector(tabClicked(_:)))
        tabView.addGestureRecognizer(clickGesture)

        if isHomeLxApp {
            let homeIcon = createHomeIcon()
            homeIcon.translatesAutoresizingMaskIntoConstraints = false
            tabView.addSubview(homeIcon)

            NSLayoutConstraint.activate([
                homeIcon.centerXAnchor.constraint(equalTo: tabView.centerXAnchor),
                homeIcon.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),
                homeIcon.widthAnchor.constraint(equalToConstant: 16),
                homeIcon.heightAnchor.constraint(equalToConstant: 16)
            ])
        } else {
            let maxLength = 10
            let truncatedTitle = tab.title.count > maxLength ? String(tab.title.prefix(maxLength - 1)) + "…" : tab.title
            let titleLabel = NSTextField(labelWithString: truncatedTitle)
            let isActive = (tabManager.activeTab?.appId == tab.appId)

            titleLabel.font = isActive ? NSFont.systemFont(ofSize: 13, weight: .semibold) : NSFont.systemFont(ofSize: 13)
            titleLabel.textColor = isActive ? NSColor.labelColor : NSColor.secondaryLabelColor
            titleLabel.lineBreakMode = .byTruncatingTail
            titleLabel.translatesAutoresizingMaskIntoConstraints = false
            titleLabel.toolTip = tab.title
            tabView.addSubview(titleLabel)

            if tab.isClosable {
                let closeButton = createCloseButton(for: tab)
                closeButton.isHidden = !isActive
                tabView.addSubview(closeButton)

                NSLayoutConstraint.activate([
                    titleLabel.leadingAnchor.constraint(equalTo: tabView.leadingAnchor, constant: 12),
                    titleLabel.trailingAnchor.constraint(equalTo: closeButton.leadingAnchor, constant: -8),
                    titleLabel.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),
                    closeButton.trailingAnchor.constraint(equalTo: tabView.trailingAnchor, constant: -8),
                    closeButton.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),
                    closeButton.widthAnchor.constraint(equalToConstant: 16),
                    closeButton.heightAnchor.constraint(equalToConstant: 16)
                ])
            } else {
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
        return button
    }

    private func setupTabStyle(_ tabView: NSView, isActive: Bool) {
        guard let layer = tabView.layer else { return }
        layer.cornerRadius = 6

        if isActive {
            layer.backgroundColor = NSColor.controlBackgroundColor.cgColor
            layer.shadowColor = NSColor.black.cgColor
            layer.shadowOpacity = 0.2
            layer.shadowOffset = CGSize(width: 0, height: 2)
            layer.shadowRadius = 4
        } else {
            layer.backgroundColor = NSColor.controlBackgroundColor.withAlphaComponent(0.1).cgColor
            layer.shadowOpacity = 0
        }
    }

    private func createHomeIcon() -> NSImageView {
        let imageView = NSImageView()
        imageView.image = NSImage(systemSymbolName: "house.fill", accessibilityDescription: "Home")
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
            context.setLineWidth(1.8)
            context.setStrokeColor(NSColor.labelColor.withAlphaComponent(0.6).cgColor)
            context.setLineCap(.round)

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

    @objc private func tabClicked(_ sender: NSClickGestureRecognizer) {
        guard let tabView = sender.view,
              let identifier = tabView.identifier else { return }

        let appId = identifier.rawValue
        tabManager.selectTab(appId: appId)
        onTabSelected?(appId)
    }

    @objc private func closeButtonClicked(_ sender: NSButton) {
        guard let identifier = sender.identifier?.rawValue,
              identifier.hasPrefix("close_"),
              let appId = identifier.components(separatedBy: "_").last else { return }

        onTabClosed?(appId)
    }

    public override func layout() {
        super.layout()
        refreshTabs()
    }
}

/// Custom click gesture recognizer
@MainActor
private class TabClickGestureRecognizer: NSClickGestureRecognizer {
    override func mouseDown(with event: NSEvent) {
        guard let view = self.view else {
            super.mouseDown(with: event)
            return
        }

        let locationInView = view.convert(event.locationInWindow, from: nil)
        for subview in view.subviews {
            if let button = subview as? NSButton, button.frame.contains(locationInView) {
                return
            }
        }
        super.mouseDown(with: event)
    }
}

#endif
