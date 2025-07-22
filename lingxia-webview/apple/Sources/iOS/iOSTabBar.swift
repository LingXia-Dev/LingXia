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
        backgroundColor = UIColor(hexString: TabBarConfig.DEFAULT_BACKGROUND_COLOR) ?? UIColor.white
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

        itemsContainer.axis = isVerticalTabBar ? .vertical : .horizontal
        itemsContainer.distribution = .fillEqually
        itemsContainer.alignment = .fill

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

        // Create new tab views
        for (index, item) in newItems.enumerated() {
            let tabView = createTabView(for: item, at: index)
            tabViews.append(tabView)
            itemsContainer.addArrangedSubview(tabView)
        }

        // Set initial selection
        let selectedPosition = controller.getSelectedPosition()
        if selectedPosition >= 0 && selectedPosition < newItems.count {
            updateTabSelection()
        }
    }

    private func createTabView(for item: TabBarItem, at index: Int) -> UIView {
        let tabView = UIView()
        tabView.backgroundColor = UIColor.clear

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
        label.text = item.text
        label.font = UIFont.systemFont(ofSize: TabBarConstants.ITEM_FONT_SIZE)
        label.textAlignment = .center
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

        // Update colors
        let selectedColor = config.parseColor(config.selectedColor) ?? UIColor(hexString: TabBarConfig.DEFAULT_SELECTED_COLOR) ?? UIColor.systemBlue
        let normalColor = config.parseColor(config.color) ?? UIColor(hexString: TabBarConfig.DEFAULT_UNSELECTED_COLOR) ?? UIColor.gray

        let color = isSelected ? selectedColor : normalColor
        iconView.tintColor = color

        // Update label color if exists
        if stackView.arrangedSubviews.count > 1,
           let label = stackView.arrangedSubviews[1] as? UILabel {
            label.textColor = color
        }

        // Load appropriate icon
        let iconPath = isSelected ? item.selectedIconPath : item.iconPath
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

    /// Gets the tab bar height for iOS
    public static func getTabBarHeight() -> CGFloat {
        return TabBarConstants.TAB_HEIGHT
    }

    /// Configures tab bar transparency mode
    public static func configureTabBarTransparencyMode(_ tabBar: iOSLingXiaTabBar, isTransparent: Bool) {
        if isTransparent {
            tabBar.backgroundColor = UIColor.clear
            tabBar.layer.backgroundColor = UIColor.clear.cgColor
        } else {
            // Use the configured background color or default
            let config = tabBar.config
            if let bgColor = config.parseColor(config.backgroundColor) {
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
        let isVertical = position == .left || position == .right

        // Configure orientation
        if isVertical {
            tabBar.transform = CGAffineTransform(rotationAngle: .pi / 2)
        } else {
            tabBar.transform = CGAffineTransform.identity
        }

        // Apply height if specified
        if let height = config.height {
            tabBar.frame.size.height = height
        }

        // Configure background - CRITICAL: Don't override transparent backgrounds!
        if TabBarConfig.isTransparent(config.backgroundColor) {
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
    public static func getContentAreaFrame(containerFrame: CGRect, tabBarPosition: TabBarConfig.Position, tabBarHeight: CGFloat, hasTabBar: Bool) -> CGRect {
        guard hasTabBar else { return containerFrame }

        switch tabBarPosition {
        case .bottom:
            return CGRect(x: 0, y: 0, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case .top:
            return CGRect(x: 0, y: tabBarHeight, width: containerFrame.width, height: containerFrame.height - tabBarHeight)
        case .left:
            return CGRect(x: tabBarHeight, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        case .right:
            return CGRect(x: 0, y: 0, width: containerFrame.width - tabBarHeight, height: containerFrame.height)
        }
    }

    /// Calculates the appropriate anchor points for tab bar positioning
    public static func calculateTabBarAnchors(for position: TabBarConfig.Position, in containerView: UIView, safeArea: UILayoutGuide) -> (top: NSLayoutYAxisAnchor, bottom: NSLayoutYAxisAnchor, leading: NSLayoutXAxisAnchor, trailing: NSLayoutXAxisAnchor) {
        switch position {
        case .bottom:
            return (
                top: containerView.bottomAnchor,
                bottom: safeArea.bottomAnchor,
                leading: safeArea.leadingAnchor,
                trailing: safeArea.trailingAnchor
            )
        case .top:
            return (
                top: safeArea.topAnchor,
                bottom: containerView.topAnchor,
                leading: safeArea.leadingAnchor,
                trailing: safeArea.trailingAnchor
            )
        case .left:
            return (
                top: safeArea.topAnchor,
                bottom: safeArea.bottomAnchor,
                leading: safeArea.leadingAnchor,
                trailing: containerView.leadingAnchor
            )
        case .right:
            return (
                top: safeArea.topAnchor,
                bottom: safeArea.bottomAnchor,
                leading: containerView.trailingAnchor,
                trailing: safeArea.trailingAnchor
            )
        }
    }
}

#endif
