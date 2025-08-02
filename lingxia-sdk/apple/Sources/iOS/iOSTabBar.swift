#if os(iOS)
import UIKit
import Foundation

/// iOS TabBar implementation using shared controller
@MainActor
public class iOSLingXiaTabBar: UIView, EnhancedTabBarProtocol, TabBarUIDelegate {
    public let controller = TabBarController()
    public weak var uiDelegate: TabBarUIDelegate?
    private var tabViews = [UIView]()
    private var itemsContainer: UIStackView?

    public override init(frame: CGRect) {
        super.init(frame: frame)
        setupUI()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupUI()
    }

    private func setupUI() {
        isHidden = true
        backgroundColor = UIColor.white
        uiDelegate = self

        itemsContainer = UIStackView()
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
        backgroundColor = UIColor.clear
        layer.backgroundColor = UIColor.clear.cgColor
        isOpaque = false
        layer.isOpaque = false
    }

    private func updateItemsContainerLayout() {
        guard let itemsContainer = itemsContainer else { return }

        let isVerticalTabBar = controller.isVertical()

        // Basic container setup - detailed layout will be handled in setupGroupedLayout
        itemsContainer.axis = isVerticalTabBar ? .vertical : .horizontal
        itemsContainer.distribution = .fill
        itemsContainer.alignment = .center  // Always center items

        if controller.shouldUseTransparentBackground() {
            itemsContainer.backgroundColor = UIColor.clear
            itemsContainer.layer.backgroundColor = UIColor.clear.cgColor
            itemsContainer.isOpaque = false
            itemsContainer.layer.isOpaque = false
        } else {
            let backgroundColor = controller.getResolvedBackgroundColor()
            itemsContainer.backgroundColor = backgroundColor
            itemsContainer.layer.backgroundColor = backgroundColor.cgColor
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

        // Remove any default margins/insets
        itemsContainer.layoutMargins = UIEdgeInsets.zero
        itemsContainer.isLayoutMarginsRelativeArrangement = false

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

        // Always use grouped layout - items without group go to center
        setupGroupedLayout(items: newItems, container: itemsContainer)

        // Set initial selection
        let selectedPosition = controller.getSelectedPosition()
        if selectedPosition >= 0 && selectedPosition < newItems.count {
            updateTabSelection()
        }
    }

    /// Setup universal grouped layout with separate containers for start/center/end items
    /// Works for all TabBar orientations - group property is independent of position
    private func setupGroupedLayout(items: [TabBarItem], container: UIStackView) {
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
        let hasGroupedItems = !startItems.isEmpty || !endItems.isEmpty

        // Configure container based on orientation
        if isVertical {
            container.axis = .vertical
            container.distribution = .fill
            container.alignment = .center  // Center items horizontally in vertical TabBar
        } else {
            container.axis = .horizontal
            container.distribution = .fill  // This is the problem! Should not use .fill for grouped layout
            container.alignment = .center
        }

        if isVertical && hasGroupedItems {
            // Grouped layout for vertical TabBar

            // Add small top spacing to avoid being too close to status bar/clock (only for vertical)
            let topSpacer = UIView()
            topSpacer.heightAnchor.constraint(equalToConstant: 4).isActive = true
            container.addArrangedSubview(topSpacer)

            // Create start container - should be at top
            if !startItems.isEmpty {
                let startContainer = createGroupContainer(items: startItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)
                container.addArrangedSubview(startContainer)
            }

            // Create flexible spacer to push end items to bottom
            let flexibleSpacer = UIView()
            flexibleSpacer.setContentHuggingPriority(.defaultLow, for: .vertical)
            flexibleSpacer.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
            container.addArrangedSubview(flexibleSpacer)

            // Create center container (middle) - only if we have center items
            if !centerItems.isEmpty {
                let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                container.addArrangedSubview(centerContainer)

                // Add another spacer after center items
                let spacer2 = UIView()
                spacer2.setContentHuggingPriority(.defaultLow, for: .vertical)
                spacer2.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
                container.addArrangedSubview(spacer2)
            }

            // Create end container - should be at bottom
            if !endItems.isEmpty {
                let endContainer = createGroupContainer(items: endItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)
                container.addArrangedSubview(endContainer)
            }

            // Add bottom spacing for safe area (iPhone rounded corners and home indicator)
            let bottomSpacer = UIView()
            bottomSpacer.heightAnchor.constraint(equalToConstant: 20).isActive = true
            container.addArrangedSubview(bottomSpacer)

        } else {
            // Simple centered layout for vertical without grouping
            if isVertical {
                // Add flexible spacer before center items to push them to center
                let spacer = UIView()
                spacer.setContentHuggingPriority(.defaultLow, for: .vertical)
                spacer.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
                container.addArrangedSubview(spacer)

                // Create center container with all items
                if !centerItems.isEmpty {
                    let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                    container.addArrangedSubview(centerContainer)
                }

                // Add flexible spacer after center items
                let spacer2 = UIView()
                spacer2.setContentHuggingPriority(.defaultLow, for: .vertical)
                spacer2.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
                container.addArrangedSubview(spacer2)

                // Add bottom spacing for safe area (iPhone rounded corners and home indicator)
                let bottomSpacer = UIView()
                bottomSpacer.heightAnchor.constraint(equalToConstant: 20).isActive = true
                container.addArrangedSubview(bottomSpacer)

            } else if hasGroupedItems {
                // Create start container - should be at left
                if !startItems.isEmpty {
                    let startContainer = createGroupContainer(items: startItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)

                    container.addSubview(startContainer)

                    // Pin to left
                    startContainer.translatesAutoresizingMaskIntoConstraints = false
                    NSLayoutConstraint.activate([
                        startContainer.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                        startContainer.centerYAnchor.constraint(equalTo: container.centerYAnchor)
                    ])
                }

                // Create end container - should be at right
                if !endItems.isEmpty {
                    // Use smaller spacing for end items to reduce gap
                    let endContainer = createGroupContainer(items: endItems, spacing: 8, isVertical: isVertical)

                    container.addSubview(endContainer)

                    // Pin to right
                    endContainer.translatesAutoresizingMaskIntoConstraints = false
                    NSLayoutConstraint.activate([
                        endContainer.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                        endContainer.centerYAnchor.constraint(equalTo: container.centerYAnchor)
                    ])
                }

                // Create center container (middle) - only if we have center items
                if !centerItems.isEmpty {
                    let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                    container.addSubview(centerContainer)

                    // Pin to center
                    centerContainer.translatesAutoresizingMaskIntoConstraints = false
                    NSLayoutConstraint.activate([
                        centerContainer.centerXAnchor.constraint(equalTo: container.centerXAnchor),
                        centerContainer.centerYAnchor.constraint(equalTo: container.centerYAnchor)
                    ])
                }

            } else {
                // Simple centered layout for horizontal without grouping
                // Create center container with all items (should be centerItems only when no grouping)
                if !centerItems.isEmpty {
                    let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
                    container.addArrangedSubview(centerContainer)
                }
            }
        }
    }

    /// Create a container for a group of tab items
    private func createGroupContainer(items: [TabBarItem], spacing: CGFloat, isVertical: Bool) -> UIStackView {
        let groupContainer = UIStackView()
        groupContainer.axis = isVertical ? .vertical : .horizontal
        // Use different distribution for horizontal vs vertical
        groupContainer.distribution = isVertical ? .fill : .fillProportionally
        groupContainer.spacing = spacing
        groupContainer.translatesAutoresizingMaskIntoConstraints = false

        // Remove any default margins/insets
        groupContainer.layoutMargins = UIEdgeInsets.zero
        groupContainer.isLayoutMarginsRelativeArrangement = false

        // For vertical TabBar, allow container to expand to give items proper space
        if isVertical {
            groupContainer.setContentHuggingPriority(.defaultLow, for: .vertical)
            // Set minimum height for the container to ensure proper spacing
            let minHeight = CGFloat(items.count) * 60.0 + CGFloat(items.count - 1) * spacing
            groupContainer.heightAnchor.constraint(greaterThanOrEqualToConstant: minHeight).isActive = true
        } else {
            // For horizontal TabBar, container should hug its content tightly
            groupContainer.setContentHuggingPriority(.required, for: .horizontal)
            groupContainer.setContentCompressionResistancePriority(.required, for: .horizontal)
            // For horizontal, ensure minimum height
            groupContainer.heightAnchor.constraint(greaterThanOrEqualToConstant: 60).isActive = true
        }

        for (index, item) in items.enumerated() {
            // Find the global index of this item
            let allItems = controller.getItems()
            let globalIndex = allItems.firstIndex { $0.page_path.toString() == item.page_path.toString() } ?? index
            let tabView = createTabView(for: item, at: globalIndex)
            tabViews.append(tabView)
            groupContainer.addArrangedSubview(tabView)
        }

        return groupContainer
    }

    private func createTabView(for item: TabBarItem, at index: Int) -> UIView {
        let tabView = UIView()
        // Use semi-transparent background for debugging click area
        tabView.backgroundColor = UIColor.clear
        tabView.isUserInteractionEnabled = true  // Ensure tap gestures work

        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.alignment = .center
        stackView.spacing = 4
        stackView.translatesAutoresizingMaskIntoConstraints = false

        // Create icon
        let iconView = UIImageView()
        iconView.contentMode = .scaleAspectFit
        iconView.translatesAutoresizingMaskIntoConstraints = false

        // Create label
        let label = UILabel()
        label.text = item.text.toString()
        label.font = UIFont.systemFont(ofSize: TabBarConstants.ITEM_FONT_SIZE)
        label.textAlignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false

        stackView.addArrangedSubview(iconView)
        if !item.text.toString().isEmpty {
            stackView.addArrangedSubview(label)
        }

        tabView.addSubview(stackView)

        // Set fixed dimensions for TabBar items
        let isVertical = controller.isVertical()
        if isVertical {
            tabView.heightAnchor.constraint(equalToConstant: 60).isActive = true
            // Ensure TabView fills the width of the TabBar for proper click area
            tabView.widthAnchor.constraint(greaterThanOrEqualToConstant: 40).isActive = true
        } else {
            // For horizontal TabBar, set smaller minimum width for tighter spacing
            tabView.widthAnchor.constraint(greaterThanOrEqualToConstant: 60).isActive = true
            tabView.heightAnchor.constraint(equalToConstant: 60).isActive = true
        }

        NSLayoutConstraint.activate([
            stackView.centerXAnchor.constraint(equalTo: tabView.centerXAnchor),
            stackView.centerYAnchor.constraint(equalTo: tabView.centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: TabBarConstants.ICON_SIZE),
            iconView.heightAnchor.constraint(equalToConstant: TabBarConstants.ICON_SIZE)
        ])

        // Add tap gesture
        let tapGesture = UITapGestureRecognizer(target: self, action: #selector(tabTapped(_:)))
        tabView.addGestureRecognizer(tapGesture)
        tabView.tag = index

        return tabView
    }

    @objc private func tabTapped(_ gesture: UITapGestureRecognizer) {
        guard let tabView = gesture.view else { return }
        let index = tabView.tag

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

    private func updateTabAppearance(tabView: UIView, isSelected: Bool, item: TabBarItem) {
        guard let stackView = tabView.subviews.first as? UIStackView,
              let iconView = stackView.arrangedSubviews.first as? UIImageView else { return }

        // Update colors - config should always have values due to Rust defaults
        let selectedColor = TabBarHelper.parseColor(config?.selected_color.toString() ?? "") ?? UIColor.systemBlue
        let normalColor = TabBarHelper.parseColor(config?.color.toString() ?? "") ?? UIColor.gray

        let color = isSelected ? selectedColor : normalColor
        iconView.tintColor = color

        // Update label color if exists
        if stackView.arrangedSubviews.count > 1,
           let label = stackView.arrangedSubviews[1] as? UILabel {
            label.textColor = color
        }

        // Load appropriate icon
        let iconPath = isSelected ? item.selected_icon_path.toString() : item.icon_path.toString()
        loadIcon(for: iconView, iconPath: iconPath)
    }

    private func loadIcon(for imageView: UIImageView, iconPath: String) {
        // Simple icon loading - you can enhance this with proper image loading
        if iconPath.hasPrefix("SF:") {
            let symbolName = String(iconPath.dropFirst(3))
            if #available(iOS 13.0, *) {
                imageView.image = UIImage(systemName: symbolName)
            }
        } else {
            imageView.image = UIImage(named: iconPath)
        }
    }
}

/// iOS-specific TabBar support utilities
@MainActor
public class iOSTabBarSupport {

    /// Creates a TabBar for iOS
    public static func createTabBar(frame: CGRect) -> iOSLingXiaTabBar {
        return iOSLingXiaTabBar(frame: frame)
    }

    /// Configures tab bar transparency mode
    public static func configureTabBarTransparencyMode(_ tabBar: iOSLingXiaTabBar, isTransparent: Bool) {
        if isTransparent {
            tabBar.backgroundColor = UIColor.clear
            tabBar.layer.backgroundColor = UIColor.clear.cgColor
        } else {
            // Use the configured background color or default
            let config = tabBar.config
            if let bgColor = TabBarConfig.parseColor(config?.background_color.toString() ?? "") {
                tabBar.backgroundColor = bgColor
                tabBar.layer.backgroundColor = bgColor.cgColor
            } else {
                tabBar.backgroundColor = UIColor.systemBackground
                tabBar.layer.backgroundColor = UIColor.systemBackground.cgColor
            }
        }
    }

    /// Applies tab bar layout parameters
    public static func applyTabBarLayoutParams(tabBar: iOSLingXiaTabBar, config: TabBarConfig) {
        let position = config.position
        let isVertical = position == 2 || position == 3 // 2=left, 3=right

        // No transform needed - we handle orientation internally with UIStackView
        tabBar.transform = CGAffineTransform.identity

        // Apply dimension (height/width)
        if isVertical {
            tabBar.frame.size.width = CGFloat(config.dimension)
        } else {
            tabBar.frame.size.height = CGFloat(config.dimension)
        }

        // Configure background - CRITICAL: Don't override transparent backgrounds!
        if TabBarConfig.isTransparent(config.background_color.toString()) {
            // For transparent backgrounds, force transparency mode instead of using resolved color
            tabBar.forceTransparencyMode()
        } else {
            // For non-transparent backgrounds, use resolved color
            let resolvedColor = config.resolvedBackgroundColor(isVertical: isVertical)
            tabBar.backgroundColor = resolvedColor
            tabBar.layer.backgroundColor = resolvedColor.cgColor
        }
    }

    /// Gets the appropriate content area frame considering tab bar position
    public static func getContentAreaFrame(containerFrame: CGRect, tabBarPosition: Int32, tabBarHeight: CGFloat, hasTabBar: Bool) -> CGRect {
        guard hasTabBar else { return containerFrame }

        switch tabBarPosition {
        case 0: // bottom
            return CGRect(x: 0, y: 0, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case 1: // top
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case 2: // left
            return CGRect(x: tabBarHeight, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        case 3: // right
            return CGRect(x: 0, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        default:
            return containerFrame
        }
    }

    /// Calculates the appropriate anchor points for tab bar positioning
    public static func calculateTabBarAnchors(for position: Int32, in containerView: UIView, safeArea: UILayoutGuide) -> (top: NSLayoutYAxisAnchor, bottom: NSLayoutYAxisAnchor, leading: NSLayoutXAxisAnchor, trailing: NSLayoutXAxisAnchor) {
        switch position {
        case 0: // bottom
            return (
                top: containerView.bottomAnchor,
                bottom: safeArea.bottomAnchor,
                leading: safeArea.leadingAnchor,
                trailing: safeArea.trailingAnchor
            )
        case 1: // top
            return (
                top: safeArea.topAnchor,
                bottom: containerView.topAnchor,
                leading: safeArea.leadingAnchor,
                trailing: safeArea.trailingAnchor
            )
        case 2: // left
            return (
                top: safeArea.topAnchor,
                bottom: safeArea.bottomAnchor,
                leading: safeArea.leadingAnchor,
                trailing: containerView.leadingAnchor
            )
        case 3: // right
            return (
                top: safeArea.topAnchor,
                bottom: safeArea.bottomAnchor,
                leading: containerView.trailingAnchor,
                trailing: safeArea.trailingAnchor
            )
        default:
            return (
                top: containerView.bottomAnchor,
                bottom: safeArea.bottomAnchor,
                leading: safeArea.leadingAnchor,
                trailing: safeArea.trailingAnchor
            )
        }
    }
}

#endif
