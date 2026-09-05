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
        let path = item.icon_path.toString()
        let template = TabBarHelper.isTemplateIcon(path)
        icon.contentMode = template ? .scaleAspectFit : .scaleAspectFill
        icon.clipsToBounds = !template
        icon.layer.cornerRadius = template ? 0 : 6
        applyUIKitTabIcon(
            to: icon,
            path: path,
            tint: selected
                ? PlatformColor(argb: config.selected_color)
                : PlatformColor(argb: config.color)
        )
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

        if selected {
            let indicator = UIView()
            indicator.backgroundColor = PlatformColor(argb: config.selected_color)
                .withAlphaComponent(TabBarMetrics.activeIndicatorOpacity)
            indicator.layer.cornerRadius = 16
            indicator.translatesAutoresizingMaskIntoConstraints = false
            button.insertSubview(indicator, at: 0)
            NSLayoutConstraint.activate([
                indicator.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
                indicator.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
                indicator.widthAnchor.constraint(equalToConstant: 32),
                indicator.heightAnchor.constraint(equalToConstant: 32)
            ])
        }

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

#elseif os(macOS)
import AppKit

/// AppKit counterpart of the iOS overflow panel. The runner's WKWebView is
/// layer-backed and paints over any sibling of the strip, so this overlay is
/// installed on the window and then pinned back to the simulated screen.
@MainActor
final class LxAppTabBarOverflowPanel: NSView {
    private enum Metrics {
        static let columns = 5
        static let cornerRadius: CGFloat = 16
        static let panelPadding: CGFloat = 8
        static let horizontalInset: CGFloat = 12
        static let bottomGap: CGFloat = 8
        static let cellHeight: CGFloat = 64
        static let iconSize: CGFloat = 24
        static let iconContainer: CGFloat = 32
        static let enterDuration: TimeInterval = 0.16
        static let labelSize: CGFloat = 10
    }

    private let plate = OverflowHitView()
    private let scrim = OverflowScrimView()
    private let indices: [Int]
    private let config: TabBar
    private let appId: String
    private let displayScale: CGFloat
    private let onPick: (Int) -> Void
    private let onDismiss: () -> Void
    private var didDismiss = false

    init(
        items: [TabBarItem],
        indices: [Int],
        config: TabBar,
        selectedIndex: Int,
        appId: String,
        displayScale: CGFloat = 1,
        onPick: @escaping (Int) -> Void,
        onDismiss: @escaping () -> Void
    ) {
        self.indices = indices
        self.config = config
        self.appId = appId
        self.displayScale = min(1, max(0.4, displayScale))
        self.onPick = onPick
        self.onDismiss = onDismiss
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        buildScrim()
        buildPlate(items: items, selectedIndex: selectedIndex)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    private func scaled(_ value: CGFloat) -> CGFloat {
        value * displayScale
    }

    /// Empty regions of the window overlay must not swallow runner chrome.
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        paintPlate()
    }

    /// - Parameters:
    ///   - host: the window content view, so the overlay paints above WKWebView.
    ///   - anchor: the tab strip; the card sits just above it.
    ///   - screen: the simulated phone, which the scrim and card must stay inside.
    func present(in host: NSView, above anchor: NSView, clippedTo screen: NSView) {
        host.addSubview(self, positioned: .above, relativeTo: nil)
        paintPlate()
        let bottom = plate.bottomAnchor.constraint(
            equalTo: anchor.topAnchor,
            constant: -scaled(Metrics.bottomGap)
        )
        NSLayoutConstraint.activate([
            topAnchor.constraint(equalTo: host.topAnchor),
            leadingAnchor.constraint(equalTo: host.leadingAnchor),
            trailingAnchor.constraint(equalTo: host.trailingAnchor),
            bottomAnchor.constraint(equalTo: host.bottomAnchor),
            scrim.topAnchor.constraint(equalTo: screen.topAnchor),
            scrim.leadingAnchor.constraint(equalTo: screen.leadingAnchor),
            scrim.trailingAnchor.constraint(equalTo: screen.trailingAnchor),
            scrim.bottomAnchor.constraint(equalTo: anchor.topAnchor),
            plate.leadingAnchor.constraint(
                equalTo: anchor.leadingAnchor,
                constant: scaled(Metrics.horizontalInset)
            ),
            plate.trailingAnchor.constraint(
                equalTo: anchor.trailingAnchor,
                constant: -scaled(Metrics.horizontalInset)
            ),
            bottom
        ])

        layoutSubtreeIfNeeded()
        bottom.constant = plate.frame.height
        layoutSubtreeIfNeeded()
        NSAnimationContext.runAnimationGroup { context in
            context.duration = Metrics.enterDuration
            context.allowsImplicitAnimation = true
            bottom.animator().constant = -scaled(Metrics.bottomGap)
            layoutSubtreeIfNeeded()
        }
    }

