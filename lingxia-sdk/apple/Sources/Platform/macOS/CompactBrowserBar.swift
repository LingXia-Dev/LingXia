#if os(macOS)
import AppKit

/// Browser chrome used when the desktop shell projects into the compact size
/// class. The provider owns browser state; this view only renders controls and
/// forwards semantic actions so main and aside browsers share one compact UI.
@MainActor
final class CompactBrowserBar: NSView {
    enum Mode: Equatable {
        case main(dismissible: Bool)
        case aside

        var hasAddressRow: Bool {
            if case .main = self { return true }
            return false
        }

        var canCreateTab: Bool {
            if case .main = self { return true }
            return false
        }

        var isDismissible: Bool {
            switch self {
            case .main(let dismissible): return dismissible
            case .aside: return true
            }
        }
    }

    struct TabItem {
        let id: String
        let title: String
        let active: Bool
    }

    static let mainHeight: CGFloat = 96
    static let asideHeight: CGFloat = 56

    var onBack: (() -> Void)?
    var onForward: (() -> Void)?
    var onReload: (() -> Void)?
    var onNewTab: (() -> Void)?
    var onSelectTab: ((String) -> Void)?
    var onCloseTab: ((String) -> Void)?
    var onDismiss: (() -> Void)?
    var onSubmitAddress: ((String) -> Void)?

    private var mode: Mode
    private var tabs: [TabItem] = []

    private let background = NSVisualEffectView()
    private let addressPill = NSView()
    private let addressField = NSTextField()
    private let addressReloadButton = NSButton()
    private let actionRow = NSStackView()
    private let backButton = NSButton()
    private let forwardButton = NSButton()
    private let rowReloadButton = NSButton()
    private let newTabButton = NSButton()
    private let tabsButton = NSButton()
    private let tabsBadge = NSTextField(labelWithString: "0")
    private let dismissButton = NSButton()
    private var actionTopWithAddress: NSLayoutConstraint?
    private var actionTopWithoutAddress: NSLayoutConstraint?
    private weak var tabSwitcherOverlay: NSView?

    var preferredHeight: CGFloat {
        mode.hasAddressRow ? Self.mainHeight : Self.asideHeight
    }

    init(mode: Mode) {
        self.mode = mode
        super.init(frame: .zero)
        buildChrome()
        applyMode()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setMode(_ mode: Mode) {
        guard self.mode != mode else { return }
        self.mode = mode
        dismissTabSwitcher()
        applyMode()
    }

    func update(
        address: String,
        canGoBack: Bool,
        canGoForward: Bool,
        canReload: Bool,
        tabs: [TabItem]
    ) {
        self.tabs = tabs
        if addressField.currentEditor() == nil {
            addressField.stringValue = address
        }
        applyButtonState(backButton, enabled: canGoBack)
        applyButtonState(forwardButton, enabled: canGoForward)
        applyButtonState(addressReloadButton, enabled: canReload)
        applyButtonState(rowReloadButton, enabled: canReload)
        tabsBadge.stringValue = String(min(tabs.count, 99))
        tabsButton.toolTip = L10n.string("lx_browser_tabs")
        tabsButton.setAccessibilityLabel(L10n.string("lx_browser_tabs"))
    }

    func dismissTabSwitcher() {
        tabSwitcherOverlay?.removeFromSuperview()
    }

    private func buildChrome() {
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true

        background.translatesAutoresizingMaskIntoConstraints = false
        background.material = .hudWindow
        background.blendingMode = .withinWindow
        background.state = .active
        background.wantsLayer = true
        addSubview(background)

        addressPill.translatesAutoresizingMaskIntoConstraints = false
        addressPill.wantsLayer = true
        addressPill.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.07).cgColor
        addressPill.layer?.cornerRadius = 16
        addressPill.layer?.borderWidth = 1
        addressPill.layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.7).cgColor
        addressPill.layer?.masksToBounds = true
        background.addSubview(addressPill)

