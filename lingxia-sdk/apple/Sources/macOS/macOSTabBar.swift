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

    /// Creates a TabBar for macOS
    public static func createTabBar(frame: CGRect) -> macOSTabBar {
        return macOSTabBar(frame: NSRect(x: frame.origin.x, y: frame.origin.y, width: frame.width, height: frame.height))
    }

    /// Configures tab bar positioning for macOS layout
    public static func configureTabBarLayout(_ tabBar: macOSTabBar, position: Int32, containerFrame: CGRect) {
        let tabBarHeight = tabBar.controller.getEffectiveHeight()
        var tabBarFrame: NSRect

        switch position {
        case 0: // bottom
            tabBarFrame = NSRect(x: 0, y: 0, width: containerFrame.width, height: tabBarHeight)
        case 1: // top
            tabBarFrame = NSRect(x: 0, y: containerFrame.height - tabBarHeight, width: containerFrame.width, height: tabBarHeight)
        case 2: // left
            tabBarFrame = NSRect(x: 0, y: 0, width: tabBarHeight, height: containerFrame.height)
        case 3: // right
            tabBarFrame = NSRect(x: containerFrame.width - tabBarHeight, y: 0, width: tabBarHeight, height: containerFrame.height)
        default:
            tabBarFrame = NSRect(x: 0, y: 0, width: containerFrame.width, height: tabBarHeight)
        }

        tabBar.frame = tabBarFrame
    }

    /// Gets the appropriate content area frame considering tab bar position
    public static func getContentAreaFrame(containerFrame: CGRect, tabBarPosition: Int32, hasTabBar: Bool, tabBarHeight: CGFloat = 40) -> CGRect {
        guard hasTabBar else { return containerFrame }

        switch tabBarPosition {
        case 0: // bottom
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case 1: // top
            return CGRect(x: 0, y: 0, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case 2: // left
            return CGRect(x: tabBarHeight, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        case 3: // right
            return CGRect(x: 0, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        default:
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        }
    }

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
        layer?.backgroundColor = NSColor.white.cgColor
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

        // Update colors - config should always have values due to Rust defaults
        let selectedColor = PlatformColor(argb: config.selected_color)
        let normalColor = PlatformColor(argb: config.color)

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
            container.distribution = .fillEqually
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
            // Simple centered layout for vertical without grouping
            if isVertical {
                // Add top spacing for borderless windows to avoid window controls
                let topSpacer = createTopSpacer()
                container.addArrangedSubview(topSpacer)

                // Add flexible spacer before center items to push them to center
                let spacer = NSView()
                spacer.setContentHuggingPriority(.defaultLow, for: .vertical)
                spacer.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
                container.addArrangedSubview(spacer)

                // Create center container with all items
                if !centerItems.isEmpty {
                    let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                    container.addArrangedSubview(centerContainer)
                }

                // Add flexible spacer after center items to keep them centered
                let spacer2 = NSView()
                spacer2.setContentHuggingPriority(.defaultLow, for: .vertical)
                spacer2.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
                container.addArrangedSubview(spacer2)
            } else {
                // Horizontal TabBar (bottom/top) - PROPER GROUPED LAYOUT

                // Clear container
                container.arrangedSubviews.forEach { $0.removeFromSuperview() }

                // Create main horizontal stack
                let mainStack = NSStackView()
                mainStack.orientation = .horizontal
                mainStack.distribution = .fill
                mainStack.alignment = .centerY
                mainStack.spacing = 0

                // Create containers for each group
                let startContainer = createGroupContainer(items: startItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)
                let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                let endContainer = createGroupContainer(items: endItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)

                // Create flexible spacers
                let leftSpacer = NSView()
                let centerSpacer = NSView()
                let rightSpacer = NSView()

                // Configure spacers
                leftSpacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
                centerSpacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
                rightSpacer.setContentHuggingPriority(.defaultLow, for: .horizontal)

                leftSpacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
                centerSpacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
                rightSpacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

                // Configure containers to maintain their size
                startContainer.setContentHuggingPriority(NSLayoutConstraint.Priority.required, for: NSLayoutConstraint.Orientation.horizontal)
                centerContainer.setContentHuggingPriority(NSLayoutConstraint.Priority.required, for: NSLayoutConstraint.Orientation.horizontal)
                endContainer.setContentHuggingPriority(NSLayoutConstraint.Priority.required, for: NSLayoutConstraint.Orientation.horizontal)

                // Add padding views for edges
                let leftPadding = NSView()
                let rightPadding = NSView()
                leftPadding.widthAnchor.constraint(equalToConstant: 8).isActive = true
                rightPadding.widthAnchor.constraint(equalToConstant: 8).isActive = true

                // Build the layout based on which groups exist
                if !startItems.isEmpty && !centerItems.isEmpty && !endItems.isEmpty {
                    // All three groups: [start][spacer][center][spacer][end]
                    mainStack.addArrangedSubview(startContainer)
                    mainStack.addArrangedSubview(leftSpacer)
                    mainStack.addArrangedSubview(centerContainer)
                    mainStack.addArrangedSubview(rightSpacer)
                    mainStack.addArrangedSubview(endContainer)
                } else if !startItems.isEmpty && !centerItems.isEmpty {
                    // Start and center: [start][spacer][center]
                    mainStack.addArrangedSubview(startContainer)
                    mainStack.addArrangedSubview(leftSpacer)
                    mainStack.addArrangedSubview(centerContainer)
                } else if !startItems.isEmpty && !endItems.isEmpty {
                    // Start and end: [start][spacer][end]
                    mainStack.addArrangedSubview(startContainer)
                    mainStack.addArrangedSubview(leftSpacer)
                    mainStack.addArrangedSubview(endContainer)
                } else if !centerItems.isEmpty && !endItems.isEmpty {
                    // Center and end: [center][spacer][end]
                    mainStack.addArrangedSubview(centerContainer)
                    mainStack.addArrangedSubview(leftSpacer)
                    mainStack.addArrangedSubview(endContainer)
                } else if !startItems.isEmpty {
                    // Only start: [start]
                    mainStack.addArrangedSubview(leftPadding)
                    mainStack.addArrangedSubview(startContainer)
                    mainStack.addArrangedSubview(rightPadding)
                } else if !centerItems.isEmpty {
                    // Only center: [spacer][center][spacer]
                    mainStack.addArrangedSubview(leftSpacer)
                    mainStack.addArrangedSubview(centerContainer)
                    mainStack.addArrangedSubview(rightSpacer)
                } else if !endItems.isEmpty {
                    // Only end: [end]
                    mainStack.addArrangedSubview(leftPadding)
                    mainStack.addArrangedSubview(endContainer)
                    mainStack.addArrangedSubview(rightPadding)
                }

                container.addArrangedSubview(mainStack)
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
        spacer.heightAnchor.constraint(equalToConstant: 28).isActive = true // Standard macOS title bar height
        return spacer
    }
}

#endif