    func dismissPanel() {
        finishDismiss()
    }

    func paintPlate() {
        let color: NSColor = TabBarHelper.isTransparent(config.background_color)
            ? WebViewManager.overflowPanelColor(appId: appId)
            : PlatformColor(argb: config.background_color)
        effectiveAppearance.performAsCurrentDrawingAppearance {
            plate.layer?.backgroundColor = color.cgColor
        }
    }

    private func finishDismiss() {
        guard !didDismiss else { return }
        didDismiss = true
        removeFromSuperview()
        onDismiss()
    }

    private func buildScrim() {
        scrim.translatesAutoresizingMaskIntoConstraints = false
        scrim.wantsLayer = true
        scrim.layer?.backgroundColor = NSColor.clear.cgColor
        scrim.onClick = { [weak self] in self?.finishDismiss() }
        addSubview(scrim)
    }

    private func buildPlate(items: [TabBarItem], selectedIndex: Int) {
        plate.translatesAutoresizingMaskIntoConstraints = false
        plate.wantsLayer = true
        plate.layer?.cornerRadius = scaled(Metrics.cornerRadius)
        plate.layer?.masksToBounds = false
        plate.layer?.shadowColor = NSColor.black.cgColor
        plate.layer?.shadowOpacity = 0.16
        plate.layer?.shadowRadius = scaled(12)
        plate.layer?.shadowOffset = CGSize(width: 0, height: scaled(-4))
        paintPlate()
        addSubview(plate)

        let rows = NSStackView()
        rows.orientation = .vertical
        rows.spacing = 0
        rows.alignment = .width
        rows.setHuggingPriority(.required, for: .vertical)
        rows.translatesAutoresizingMaskIntoConstraints = false
        plate.addSubview(rows)

        for chunk in indices.chunked(into: Metrics.columns) {
            rows.addArrangedSubview(buildRow(chunk, items: items, selectedIndex: selectedIndex))
        }

        NSLayoutConstraint.activate([
            rows.topAnchor.constraint(equalTo: plate.topAnchor, constant: scaled(Metrics.panelPadding)),
            rows.leadingAnchor.constraint(
                equalTo: plate.leadingAnchor,
                constant: scaled(Metrics.panelPadding)
            ),
            rows.trailingAnchor.constraint(
                equalTo: plate.trailingAnchor,
                constant: -scaled(Metrics.panelPadding)
            ),
            rows.bottomAnchor.constraint(
                equalTo: plate.bottomAnchor,
                constant: -scaled(Metrics.panelPadding)
            )
        ])
    }

    private func buildRow(
        _ rowIndices: [Int],
        items: [TabBarItem],
        selectedIndex: Int
    ) -> NSView {
        let row = NSView()
        row.translatesAutoresizingMaskIntoConstraints = false
        row.heightAnchor.constraint(equalToConstant: scaled(Metrics.cellHeight)).isActive = true

        var slots: [NSView] = []
        for index in rowIndices {
            guard index < items.count else { continue }
            slots.append(
                buildCell(
                    index: index,
                    item: items[index],
                    selected: items[index].cachedIndex == selectedIndex
                )
            )
        }
        for _ in slots.count..<Metrics.columns {
            let spacer = NSView()
            spacer.translatesAutoresizingMaskIntoConstraints = false
            slots.append(spacer)
        }

        var previous: NSView?
        for slot in slots {
            row.addSubview(slot)
            NSLayoutConstraint.activate([
                slot.topAnchor.constraint(equalTo: row.topAnchor),
                slot.bottomAnchor.constraint(equalTo: row.bottomAnchor),
                slot.widthAnchor.constraint(
                    equalTo: row.widthAnchor,
                    multiplier: 1 / CGFloat(Metrics.columns)
                ),
                slot.leadingAnchor.constraint(equalTo: previous?.trailingAnchor ?? row.leadingAnchor)
            ])
            previous = slot
        }
        previous?.trailingAnchor.constraint(equalTo: row.trailingAnchor).isActive = true
        return row
    }