        addressField.translatesAutoresizingMaskIntoConstraints = false
        addressField.isBordered = false
        addressField.drawsBackground = false
        addressField.focusRingType = .none
        addressField.font = NSFont.systemFont(ofSize: 13)
        addressField.placeholderString = L10n.string("lx_browser_address_placeholder")
        addressField.lineBreakMode = .byTruncatingMiddle
        addressField.usesSingleLineMode = true
        addressField.target = self
        addressField.action = #selector(addressSubmitted)
        addressPill.addSubview(addressField)

        configureButton(
            addressReloadButton,
            iconName: "icon_browser_refresh",
            label: L10n.string("lx_common_refresh"),
            action: #selector(reloadClicked),
            side: 30
        )
        addressPill.addSubview(addressReloadButton)

        actionRow.translatesAutoresizingMaskIntoConstraints = false
        actionRow.orientation = .horizontal
        actionRow.alignment = .centerY
        actionRow.spacing = 4
        background.addSubview(actionRow)

        configureButton(
            backButton,
            iconName: "icon_back",
            label: L10n.string("lx_common_back"),
            action: #selector(backClicked)
        )
        configureButton(
            forwardButton,
            iconName: "icon_forward",
            label: L10n.string("lx_common_forward"),
            action: #selector(forwardClicked)
        )
        configureButton(
            rowReloadButton,
            iconName: "icon_browser_refresh",
            label: L10n.string("lx_common_refresh"),
            action: #selector(reloadClicked)
        )
        configureButton(
            newTabButton,
            iconName: "icon_browser_plus",
            label: L10n.string("lx_browser_new_tab"),
            action: #selector(newTabClicked)
        )
        configureButton(
            tabsButton,
            iconName: "icon_browser_tabs",
            label: L10n.string("lx_browser_tabs"),
            action: #selector(tabsClicked)
        )
        configureButton(
            dismissButton,
            iconName: "icon_close_x",
            label: L10n.string("lx_common_close"),
            action: #selector(dismissClicked)
        )

