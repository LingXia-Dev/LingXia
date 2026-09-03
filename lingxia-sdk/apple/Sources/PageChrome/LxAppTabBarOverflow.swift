#if os(iOS)
import UIKit

/// The tab items a compact strip could not fit, shown as a panel directly above
/// the bar so the "more" slot reads as an extension of it rather than a modal.
@MainActor
final class LxAppTabBarOverflowPanel: UIView {
    private enum Metrics {
        static let columns = 5
        static let cornerRadius: CGFloat = 16
        static let panelPadding: CGFloat = 8
        static let horizontalInset: CGFloat = 12
        static let bottomGap: CGFloat = 8
        static let cellHeight: CGFloat = 64
        static let iconSize: CGFloat = 24
        static let enterDuration: TimeInterval = 0.16
    }

    private let panel = UIView()
    private let scrim = UIView()
    private let indices: [Int]
    private let onPick: (Int) -> Void
    private let onDismiss: () -> Void
    private var didDismiss = false

    /// - Parameters:
    ///   - indices: positions in `items` to offer, in declaration order.
    ///   - onPick: receives the picked item's declaration index.
    init(
        items: [TabBarItem],
        indices: [Int],
        config: TabBar,
        selectedIndex: Int,
        appId: String,
        onPick: @escaping (Int) -> Void,
        onDismiss: @escaping () -> Void
    ) {
        self.indices = indices
        self.onPick = onPick
        self.onDismiss = onDismiss
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        buildScrim()
        buildPanel(items: items, config: config, selectedIndex: selectedIndex, appId: appId)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        panel.frame.contains(point) || scrim.frame.contains(point)
    }

