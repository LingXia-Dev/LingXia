#if os(macOS)
import AppKit

/// One tab in an aside slot's header strip.
struct AsideSlotTab: Equatable {
    let id: String
    let title: String
    /// Resolved icon file path (an lxapp's declared icon); nil falls back to
    /// the bundled default mark.
    var iconPath: String? = nil
}

/// Header tab strip for an aside slot — the same bar the browser slot draws:
/// Chrome-silhouette title tabs (ChromeTabRowView) bottom-aligned in a
/// bar-height strip over a hairline, identical metrics and states. Always
/// shown while the slot has content — even one child keeps its tab, since the
/// strip is the slot's management surface (switch and close both live here).
@MainActor
final class AsideSlotTabStripView: NSView {
    /// Bar + the separator hairline the active tab's feet bridge into.
    static let stripHeight: CGFloat = ChromeTabMetrics.barHeight + 1

    var onSelect: ((String) -> Void)?
    var onClose: ((String) -> Void)?

    private let stack = NSStackView()
    private let separator = NSView()
    private var tabs: [AsideSlotTab] = []
    private var activeId: String?
    private var itemViews: [AsideSlotTabItemView] = []

    /// The bundled default LingXia mark, for lxapps that declare no icon.
    private static let defaultTabIcon: NSImage? = {
        guard let url = Bundle.lingxiaResources.url(
            forResource: "lxapp_default", withExtension: "png", subdirectory: "icons")
        else { return nil }
        return NSImage(contentsOf: url)
    }()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true

        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.wantsLayer = true
        separator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        addSubview(separator)

        stack.orientation = .horizontal
        // Negative spacing overlaps the flared feet so neighbouring tabs
        // interlock like Chrome (matches the browser slot's title tabs).
        stack.spacing = -ChromeTabMetrics.tabFoot
        stack.alignment = .bottom
        stack.distribution = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

        NSLayoutConstraint.activate([
            separator.leadingAnchor.constraint(equalTo: leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: trailingAnchor),
            separator.bottomAnchor.constraint(equalTo: bottomAnchor),
            separator.heightAnchor.constraint(equalToConstant: 1),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: ChromeTabMetrics.edge),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -ChromeTabMetrics.edge),
            stack.bottomAnchor.constraint(equalTo: separator.topAnchor),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func update(tabs: [AsideSlotTab], activeId: String?) {
        guard self.tabs != tabs || self.activeId != activeId else { return }
        self.tabs = tabs
        self.activeId = activeId
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        itemViews.removeAll()
        for tab in tabs {
            let icon = tab.iconPath.flatMap { NSImage(contentsOfFile: $0) } ?? Self.defaultTabIcon
            let item = AsideSlotTabItemView(tab: tab, icon: icon)
            item.onSelect = { [weak self] in self?.onSelect?(tab.id) }
            item.onClose = { [weak self] in self?.onClose?(tab.id) }
            stack.addArrangedSubview(item)
            itemViews.append(item)
        }
        applySelection()
    }

    /// Mirror of the browser strip's selection pass: active fill + z-lift,
    /// tint steps for title/icon/close, and seam-divider suppression.
    private func applySelection() {
        for (index, item) in itemViews.enumerated() {
            let selected = tabs[index].id == activeId
            item.row.isActiveTab = selected
            item.row.layer?.zPosition = selected ? 2 : 0
            let nextSelected = index + 1 < tabs.count && tabs[index + 1].id == activeId
            item.row.suppressTrailingSeparator = selected || nextSelected
            item.applyTint(selected: selected)
        }
    }
}

/// A single slot tab assembled exactly like the browser's tab chip:
/// icon + title button (activates) + close button inside a ChromeTabRowView.
@MainActor
private final class AsideSlotTabItemView: NSView {
    var onSelect: (() -> Void)?
    var onClose: (() -> Void)?

    let row = ChromeTabRowView()
    private let iconView = NSImageView()
    private let titleButton = NSButton()
    private let closeButton = NSButton()

    init(tab: AsideSlotTab, icon: NSImage?) {
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false

        titleButton.translatesAutoresizingMaskIntoConstraints = false
        titleButton.isBordered = false
        titleButton.bezelStyle = .regularSquare
        titleButton.font = .systemFont(ofSize: 12, weight: .medium)
        titleButton.alignment = .left
        titleButton.lineBreakMode = .byTruncatingTail
        titleButton.title = tab.title
        titleButton.target = self
        titleButton.action = #selector(selectTapped)

        iconView.translatesAutoresizingMaskIntoConstraints = false
        if let icon {
            icon.size = NSSize(width: ChromeTabMetrics.iconSize, height: ChromeTabMetrics.iconSize)
            iconView.image = icon
        }
        iconView.imageScaling = .scaleProportionallyDown

        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.isBordered = false
        closeButton.imagePosition = .imageOnly
        closeButton.image = LxIcon.image(
            named: "icon_close_x", size: CGSize(width: 12, height: 12))
        closeButton.contentTintColor = .tertiaryLabelColor
        closeButton.target = self
        closeButton.action = #selector(closeTapped)

        row.orientation = .horizontal
        row.spacing = 5
        row.alignment = .centerY
        // Content clears the flared foot on the leading edge and the tab's
        // top inset, so the icon/title sit centred in the tab body.
        row.edgeInsets = NSEdgeInsets(
            top: ChromeTabMetrics.tabTopInset + 4,
            left: ChromeTabMetrics.tabFoot + 6,
            bottom: 4,
            right: ChromeTabMetrics.tabFoot
        )
        row.addArrangedSubview(iconView)
        row.addArrangedSubview(titleButton)
        row.addArrangedSubview(closeButton)
        row.translatesAutoresizingMaskIntoConstraints = false

        addSubview(row)
        titleButton.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        titleButton.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let minWidth = row.widthAnchor.constraint(
            greaterThanOrEqualToConstant: ChromeTabMetrics.minTabWidth)
        minWidth.priority = .defaultLow
        NSLayoutConstraint.activate([
            iconView.widthAnchor.constraint(equalToConstant: ChromeTabMetrics.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: ChromeTabMetrics.iconSize),
            closeButton.widthAnchor.constraint(equalToConstant: 16),
            row.heightAnchor.constraint(equalToConstant: ChromeTabMetrics.tabHeight),
            minWidth,
            row.widthAnchor.constraint(lessThanOrEqualToConstant: ChromeTabMetrics.maxTabWidth),
            row.topAnchor.constraint(equalTo: topAnchor),
            row.leadingAnchor.constraint(equalTo: leadingAnchor),
            row.trailingAnchor.constraint(equalTo: trailingAnchor),
            row.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func applyTint(selected: Bool) {
        titleButton.contentTintColor = selected ? .labelColor : .secondaryLabelColor
        closeButton.contentTintColor = selected ? .secondaryLabelColor : .tertiaryLabelColor
        closeButton.alphaValue = selected ? 1 : 0.65
    }

    @objc private func selectTapped() { onSelect?() }
    @objc private func closeTapped() { onClose?() }
}
#endif
