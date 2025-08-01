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
        layer?.backgroundColor = NSColor(hexString: TabBarHelper.DEFAULT_BACKGROUND_COLOR)?.cgColor ?? NSColor.white.cgColor
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

        // Apply dimension configuration
        let isVertical = controller.isVertical()
        let dimension = controller.getEffectiveHeight() // This returns the configured dimension

        var constraints = [
            itemsContainer.topAnchor.constraint(equalTo: topAnchor),
            itemsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            itemsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            itemsContainer.bottomAnchor.constraint(equalTo: bottomAnchor)
        ]

        // Apply dimension constraints based on orientation
        if isVertical {
            // For vertical TabBar, apply width dimension
            constraints.append(widthAnchor.constraint(equalToConstant: dimension))
        } else {
            // For horizontal TabBar, apply height dimension
            constraints.append(heightAnchor.constraint(equalToConstant: dimension))
        }

        NSLayoutConstraint.activate(constraints)
    }

    private func setItems(_ newItems: [TabBarItem]) {
        guard let itemsContainer = itemsContainer else { return }

        // Remove existing tab views
        itemsContainer.arrangedSubviews.forEach { $0.removeFromSuperview() }
        tabViews.removeAll()
        tabIndexMap.removeAll()

        // Always use grouped layout - items without group go to center
        setupGroupedLayout(items: newItems, container: itemsContainer)

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
        let labelText = item.text.toString()
        let label = NSTextField(labelWithString: labelText)
        label.font = NSFont.systemFont(ofSize: TabBarConstants.ITEM_FONT_SIZE)
        label.alignment = NSTextAlignment.center
        label.translatesAutoresizingMaskIntoConstraints = false

        stackView.addArrangedSubview(iconView)
        if !labelText.isEmpty {
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

        guard let config = controller.getConfig() else { return }

        // Update colors
        let selectedColor = TabBarHelper.parseColor(config.selected_color.toString()) ?? NSColor(hexString: TabBarHelper.DEFAULT_SELECTED_COLOR) ?? NSColor.systemBlue
        let normalColor = TabBarHelper.parseColor(config.color.toString()) ?? NSColor(hexString: TabBarHelper.DEFAULT_UNSELECTED_COLOR) ?? NSColor.gray

        let color = isSelected ? selectedColor : normalColor

        // Update label color if exists
        if stackView.arrangedSubviews.count > 1,
           let label = stackView.arrangedSubviews[1] as? NSTextField {
            label.textColor = color
        }

        // Load appropriate icon
        let iconPath = isSelected ? item.selected_icon_path.toString() : item.icon_path.toString()
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

    /// Setup universal grouped layout with separate containers for start/center/end items
    /// Works for all TabBar orientations - group property is independent of position
    private func setupGroupedLayout(items: [TabBarItem], container: NSStackView) {
        // Group items directly here to avoid any issues with the config method
        var startItems: [TabBarItem] = []
        var centerItems: [TabBarItem] = []
        var endItems: [TabBarItem] = []

        for item in items {
            switch item.group {
            case 1: // start (top for vertical, left for horizontal)
                startItems.append(item)
            case 2: // end (bottom for vertical, right for horizontal) - recommended for settings
                endItems.append(item)
            default: // 0 or any other value = middle/center (default)
                centerItems.append(item)
            }
        }
        let isVertical = controller.isVertical()

        // Configure container based on orientation
        if isVertical {
            container.orientation = .vertical
            container.distribution = .fill
            container.alignment = .leading
        } else {
            container.orientation = .horizontal
            container.distribution = .fill
            container.alignment = .centerY
        }

        // Check if we have any grouped items (start or end)
        let hasGroupedItems = !startItems.isEmpty || !endItems.isEmpty

        if isVertical && hasGroupedItems {
            // Grouped layout for vertical TabBar

            // Add top spacing for borderless windows to avoid window controls
            let topSpacer = createTopSpacer()
            container.addArrangedSubview(topSpacer)

            // Create start container
            if !startItems.isEmpty {
                let startContainer = createGroupContainer(items: startItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)
                container.addArrangedSubview(startContainer)
            }

            // Create spacer for center items or empty space
            let spacer = NSView()
            spacer.setContentHuggingPriority(.defaultLow, for: .vertical)
            container.addArrangedSubview(spacer)

            // Create center container (middle)
            if !centerItems.isEmpty {
                let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                container.addArrangedSubview(centerContainer)
            }

            // Create another spacer
            let spacer2 = NSView()
            spacer2.setContentHuggingPriority(.defaultLow, for: .vertical)
            container.addArrangedSubview(spacer2)

            // Create end container
            if !endItems.isEmpty {
                let endContainer = createGroupContainer(items: endItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)
                container.addArrangedSubview(endContainer)
            }

        } else {
            // Simple centered layout (no grouping or horizontal TabBar)

            if isVertical {
                // Add top spacing for borderless windows to avoid window controls
                let topSpacer = createTopSpacer()
                container.addArrangedSubview(topSpacer)

                // Add flexible spacer before center items to push them to center
                let spacer = NSView()
                spacer.setContentHuggingPriority(.defaultLow, for: .vertical)
                spacer.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
                container.addArrangedSubview(spacer)
            }

            // Create center container with all items (should be centerItems only when no grouping)
            if !centerItems.isEmpty {
                let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                container.addArrangedSubview(centerContainer)
            }

            if isVertical {
                // Add flexible spacer after center items to keep them centered
                let spacer2 = NSView()
                spacer2.setContentHuggingPriority(.defaultLow, for: .vertical)
                spacer2.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
                container.addArrangedSubview(spacer2)
            }
        }
    }

    /// Create a container for a group of tab items
    private func createGroupContainer(items: [TabBarItem], spacing: CGFloat, isVertical: Bool) -> NSStackView {
        let groupContainer = NSStackView()
        groupContainer.orientation = isVertical ? .vertical : .horizontal
        groupContainer.distribution = isVertical ? .equalSpacing : .fillEqually
        groupContainer.spacing = spacing
        groupContainer.translatesAutoresizingMaskIntoConstraints = false

        for (index, item) in items.enumerated() {
            // Find the global index of this item
            let globalIndex = controller.getItems().firstIndex { $0.page_path.toString() == item.page_path.toString() } ?? index
            let tabView = createTabView(for: item, at: globalIndex)
            tabViews.append(tabView)
            tabIndexMap[tabView] = globalIndex
            groupContainer.addArrangedSubview(tabView)
        }

        return groupContainer
    }

    /// Create top spacer for window controls avoidance
    private func createTopSpacer() -> NSView {
        let spacer = NSView()
        spacer.heightAnchor.constraint(equalToConstant: TabBarConstants.WINDOW_CONTROLS_HEIGHT).isActive = true
        return spacer
    }
}

#endif
