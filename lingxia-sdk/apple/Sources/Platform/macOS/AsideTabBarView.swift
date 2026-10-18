#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// The tabBar of an lxapp shown in an aside. A main's tab items live in the
/// sidebar, which asides never enter, so the framework draws them here as a
/// compact segmented row under the navigation bar: icon + short label,
/// scrolling when they overflow, a pop-up of the current tab when the panel is
/// too narrow for segments. Selection and clicks are the sidebar's own:
/// `selected_index` from the runtime, `TabBarClick` back to it.
@MainActor
final class AsideTabBarView: NSView {
    static let height: CGFloat = 28
    /// Below this width the row becomes a pop-up of the current tab.
    static let segmentsMinWidth: CGFloat = 240

    private let appId: String
    private let scrollView = NSScrollView()
    private let stack = NSStackView()
    private let popUp = NSPopUpButton(frame: .zero, pullsDown: false)
    private let separator = NSView()
    private var heightConstraint: NSLayoutConstraint!
    private var items: [TabBarItem] = []
    private var selectedIndex: Int32 = -1
    private var shown = false

    init(appId: String) {
        self.appId = appId
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        clipsToBounds = true

        heightConstraint = heightAnchor.constraint(equalToConstant: 0)
        heightConstraint.isActive = true

        stack.orientation = .horizontal
        stack.spacing = 2
        stack.alignment = .centerY
        stack.edgeInsets = NSEdgeInsets(top: 0, left: 8, bottom: 0, right: 8)
        stack.translatesAutoresizingMaskIntoConstraints = false

        scrollView.drawsBackground = false
        scrollView.hasHorizontalScroller = false
        scrollView.hasVerticalScroller = false
        scrollView.horizontalScrollElasticity = .allowed
        scrollView.verticalScrollElasticity = .none
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        let document = FlippedView()
        document.translatesAutoresizingMaskIntoConstraints = false
        document.addSubview(stack)
        scrollView.documentView = document
        addSubview(scrollView)

        popUp.translatesAutoresizingMaskIntoConstraints = false
        popUp.controlSize = .small
        popUp.font = .systemFont(ofSize: 12)
        popUp.target = self
        popUp.action = #selector(popUpChanged)
        popUp.isHidden = true
        addSubview(popUp)

        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.wantsLayer = true
        separator.layer?.backgroundColor = LxAppHostTheme.separator.cgColor
        addSubview(separator)

        NSLayoutConstraint.activate([
            scrollView.topAnchor.constraint(equalTo: topAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrollView.heightAnchor.constraint(equalToConstant: Self.height - 1),
            document.topAnchor.constraint(equalTo: scrollView.contentView.topAnchor),
            document.leadingAnchor.constraint(equalTo: scrollView.contentView.leadingAnchor),
            document.heightAnchor.constraint(equalTo: scrollView.contentView.heightAnchor),
            document.widthAnchor.constraint(greaterThanOrEqualTo: scrollView.contentView.widthAnchor),
            stack.topAnchor.constraint(equalTo: document.topAnchor),
            stack.bottomAnchor.constraint(equalTo: document.bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: document.leadingAnchor),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: document.trailingAnchor),
            document.widthAnchor.constraint(greaterThanOrEqualTo: stack.widthAnchor),

            popUp.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            popUp.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
            popUp.centerYAnchor.constraint(equalTo: scrollView.centerYAnchor),

            separator.leadingAnchor.constraint(equalTo: leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: trailingAnchor),
            separator.bottomAnchor.constraint(equalTo: bottomAnchor),
            separator.heightAnchor.constraint(equalToConstant: 1),
        ])
        setAccessibilityRole(.tabGroup)
        setAccessibilityLabel("Tabs")
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        separator.layer?.backgroundColor = LxAppHostTheme.separator.cgColor
        applySelection()
    }

    override func layout() {
        super.layout()
        applyForm()
    }