        let spacer = NSView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)

        actionRow.addArrangedSubview(backButton)
        actionRow.addArrangedSubview(forwardButton)
        actionRow.addArrangedSubview(rowReloadButton)
        actionRow.addArrangedSubview(spacer)
        actionRow.addArrangedSubview(newTabButton)
        actionRow.addArrangedSubview(tabsButton)
        actionRow.addArrangedSubview(dismissButton)

        tabsBadge.translatesAutoresizingMaskIntoConstraints = false
        tabsBadge.font = NSFont.systemFont(ofSize: 9, weight: .semibold)
        tabsBadge.textColor = LxAppHostTheme.foreground
        tabsBadge.alignment = .center
        tabsBadge.isEditable = false
        tabsBadge.isSelectable = false
        tabsBadge.setAccessibilityElement(false)
        tabsButton.addSubview(tabsBadge)

        NSLayoutConstraint.activate([
            background.topAnchor.constraint(equalTo: topAnchor),
            background.leadingAnchor.constraint(equalTo: leadingAnchor),
            background.trailingAnchor.constraint(equalTo: trailingAnchor),
            background.bottomAnchor.constraint(equalTo: bottomAnchor),

            addressPill.leadingAnchor.constraint(equalTo: background.leadingAnchor, constant: 10),
            addressPill.trailingAnchor.constraint(equalTo: background.trailingAnchor, constant: -10),
            addressPill.topAnchor.constraint(equalTo: background.topAnchor, constant: 8),
            addressPill.heightAnchor.constraint(equalToConstant: 34),

            addressField.leadingAnchor.constraint(equalTo: addressPill.leadingAnchor, constant: 12),
            addressField.trailingAnchor.constraint(equalTo: addressReloadButton.leadingAnchor, constant: -4),
            addressField.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),

            addressReloadButton.trailingAnchor.constraint(equalTo: addressPill.trailingAnchor, constant: -2),
            addressReloadButton.centerYAnchor.constraint(equalTo: addressPill.centerYAnchor),

            actionRow.leadingAnchor.constraint(equalTo: background.leadingAnchor, constant: 6),
            actionRow.trailingAnchor.constraint(equalTo: background.trailingAnchor, constant: -6),

            tabsBadge.centerXAnchor.constraint(equalTo: tabsButton.centerXAnchor, constant: 1),
            tabsBadge.centerYAnchor.constraint(equalTo: tabsButton.centerYAnchor, constant: -1),
        ])

        let withAddress = actionRow.topAnchor.constraint(equalTo: addressPill.bottomAnchor, constant: 4)
        let withoutAddress = actionRow.topAnchor.constraint(equalTo: background.topAnchor, constant: 8)
        actionTopWithAddress = withAddress
        actionTopWithoutAddress = withoutAddress
        withAddress.isActive = true
    }

    private func applyMode() {
        let hasAddress = mode.hasAddressRow
        addressPill.isHidden = !hasAddress
        rowReloadButton.isHidden = hasAddress
        newTabButton.isHidden = !mode.canCreateTab
        dismissButton.isHidden = !mode.isDismissible
        actionTopWithAddress?.isActive = hasAddress
        actionTopWithoutAddress?.isActive = !hasAddress
        needsLayout = true
    }

    private func configureButton(
        _ button: NSButton,
        iconName: String,
        label: String,
        action: Selector,
        side: CGFloat = 38
    ) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.title = ""
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.imagePosition = .imageOnly
        button.imageScaling = .scaleProportionallyDown
        button.image = LxIcon.image(named: iconName, size: CGSize(width: 20, height: 20))
        button.contentTintColor = LxAppHostTheme.foreground.withAlphaComponent(0.85)
        button.toolTip = label
        button.setAccessibilityLabel(label)
        button.target = self
        button.action = action
        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: side),
            button.heightAnchor.constraint(equalToConstant: side),
        ])
    }

    private func applyButtonState(_ button: NSButton, enabled: Bool) {
        button.isEnabled = enabled
        button.alphaValue = enabled ? 1 : 0.35
    }

    @objc private func backClicked() { onBack?() }
    @objc private func forwardClicked() { onForward?() }
    @objc private func reloadClicked() { onReload?() }
    @objc private func newTabClicked() { onNewTab?() }
    @objc private func dismissClicked() { onDismiss?() }

    @objc private func addressSubmitted() {
        onSubmitAddress?(addressField.stringValue)
    }

    @objc private func tabsClicked() {
        if tabSwitcherOverlay != nil {
            dismissTabSwitcher()
            return
        }
        guard let host = superview else { return }
        presentTabSwitcher(in: host)
    }

    private func presentTabSwitcher(in host: NSView) {
        let backdrop = CompactBrowserTabSwitcherBackdrop()
        backdrop.translatesAutoresizingMaskIntoConstraints = false
        backdrop.wantsLayer = true
        backdrop.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.35).cgColor
        backdrop.onDismiss = { [weak self] in self?.dismissTabSwitcher() }
        host.addSubview(backdrop, positioned: .above, relativeTo: nil)

        let panel = CompactBrowserTabSwitcherPanel()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.material = .hudWindow
        panel.blendingMode = .withinWindow
        panel.state = .active
        panel.wantsLayer = true
        panel.layer?.cornerRadius = 18
        panel.layer?.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        panel.layer?.masksToBounds = true
        backdrop.addSubview(panel)

        let title = NSTextField(labelWithString: L10n.string("lx_browser_tabs"))
        title.translatesAutoresizingMaskIntoConstraints = false
        title.font = NSFont.systemFont(ofSize: 15, weight: .semibold)
        panel.addSubview(title)

        let scroll = NSScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        panel.addSubview(scroll)

        let list = NSStackView()
        list.translatesAutoresizingMaskIntoConstraints = false
        list.orientation = .vertical
        list.alignment = .leading
        list.spacing = 2
        scroll.documentView = list

        let desiredHeight = min(
            max(120, CGFloat(tabs.count) * 42 + 58),
            max(120, host.bounds.height * 0.6)
        )
        NSLayoutConstraint.activate([
            backdrop.topAnchor.constraint(equalTo: host.topAnchor),
            backdrop.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            backdrop.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            backdrop.bottomAnchor.constraint(equalTo: host.bottomAnchor),

            panel.leadingAnchor.constraint(equalTo: backdrop.leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: backdrop.trailingAnchor),
            panel.bottomAnchor.constraint(equalTo: backdrop.bottomAnchor),
            panel.heightAnchor.constraint(equalToConstant: desiredHeight),

            title.topAnchor.constraint(equalTo: panel.topAnchor, constant: 14),
            title.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 16),

            scroll.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 8),
            scroll.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 8),
            scroll.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -8),
            scroll.bottomAnchor.constraint(equalTo: panel.bottomAnchor, constant: -12),

            list.topAnchor.constraint(equalTo: scroll.contentView.topAnchor),
            list.leadingAnchor.constraint(equalTo: scroll.contentView.leadingAnchor),
            list.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor),
            list.heightAnchor.constraint(equalToConstant: max(1, CGFloat(tabs.count) * 42)),
        ])

        tabSwitcherOverlay = backdrop
        for tab in tabs {
            let row = makeSwitcherRow(tab)
            list.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: list.widthAnchor).isActive = true
        }
    }

    private func makeSwitcherRow(_ tab: TabItem) -> NSView {
        let row = CompactBrowserTabSwitcherRow()
        row.translatesAutoresizingMaskIntoConstraints = false
        row.onSelect = { [weak self] in
            self?.dismissTabSwitcher()
            self?.onSelectTab?(tab.id)
        }

        let resolvedTitle = tab.title.isEmpty ? L10n.string("lx_browser_new_tab") : tab.title
        let label = NSTextField(labelWithString: resolvedTitle)
        label.translatesAutoresizingMaskIntoConstraints = false
        label.font = NSFont.systemFont(ofSize: 13, weight: tab.active ? .semibold : .regular)
        label.textColor = tab.active ? LxAppHostTheme.foreground : LxAppHostTheme.mutedForeground
        label.lineBreakMode = .byTruncatingTail
        label.isEditable = false
        label.isSelectable = false
        label.drawsBackground = false
        label.isBordered = false
        row.addSubview(label)

        let close = NSButton()
        configureButton(
            close,
            iconName: "icon_close_x",
            label: L10n.string("lx_common_close"),
            action: #selector(closeTabFromSwitcher(_:)),
            side: 28
        )
        close.identifier = NSUserInterfaceItemIdentifier(tab.id)
        row.addSubview(close)

        NSLayoutConstraint.activate([
            row.heightAnchor.constraint(equalToConstant: 40),
            label.leadingAnchor.constraint(equalTo: row.leadingAnchor, constant: 10),
            label.trailingAnchor.constraint(equalTo: close.leadingAnchor, constant: -8),
            label.centerYAnchor.constraint(equalTo: row.centerYAnchor),
            close.trailingAnchor.constraint(equalTo: row.trailingAnchor, constant: -8),
            close.centerYAnchor.constraint(equalTo: row.centerYAnchor),
        ])
        return row
    }

    @objc private func closeTabFromSwitcher(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        dismissTabSwitcher()
        onCloseTab?(id)
    }
}

@MainActor
private final class CompactBrowserTabSwitcherRow: NSView {
    var onSelect: (() -> Void)?

    override func mouseDown(with event: NSEvent) {
        onSelect?()
    }
}

@MainActor
private final class CompactBrowserTabSwitcherBackdrop: NSView {
    var onDismiss: (() -> Void)?

    override func mouseDown(with event: NSEvent) {
        onDismiss?()
    }
}

@MainActor
private final class CompactBrowserTabSwitcherPanel: NSVisualEffectView {
    override func mouseDown(with event: NSEvent) {}
}
#endif