    private func buildCell(index: Int, item: TabBarItem, selected: Bool) -> NSView {
        let cell = OverflowCellView()
        cell.translatesAutoresizingMaskIntoConstraints = false
        cell.heightAnchor.constraint(equalToConstant: scaled(Metrics.cellHeight)).isActive = true
        cell.onPick = { [weak self] in
            guard let self else { return }
            self.onPick(item.cachedIndex)
            self.finishDismiss()
        }

        let tint = selected
            ? PlatformColor(argb: config.selected_color)
            : PlatformColor(argb: config.color)

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .centerX
        stack.spacing = scaled(4)
        stack.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(stack)

        let iconContainer = NSView()
        iconContainer.translatesAutoresizingMaskIntoConstraints = false
        iconContainer.wantsLayer = true

        if selected {
            let indicator = NSView()
            indicator.translatesAutoresizingMaskIntoConstraints = false
            indicator.wantsLayer = true
            indicator.layer?.backgroundColor = tint
                .withAlphaComponent(TabBarMetrics.activeIndicatorOpacity).cgColor
            indicator.layer?.cornerRadius = scaled(16)
            iconContainer.addSubview(indicator)
            NSLayoutConstraint.activate([
                indicator.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
                indicator.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
                indicator.widthAnchor.constraint(equalToConstant: scaled(32)),
                indicator.heightAnchor.constraint(equalToConstant: scaled(32))
            ])
        }

        let icon = NSImageView()
        icon.translatesAutoresizingMaskIntoConstraints = false
        icon.imageScaling = .scaleProportionallyUpOrDown
        let path = item.icon_path.toString()
        let template = TabBarHelper.isTemplateIcon(path)
        icon.image = loadIcon(path: path)
        icon.contentTintColor = template ? tint : nil
        if !template {
            icon.wantsLayer = true
            icon.layer?.cornerRadius = scaled(6)
            icon.layer?.masksToBounds = true
        }
        iconContainer.addSubview(icon)
        stack.addArrangedSubview(iconContainer)

        if let rustItem = getTabBarItem(appId, Int32(index)) {
            let badge = rustItem.badge.toString()
            if !badge.isEmpty {
                addMarker(badgeLabel(badge), to: iconContainer, inset: scaled(-6))
            } else if rustItem.has_red_dot {
                addMarker(redDot(), to: iconContainer, inset: scaled(-4))
            }
        }

        let label = NSTextField(labelWithString: item.text.toString())
        label.font = NSFont.systemFont(ofSize: scaled(Metrics.labelSize), weight: .medium)
        label.textColor = tint
        label.alignment = .center
        label.lineBreakMode = .byTruncatingTail
        label.maximumNumberOfLines = 1
        label.usesSingleLineMode = true
        label.cell?.lineBreakMode = .byTruncatingTail
        label.cell?.truncatesLastVisibleLine = true
        label.setContentCompressionResistancePriority(.fittingSizeCompression, for: .horizontal)
        label.setContentHuggingPriority(.defaultLow, for: .horizontal)
        stack.addArrangedSubview(label)

        NSLayoutConstraint.activate([
            stack.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
            stack.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: scaled(2)),
            stack.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: scaled(-2)),
            label.widthAnchor.constraint(equalTo: stack.widthAnchor),
            iconContainer.widthAnchor.constraint(equalToConstant: scaled(Metrics.iconContainer)),
            iconContainer.heightAnchor.constraint(equalToConstant: scaled(Metrics.iconContainer)),
            icon.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
            icon.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
            icon.widthAnchor.constraint(equalToConstant: scaled(Metrics.iconSize)),
            icon.heightAnchor.constraint(equalToConstant: scaled(Metrics.iconSize))
        ])
        return cell
    }

    private func loadIcon(path: String) -> NSImage? {
        let size = scaled(Metrics.iconSize)
        if path.hasPrefix("SF:") {
            let image = NSImage(
                systemSymbolName: String(path.dropFirst(3)),
                accessibilityDescription: nil
            )
            image?.isTemplate = true
            image?.size = NSSize(width: size, height: size)
            return image
        }
        if let image = NSImage(contentsOfFile: path) {
            return TabBarHelper.appKitIcon(image, path: path, size: size)
        }
        if let image = NSImage(named: path) {
            return TabBarHelper.appKitIcon(image, path: path, size: size)
        }
        let fullPath = "\(Bundle.main.resourcePath ?? "")/\(appId)/\(path)"
        if let image = NSImage(contentsOfFile: fullPath) {
            return TabBarHelper.appKitIcon(image, path: fullPath, size: size)
        }
        let fallback = NSImage(systemSymbolName: "circle.fill", accessibilityDescription: nil)
        fallback?.isTemplate = true
        fallback?.size = NSSize(width: size, height: size)
        return fallback
    }

    private func addMarker(_ marker: NSView, to container: NSView, inset: CGFloat) {
        container.addSubview(marker)
        NSLayoutConstraint.activate([
            marker.topAnchor.constraint(equalTo: container.topAnchor, constant: inset),
            marker.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: scaled(4))
        ])
    }

    private func badgeLabel(_ text: String) -> NSView {
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor(
            red: 0xFA / 255.0,
            green: 0x51 / 255.0,
            blue: 0x51 / 255.0,
            alpha: 1.0
        ).cgColor
        container.layer?.cornerRadius = scaled(8)

        let label = NSTextField(labelWithString: text)
        label.font = NSFont.systemFont(ofSize: scaled(10), weight: .medium)
        label.textColor = .white
        label.alignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(label)

        NSLayoutConstraint.activate([
            label.topAnchor.constraint(equalTo: container.topAnchor, constant: scaled(1)),
            label.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: scaled(-1)),
            label.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: scaled(5)),
            label.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: scaled(-5)),
            container.heightAnchor.constraint(equalToConstant: scaled(16))
        ])
        return container
    }

    private func redDot() -> NSView {
        let dot = NSView()
        dot.translatesAutoresizingMaskIntoConstraints = false
        dot.wantsLayer = true
        dot.layer?.backgroundColor = NSColor(
            red: 0xFA / 255.0,
            green: 0x51 / 255.0,
            blue: 0x51 / 255.0,
            alpha: 1.0
        ).cgColor
        dot.layer?.cornerRadius = scaled(4)
        NSLayoutConstraint.activate([
            dot.widthAnchor.constraint(equalToConstant: scaled(8)),
            dot.heightAnchor.constraint(equalToConstant: scaled(8))
        ])
        return dot
    }
}

/// Consumes clicks without becoming first responder, matching a UIButton.
private class OverflowHitView: NSView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) {}
}

private final class OverflowScrimView: OverflowHitView {
    var onClick: (() -> Void)?

    override func mouseDown(with event: NSEvent) {
        onClick?()
    }
}

private final class OverflowCellView: OverflowHitView {
    var onPick: (() -> Void)?

    override func mouseDown(with event: NSEvent) {
        alphaValue = 0.55
    }

    override func mouseUp(with event: NSEvent) {
        alphaValue = 1
        let point = convert(event.locationInWindow, from: nil)
        if bounds.contains(point) {
            onPick?()
        }
    }
}
#endif

private extension Array {
    func chunked(into size: Int) -> [[Element]] {
        guard size > 0 else { return [self] }
        return stride(from: 0, to: count, by: size).map { start in
            Array(self[start..<Swift.min(start + size, count)])
        }
    }
}