    /// Installs the panel over `host`, resting on top of `anchor`.
    func present(in host: UIView, above anchor: UIView) {
        host.addSubview(self)
        NSLayoutConstraint.activate([
            topAnchor.constraint(equalTo: host.topAnchor),
            leadingAnchor.constraint(equalTo: host.leadingAnchor),
            trailingAnchor.constraint(equalTo: host.trailingAnchor),
            bottomAnchor.constraint(equalTo: host.bottomAnchor),
            scrim.bottomAnchor.constraint(equalTo: anchor.topAnchor),
            panel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Metrics.horizontalInset),
            panel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Metrics.horizontalInset),
            panel.bottomAnchor.constraint(equalTo: anchor.topAnchor, constant: -Metrics.bottomGap)
        ])

        scrim.alpha = 0
        layoutIfNeeded()
        panel.transform = CGAffineTransform(translationX: 0, y: panel.bounds.height)
        UIView.animate(withDuration: Metrics.enterDuration) {
            self.scrim.alpha = 1
            self.panel.transform = .identity
        }
    }

    @objc private func dismiss() {
        finishDismiss()
    }

    func dismissPanel() {
        finishDismiss()
    }

    private func finishDismiss() {
        guard !didDismiss else { return }
        didDismiss = true
        removeFromSuperview()
        onDismiss()
    }

    private func buildScrim() {
        scrim.translatesAutoresizingMaskIntoConstraints = false
        scrim.backgroundColor = UIColor.clear
        scrim.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(dismiss)))
        addSubview(scrim)
        NSLayoutConstraint.activate([
            scrim.topAnchor.constraint(equalTo: topAnchor),
            scrim.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrim.trailingAnchor.constraint(equalTo: trailingAnchor)
        ])
    }

    private func buildPanel(items: [TabBarItem], config: TabBar, selectedIndex: Int, appId: String) {
        panel.translatesAutoresizingMaskIntoConstraints = false
        // The bar itself is drawn with the lxapp's declared colour; the panel
        // was asking the system instead, so the two disagreed whenever the app
        // and the system disagreed. Follow the bar, and fall back to the page
        // only where the bar is transparent and there is nothing to follow.
        panel.backgroundColor = TabBarHelper.isTransparent(config.background_color)
            ? WebViewManager.overflowPanelColor(appId: appId)
            : PlatformColor(argb: config.background_color)
        panel.layer.cornerRadius = Metrics.cornerRadius
        panel.layer.shadowColor = UIColor.black.cgColor
        panel.layer.shadowOpacity = 0.16
        panel.layer.shadowRadius = 12
        panel.layer.shadowOffset = CGSize(width: 0, height: 4)
        // The panel is the modal surface; taps must not fall through to the scrim.
        panel.isUserInteractionEnabled = true
        addSubview(panel)

        let rows = UIStackView()
        rows.axis = .vertical
        rows.spacing = 0
        rows.translatesAutoresizingMaskIntoConstraints = false
        panel.addSubview(rows)

        for chunk in indices.chunked(into: Metrics.columns) {
            rows.addArrangedSubview(
                buildRow(
                    chunk,
                    items: items,
                    config: config,
                    selectedIndex: selectedIndex,
                    appId: appId
                )
            )
        }

        NSLayoutConstraint.activate([
            rows.topAnchor.constraint(equalTo: panel.topAnchor, constant: Metrics.panelPadding),
            rows.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: Metrics.panelPadding),
            rows.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -Metrics.panelPadding),
            rows.bottomAnchor.constraint(equalTo: panel.bottomAnchor, constant: -Metrics.panelPadding)
        ])
    }

    /// A short final row keeps the column count of a full one, so cells stay
    /// aligned in a grid instead of spreading across the panel.
    private func buildRow(
        _ rowIndices: [Int],
        items: [TabBarItem],
        config: TabBar,
        selectedIndex: Int,
        appId: String
    ) -> UIStackView {
        let row = UIStackView()
        row.axis = .horizontal
        row.distribution = .fillEqually
        row.alignment = .fill

        for index in rowIndices {
            guard index < items.count else { continue }
            row.addArrangedSubview(
                buildCell(
                    index: index,
                    item: items[index],
                    config: config,
                    selected: items[index].cachedIndex == selectedIndex,
                    appId: appId
                )
            )
        }
        for _ in rowIndices.count..<Metrics.columns {
            row.addArrangedSubview(UIView())
        }
        return row
    }

    private func buildCell(
        index: Int,
        item: TabBarItem,
        config: TabBar,
        selected: Bool,
        appId: String
    ) -> UIView {
        let button = UIButton(type: .custom)
        button.tag = item.cachedIndex
        button.addTarget(self, action: #selector(cellTapped(_:)), for: .touchUpInside)
        button.heightAnchor.constraint(equalToConstant: Metrics.cellHeight).isActive = true

        let stack = UIStackView()
        stack.axis = .vertical
        stack.alignment = .center
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.isUserInteractionEnabled = false
        button.addSubview(stack)

        let iconContainer = UIView()
        iconContainer.translatesAutoresizingMaskIntoConstraints = false
        let icon = UIImageView()
        icon.contentMode = .scaleAspectFit
        icon.translatesAutoresizingMaskIntoConstraints = false
        // Mirrors the strip: the icon is a template the panel tints, and the
        // indicator marks whatever is selected.
        let image = LxAppTabBarOverflowPanel.icon(for: item)
        icon.image = image?.withRenderingMode(.alwaysTemplate)
        icon.tintColor = selected
            ? PlatformColor(argb: config.selected_color)
            : PlatformColor(argb: config.color)
        if selected {
            let indicator = UIView()
            indicator.backgroundColor = PlatformColor(argb: config.selected_color)
                .withAlphaComponent(TabBarMetrics.activeIndicatorOpacity)
            indicator.layer.cornerRadius = TabBarMetrics.activeIndicatorSize / 2
            indicator.translatesAutoresizingMaskIntoConstraints = false
            iconContainer.addSubview(indicator)
            NSLayoutConstraint.activate([
                indicator.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
                indicator.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
                indicator.widthAnchor.constraint(equalToConstant: TabBarMetrics.activeIndicatorSize),
                indicator.heightAnchor.constraint(equalToConstant: TabBarMetrics.activeIndicatorSize)
            ])
        }

        iconContainer.addSubview(icon)
        stack.addArrangedSubview(iconContainer)

        if let rustItem = getTabBarItem(appId, Int32(index)) {
            let badge = rustItem.badge.toString()
            if !badge.isEmpty {
                addMarker(badgeLabel(badge), to: iconContainer, inset: -6)
            } else if rustItem.has_red_dot {
                addMarker(redDot(), to: iconContainer, inset: -4)
            }
        }

        let label = UILabel()
        label.text = item.text.toString()
        label.font = UIFont.systemFont(ofSize: 10, weight: .medium)
        label.textColor = selected
            ? PlatformColor(argb: config.selected_color)
            : PlatformColor(argb: config.color)
        label.textAlignment = .center
        label.lineBreakMode = .byTruncatingTail
        stack.addArrangedSubview(label)

        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: button.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: button.centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: button.leadingAnchor, constant: 4),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: button.trailingAnchor, constant: -4),
            iconContainer.widthAnchor.constraint(equalToConstant: 32),
            iconContainer.heightAnchor.constraint(equalToConstant: 32),
            icon.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
            icon.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
            icon.widthAnchor.constraint(equalToConstant: Metrics.iconSize),
            icon.heightAnchor.constraint(equalToConstant: Metrics.iconSize)
        ])
        return button
    }

    @objc private func cellTapped(_ sender: UIButton) {
        let index = sender.tag
        onPick(index)
        finishDismiss()
    }

    private static func icon(for item: TabBarItem) -> UIImage? {
        let path = item.icon_path.toString()
        if path.hasPrefix("SF:") {
            return UIImage(systemName: String(path.dropFirst(3)))
        }
        return UIImage(named: path) ?? UIImage(systemName: "circle.fill")
    }

    private func addMarker(_ marker: UIView, to container: UIView, inset: CGFloat) {
        container.addSubview(marker)
        NSLayoutConstraint.activate([
            marker.topAnchor.constraint(equalTo: container.topAnchor, constant: inset),
            marker.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: 4)
        ])
    }

    private func badgeLabel(_ text: String) -> UIView {
        let container = UIView()
        container.backgroundColor = UIColor(red: 0xFA / 255.0, green: 0x51 / 255.0, blue: 0x51 / 255.0, alpha: 1.0)
        container.layer.cornerRadius = 8
        container.translatesAutoresizingMaskIntoConstraints = false

        let label = UILabel()
        label.text = text
        label.font = UIFont.systemFont(ofSize: 10, weight: .medium)
        label.textColor = .white
        label.textAlignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(label)

        NSLayoutConstraint.activate([
            label.topAnchor.constraint(equalTo: container.topAnchor, constant: 1),
            label.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -1),
            label.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 5),
            label.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -5),
            container.heightAnchor.constraint(equalToConstant: 16)
        ])
        return container
    }

    private func redDot() -> UIView {
        let dot = UIView()
        dot.backgroundColor = UIColor(red: 0xFA / 255.0, green: 0x51 / 255.0, blue: 0x51 / 255.0, alpha: 1.0)
        dot.layer.cornerRadius = 4
        dot.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            dot.widthAnchor.constraint(equalToConstant: 8),
            dot.heightAnchor.constraint(equalToConstant: 8)
        ])
        return dot
    }
}

private extension Array {
    func chunked(into size: Int) -> [[Element]] {
        guard size > 0 else { return [self] }
        return stride(from: 0, to: count, by: size).map { start in
            Array(self[start..<Swift.min(start + size, count)])
        }
    }
}
#endif
