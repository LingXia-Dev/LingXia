#if os(macOS)
import Cocoa
import Foundation

/// macOS TabBar implementation using shared controller
@MainActor
public class macOSTabBar: NSView, EnhancedTabBarProtocol, TabBarUIDelegate {
    public let controller = TabBarController()
    public weak var uiDelegate: TabBarUIDelegate?
    private var tabViews = [NSView]()
    private var itemsContainer: NSStackView?
    private var tabIndexMap = [NSView: Int]()

    public override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupUI()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupUI()
    }

    private func setupUI() {
        isHidden = true
        wantsLayer = true
        layer?.backgroundColor = NSColor(hexString: TabBarConfig.DEFAULT_BACKGROUND_COLOR)?.cgColor ?? NSColor.white.cgColor
        uiDelegate = self

        itemsContainer = NSStackView()
        updateItemsContainerLayout()
        performLayoutForPosition()
    }

    public func updateTabSelection(selectedIndex: Int) {
        updateTabSelection()
    }

    public func updateConfiguration() {
        updateItemsContainerLayout()
        performLayoutForPosition()
        isHidden = !controller.shouldBeVisible()
    }

    public func updateItems(_ items: [TabBarItem]) {
        setItems(items)
        isHidden = !controller.shouldBeVisible()
    }

    public func forceTransparencyMode() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
    }

    private func updateItemsContainerLayout() {
        guard let itemsContainer = itemsContainer else { return }

        let isVerticalTabBar = controller.isVertical()

        itemsContainer.orientation = isVerticalTabBar ? .vertical : .horizontal
        itemsContainer.distribution = .fillEqually
        itemsContainer.alignment = .centerY

        if controller.shouldUseTransparentBackground() {
            itemsContainer.wantsLayer = true
            itemsContainer.layer?.backgroundColor = NSColor.clear.cgColor
        } else {
            let backgroundColor = controller.getResolvedBackgroundColor()
            itemsContainer.wantsLayer = true
            itemsContainer.layer?.backgroundColor = backgroundColor.cgColor
        }
    }

    private func performLayoutForPosition() {
        subviews.forEach { $0.removeFromSuperview() }

        guard let itemsContainer = itemsContainer else { return }

        addSubview(itemsContainer)
        setupContainerConstraints()
    }

    private func setupContainerConstraints() {
        guard let itemsContainer = itemsContainer else { return }

        itemsContainer.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            itemsContainer.topAnchor.constraint(equalTo: topAnchor),
            itemsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            itemsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            itemsContainer.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }

    private func setItems(_ newItems: [TabBarItem]) {
        guard let itemsContainer = itemsContainer else { return }

        // Remove existing tab views
        itemsContainer.arrangedSubviews.forEach { $0.removeFromSuperview() }
        tabViews.removeAll()
        tabIndexMap.removeAll()

        // Create new tab views
        for (index, item) in newItems.enumerated() {
            let tabView = createTabView(for: item, at: index)
            tabViews.append(tabView)
            tabIndexMap[tabView] = index
            itemsContainer.addArrangedSubview(tabView)
        }

        // Set initial selection
        let selectedPosition = controller.getSelectedPosition()
        if selectedPosition >= 0 && selectedPosition < newItems.count {
            updateTabSelection()
        }
    }

    private func createTabView(for item: TabBarItem, at index: Int) -> NSView {
        let tabView = NSView()
        tabView.wantsLayer = true
        tabView.layer?.backgroundColor = NSColor.clear.cgColor

        let stackView = NSStackView()
        stackView.orientation = .vertical
        stackView.alignment = .centerX
        stackView.spacing = 4
        stackView.translatesAutoresizingMaskIntoConstraints = false

        // Create icon
        let iconView = NSImageView()
        iconView.imageScaling = .scaleProportionallyUpOrDown
        iconView.translatesAutoresizingMaskIntoConstraints = false

        // Create label
        let label = NSTextField(labelWithString: item.text ?? "")
        label.font = NSFont.systemFont(ofSize: TabBarConstants.ITEM_FONT_SIZE)
        label.alignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false

        stackView.addArrangedSubview(iconView)
        if item.text != nil && !item.text!.isEmpty {
            stackView.addArrangedSubview(label)
        }

        tabView.addSubview(stackView)

        NSLayoutConstraint.activate([
            stackView.centerXAnchor.constraint(equalTo: tabView.centerXAnchor),
            stackView.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: TabBarConstants.ICON_SIZE),
            iconView.heightAnchor.constraint(equalToConstant: TabBarConstants.ICON_SIZE)
        ])

        // Add click gesture
        let clickGesture = NSClickGestureRecognizer(target: self, action: #selector(tabClicked(_:)))
        tabView.addGestureRecognizer(clickGesture)

        return tabView
    }

    @objc private func tabClicked(_ gesture: NSClickGestureRecognizer) {
        guard let tabView = gesture.view,
              let index = tabIndexMap[tabView] else { return }
        controller.handleTabSelection(at: index)
    }

    private func updateTabSelection() {
        let items = controller.getItems()
        let selectedPosition = controller.getSelectedPosition()

        for (index, tabView) in tabViews.enumerated() {
            let isSelected = index == selectedPosition
            if index < items.count {
                updateTabAppearance(tabView: tabView, isSelected: isSelected, item: items[index])
            }
        }
    }

    private func updateTabAppearance(tabView: NSView, isSelected: Bool, item: TabBarItem) {
        guard let stackView = tabView.subviews.first as? NSStackView,
              let iconView = stackView.arrangedSubviews.first as? NSImageView else { return }

        let config = controller.getConfig()

        // Update colors
        let selectedColor = config.parseColor(config.selectedColor) ?? NSColor(hexString: TabBarConfig.DEFAULT_SELECTED_COLOR) ?? NSColor.systemBlue
        let normalColor = config.parseColor(config.color) ?? NSColor(hexString: TabBarConfig.DEFAULT_UNSELECTED_COLOR) ?? NSColor.gray

        let color = isSelected ? selectedColor : normalColor

        // Update label color if exists
        if stackView.arrangedSubviews.count > 1,
           let label = stackView.arrangedSubviews[1] as? NSTextField {
            label.textColor = color
        }

        // Load appropriate icon
        let iconPath = isSelected ? item.selectedIconPath : item.iconPath
        loadIcon(for: iconView, iconPath: iconPath, tintColor: color)
    }

    private func loadIcon(for imageView: NSImageView, iconPath: String, tintColor: NSColor) {
        // Simple icon loading - you can enhance this with proper image loading
        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            if #available(macOS 11.0, *) {
                imageView.image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)
            }
        } else {
            imageView.image = NSImage(named: iconPath)
        }

        // Apply tint color
        imageView.contentTintColor = tintColor
    }
}

#endif