    /// Re-read the app's tabBar. Shown only while the current page is a tab
    /// page (the runtime's effective visibility) and there is more than one
    /// place to go.
    func refresh() {
        let tabBar = getTabBar(appId)
        let count = Int(tabBar?.items_count ?? 0)
        let next: [TabBarItem] = (0..<count).compactMap { getTabBarItem(appId, Int32($0)) }
        let visible = tabBar?.is_visible == true && next.count > 1
        let selected = tabBar?.selected_index ?? -1
        if next.map(\.index) != items.map(\.index)
            || next.map({ $0.text.toString() }) != items.map({ $0.text.toString() })
            || next.map({ $0.icon_path.toString() }) != items.map({ $0.icon_path.toString() }) {
            items = next
            rebuild()
        }
        selectedIndex = selected
        applySelection()
        setShown(visible)
    }

    private func setShown(_ visible: Bool) {
        guard shown != visible else { return }
        shown = visible
        heightConstraint.constant = visible ? Self.height : 0
        isHidden = !visible
        superview?.needsLayout = true
    }

    private func rebuild() {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        popUp.removeAllItems()
        for item in items {
            let button = NSButton(title: item.text.toString(), target: self, action: #selector(segmentClicked(_:)))
            button.tag = Int(item.index)
            button.isBordered = false
            button.bezelStyle = .regularSquare
            button.font = .systemFont(ofSize: 12, weight: .medium)
            button.wantsLayer = true
            button.layer?.cornerRadius = 6
            button.imagePosition = .imageLeading
            button.imageHugsTitle = true
            let path = item.icon_path.toString()
            if !path.isEmpty, let icon = TabBarHelper.chromeIconImage(contentsOfFile: path, maxPixelSize: 64) {
                icon.size = NSSize(width: 14, height: 14)
                icon.isTemplate = true
                button.image = icon
            }
            button.setButtonType(.momentaryChange)
            button.translatesAutoresizingMaskIntoConstraints = false
            button.heightAnchor.constraint(equalToConstant: 22).isActive = true
            button.setContentHuggingPriority(.required, for: .horizontal)
            button.setAccessibilityRole(.radioButton)
            stack.addArrangedSubview(button)
            popUp.addItem(withTitle: item.text.toString())
            popUp.lastItem?.tag = Int(item.index)
        }
        needsLayout = true
    }

    private func applySelection() {
        for case let button as NSButton in stack.arrangedSubviews {
            let selected = Int32(button.tag) == selectedIndex
            button.contentTintColor = selected ? LxAppHostTheme.accent : LxAppHostTheme.mutedForeground
            button.attributedTitle = NSAttributedString(
                string: button.title,
                attributes: [
                    .foregroundColor: selected ? LxAppHostTheme.foreground : LxAppHostTheme.mutedForeground,
                    .font: NSFont.systemFont(ofSize: 12, weight: selected ? .semibold : .medium),
                ]
            )
            button.layer?.backgroundColor = selected
                ? LxAppHostTheme.selectionBackground.cgColor
                : NSColor.clear.cgColor
            button.setAccessibilityValue(selected)
        }
        if let index = items.firstIndex(where: { $0.index == selectedIndex }) {
            popUp.selectItem(at: index)
        }
    }

    /// Segments while the panel can hold them; the current tab as a pop-up
    /// below `segmentsMinWidth`.
    private func applyForm() {
        let narrow = bounds.width > 0 && bounds.width < Self.segmentsMinWidth
        scrollView.isHidden = narrow
        popUp.isHidden = !narrow
    }

    @objc private func segmentClicked(_ sender: NSButton) {
        select(index: Int32(sender.tag))
    }

    @objc private func popUpChanged() {
        guard let item = popUp.selectedItem else { return }
        select(index: Int32(item.tag))
    }

    private func select(index: Int32) {
        guard index != selectedIndex else { return }
        _ = onLxappEvent(appId, LxAppEvent.tabBarClick, String(index))
    }
}

private final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}
#endif
