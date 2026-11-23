#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// The side panel on the Runner's phone screen, where a phone has no room to dock it: the
/// core's `panel` slot covers the main, a strip switches and closes its children, and a
/// collapsed slot leaves a tongue at the screen's edge. The iPhone's `LxAppAsideRegion` drawn
/// in AppKit; a pad or desktop shape docks the slot in the desktop shell instead.
@MainActor
final class LxAppPhoneAsideRegion {
    static let shared = LxAppPhoneAsideRegion()

    /// The device's status bar height on the simulated screen, which the runner supplies.
    static var statusInset: @MainActor () -> CGFloat = { 0 }

    struct Slot {
        let children: [String]
        let active: String?
        let visible: Bool
        let collapsed: Bool
        let maximized: Bool
    }

    private final class Child {
        let view: NSView
        let appId: String
        let title: () -> String
        let close: () -> Void
        let isURL: Bool

        init(view: NSView, appId: String, title: @escaping () -> String, close: @escaping () -> Void, isURL: Bool) {
            self.view = view
            self.appId = appId
            self.title = title
            self.close = close
            self.isURL = isURL
        }
    }

    private var children: [String: Child] = [:]
    /// Children a plan has listed: one missing from a later plan was taken out of the slot.
    private var placed = Set<String>()
    private(set) var slot: Slot?
    private weak var screen: NSView?

    private let panel = FlippedView()
    private let strip = FlippedView()
    private let stripSeparator = NSView()
    private let content = FlippedView()
    private let row = NSStackView()
    private let tabScroll = NSScrollView()
    private let tabs = NSStackView()
    private let collapseButton = NSButton()
    private let tongue = PhoneTongueView()

    private init() {
        panel.isHidden = true
        panel.wantsLayer = true
        panel.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        panel.onLayout = { [weak self] in self?.layoutPanel() }
        content.wantsLayer = true
        content.layer?.masksToBounds = true
        panel.addSubview(content)
        panel.addSubview(strip)
        stripSeparator.wantsLayer = true
        stripSeparator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        strip.addSubview(stripSeparator)

        tabs.orientation = .horizontal
        tabs.alignment = .centerY
        tabs.spacing = 4
        tabScroll.drawsBackground = false
        tabScroll.hasHorizontalScroller = false
        tabScroll.hasVerticalScroller = false
        tabScroll.horizontalScrollElasticity = .allowed
        tabScroll.verticalScrollElasticity = .none
        let document = FlippedView()
        document.addSubview(tabs)
        tabScroll.documentView = document
        tabScroll.translatesAutoresizingMaskIntoConstraints = false
        tabScroll.heightAnchor.constraint(equalToConstant: Self.stripHeight).isActive = true

        collapseButton.bezelStyle = .regularSquare
        collapseButton.isBordered = false
        collapseButton.image = NSImage(
            systemSymbolName: "sidebar.right",
            accessibilityDescription: L10n.string("lx_aside_collapse"))
        collapseButton.contentTintColor = .secondaryLabelColor
        collapseButton.toolTip = L10n.string("lx_aside_collapse")
        collapseButton.target = self
        collapseButton.action = #selector(collapseClicked)
        collapseButton.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            collapseButton.widthAnchor.constraint(equalToConstant: Self.controlSize),
            collapseButton.heightAnchor.constraint(equalToConstant: Self.controlSize),
        ])

        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 4
        row.edgeInsets = NSEdgeInsets(top: 0, left: 8, bottom: 0, right: 6)
        strip.addSubview(row)

        tongue.isHidden = true
        tongue.onRestore = { [weak self] in self?.restore() }
    }

    // MARK: - State

    private func hosts(_ id: String?) -> Bool {
        guard let id else { return false }
        return children[id] != nil
    }

    private var showing: Bool {
        guard let slot, slot.visible, !slot.collapsed, let active = slot.active else { return false }
        return hosts(active)
    }

    // MARK: - Children

    /// Hosts a surface's [view] on [screen] as the child [id]; the core's next plan places it.
    func adopt(
        id: String,
        appId: String,
        view: NSView,
        on screen: NSView,
        isURL: Bool,
        title: @escaping () -> String,
        close: @escaping () -> Void
    ) {
        release(id: id)
        attach(to: screen)
        view.isHidden = true
        view.frame = content.bounds
        view.autoresizingMask = [.width, .height]
        content.addSubview(view)
        children[id] = Child(view: view, appId: appId, title: title, close: close, isURL: isURL)
        apply(slot)
    }

    /// Lets go of a child; the surface layer tears its view down.
    func release(id: String) {
        guard let child = children.removeValue(forKey: id) else { return }
        placed.remove(id)
        child.view.removeFromSuperview()
        apply(slot)
    }

    func hostsChild(_ id: String) -> Bool { children[id] != nil }

    private func attach(to screen: NSView) {
        guard self.screen !== screen else { return }
        self.screen = screen
        panel.removeFromSuperview()
        tongue.removeFromSuperview()
        screen.addSubview(panel)
        screen.addSubview(tongue)
    }

    // MARK: - Plan

    func apply(_ next: Slot?) {
        slot = next
        let listed = next?.children ?? []
        if next != nil {
            placed.formUnion(listed.filter { hosts($0) })
        }
        // The plan dropped a child: the surface layer closes it.
        for id in placed.subtracting(listed) {
            placed.remove(id)
            children[id]?.close()
        }
        let active = next?.active
        for (id, child) in children {
            child.view.isHidden = !(id == active && showing)
        }
        bindTabs()
        layout()
    }

    // MARK: - Layout

    /// Covers the screen with the panel, or leaves the tongue of a collapsed slot at its edge.
    func layout() {
        guard let screen else { return }
        let bounds = screen.bounds
        let shown = showing
        panel.isHidden = !shown
        if shown {
            panel.frame = bounds
            panel.autoresizingMask = [.width, .height]
            // Above whatever the runner laid over the page since.
            screen.addSubview(panel, positioned: .above, relativeTo: nil)
            layoutPanel()
        }

        let tongueShown = !shown && slot?.active != nil && (slot?.collapsed == true || slot?.visible == false)
        tongue.isHidden = !tongueShown
        if tongueShown {
            let title = (slot?.active ?? slot?.children.last).map(titleOf) ?? ""
            tongue.setAccessibilityLabel(L10n.string("lx_aside_show_content", title))
            tongue.toolTip = tongue.accessibilityLabel()
            tongue.accent = accentColor
            tongue.count = slot?.children.filter { hosts($0) }.count ?? 0
            let width = tongue.count > 1 ? Self.tongueHitWidthCounted : Self.tongueHitWidth
            let y = (bounds.height - Self.tongueHitHeight) / 2
            tongue.frame = NSRect(x: bounds.maxX - width, y: y, width: width, height: Self.tongueHitHeight)
            tongue.autoresizingMask = [.minXMargin, .minYMargin, .maxYMargin]
            screen.addSubview(tongue, positioned: .above, relativeTo: nil)
        }
    }

    private func layoutPanel() {
        let bounds = panel.bounds
        let top = Self.statusInset()
        strip.frame = NSRect(x: 0, y: top, width: bounds.width, height: Self.stripHeight)
        stripSeparator.frame = NSRect(x: 0, y: Self.stripHeight - 1, width: bounds.width, height: 1)
        row.frame = strip.bounds
        content.frame = NSRect(
            x: 0, y: top + Self.stripHeight, width: bounds.width,
            height: max(0, bounds.height - top - Self.stripHeight))
    }

    // MARK: - Strip

    /// One tab per child. A single child reads as the panel's header (its name, no chip); a
    /// second child turns the strip into a tab row at the same height.
    private func bindTabs() {
        row.arrangedSubviews.forEach { $0.removeFromSuperview() }
        tabs.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let ids = (slot?.children ?? []).filter { hosts($0) }
        let header = ids.count == 1
        let active = slot?.active
        if header, let id = ids.first {
            let tab = makeTab(id: id, selected: true, header: true)
            row.addArrangedSubview(tab)
            tab.setContentHuggingPriority(.defaultLow, for: .horizontal)
        } else {
            var selectedTab: NSView?
            for id in ids {
                let tab = makeTab(id: id, selected: id == active, header: false)
                tabs.addArrangedSubview(tab)
                if id == active { selectedTab = tab }
            }
            let size = tabs.fittingSize
            tabs.frame = NSRect(x: 0, y: (Self.stripHeight - Self.tabHeight) / 2, width: size.width, height: Self.tabHeight)
            tabScroll.documentView?.frame = NSRect(x: 0, y: 0, width: size.width, height: Self.stripHeight)
            row.addArrangedSubview(tabScroll)
            tabScroll.setContentHuggingPriority(.defaultLow, for: .horizontal)
            if let selectedTab {
                DispatchQueue.main.async {
                    selectedTab.scrollToVisible(selectedTab.bounds.insetBy(dx: -8, dy: 0))
                }
            }
        }
        row.addArrangedSubview(collapseButton)
        collapseButton.setContentHuggingPriority(.required, for: .horizontal)
        collapseButton.setContentCompressionResistancePriority(.required, for: .horizontal)
    }

    private func makeTab(id: String, selected: Bool, header: Bool) -> NSView {
        let tab = PhoneTabChip()
        tab.orientation = .horizontal
        tab.alignment = .centerY
        tab.spacing = 6
        tab.edgeInsets = NSEdgeInsets(top: 0, left: header ? 2 : 10, bottom: 0, right: 0)
        tab.wantsLayer = true
        tab.layer?.cornerRadius = 8
        tab.layer?.backgroundColor = selected && !header
            ? NSColor.labelColor.withAlphaComponent(0.06).cgColor
            : NSColor.clear.cgColor
        if children[id]?.isURL == true {
            let globe = NSImageView(image: NSImage(systemSymbolName: "globe", accessibilityDescription: nil) ?? NSImage())
            globe.contentTintColor = .secondaryLabelColor
            globe.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                globe.widthAnchor.constraint(equalToConstant: 15),
                globe.heightAnchor.constraint(equalToConstant: 15),
            ])
            tab.addArrangedSubview(globe)
        }
        let label = NSTextField(labelWithString: titleOf(id))
        label.lineBreakMode = .byTruncatingTail
        label.font = header || selected
            ? .systemFont(ofSize: header ? 15 : 13, weight: .semibold)
            : .systemFont(ofSize: 13)
        label.textColor = selected ? .labelColor : .secondaryLabelColor
        label.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        if !header {
            label.widthAnchor.constraint(lessThanOrEqualToConstant: 120).isActive = true
        }
        tab.addArrangedSubview(label)
        let close = PhoneTabCloseButton()
        close.bezelStyle = .regularSquare
        close.isBordered = false
        close.image = NSImage(
            systemSymbolName: "xmark",
            accessibilityDescription: L10n.string("lx_common_close"))?
            .withSymbolConfiguration(.init(pointSize: 11, weight: .medium))
        close.contentTintColor = .secondaryLabelColor
        close.alphaValue = selected ? 1 : 0.65
        close.onClick = { [weak self] in self?.close(id: id) }
        close.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            close.widthAnchor.constraint(equalToConstant: 28),
            close.heightAnchor.constraint(equalToConstant: 28),
        ])
        tab.addArrangedSubview(close)
        tab.translatesAutoresizingMaskIntoConstraints = false
        tab.heightAnchor.constraint(equalToConstant: Self.tabHeight).isActive = true
        if !selected {
            tab.onClick = { [weak self] in self?.focus(id: id) }
        }
        return tab
    }

    private func titleOf(_ id: String) -> String {
        if let child = children[id] { return child.title() }
        let name = getLxAppInfo(id).app_name.toString()
        return name.isEmpty ? id : name
    }

    private var accentColor: NSColor {
        if let appId = graphAppId, let tabBar = getTabBar(appId) {
            return PlatformColor(argb: tabBar.selected_color)
        }
        return .controlAccentColor
    }

    // MARK: - Commands

    /// Closes [id]; the surface layer lets it go and the slot drops it.
    func close(id: String) {
        children[id]?.close()
    }

    @objc private func collapseClicked() { collapse() }

    func collapse() {
        guard let graph = graphAppId else { return }
        if slot?.maximized == true { _ = setAsideSlotMaximized(graph, "panel", false) }
        _ = collapseAsideSlot(graph, "panel")
    }

    func restore() {
        guard let graph = graphAppId else { return }
        _ = restoreAsideSlot(graph, "panel")
    }

    private func focus(id: String) {
        guard let graph = graphAppId else { return }
        _ = focusSurface(graph, id)
    }

    /// The window's graph, which any of its lxapps reaches.
    private var graphAppId: String? {
        if let appId = LxAppCore.currentAppId, !appId.isEmpty { return appId }
        return children.values.first?.appId
    }

    // MARK: - Metrics

    private static let stripHeight: CGFloat = 44
    private static let tabHeight: CGFloat = 32
    private static let controlSize: CGFloat = 36
    private static let tongueHitWidth: CGFloat = 28
    private static let tongueHitWidthCounted: CGFloat = 32
    private static let tongueHitHeight: CGFloat = 88
}

/// Lays out top-down like the phone it stands for.
private final class FlippedView: NSView {
    var onLayout: (() -> Void)?

    override var isFlipped: Bool { true }

    override func layout() {
        super.layout()
        onLayout?()
    }
}

/// A strip tab: a click focuses it.
private final class PhoneTabChip: NSStackView {
    var onClick: (() -> Void)?

    override func mouseUp(with event: NSEvent) {
        guard let onClick, bounds.contains(convert(event.locationInWindow, from: nil)) else {
            super.mouseUp(with: event)
            return
        }
        onClick()
    }

    override func mouseDown(with event: NSEvent) {
        // Claim the press so the matching mouseUp comes here.
        if onClick == nil { super.mouseDown(with: event) }
    }
}

private final class PhoneTabCloseButton: NSButton {
    var onClick: (() -> Void)? {
        didSet {
            target = self
            action = #selector(clicked)
        }
    }

    @objc private func clicked() { onClick?() }
}

/// The desktop's collapsed-slot tongue: a trapezoid whose long side lies on the edge, with a
/// chevron toward the main and, from two children, their count. A click or a pull toward the
/// main brings the slot back.
private final class PhoneTongueView: NSView {
    var onRestore: (() -> Void)?
    var accent: NSColor = .controlAccentColor { didSet { needsDisplay = true } }
    var count = 0 { didSet { needsDisplay = true } }
    private var pressed = false { didSet { needsDisplay = true } }
    private var pressX: CGFloat = 0
    private var pulled = false

    override var isFlipped: Bool { true }

    override init(frame: NSRect) {
        super.init(frame: frame)
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
    }

    required init?(coder: NSCoder) { nil }

    override func accessibilityPerformPress() -> Bool {
        onRestore?()
        return true
    }

    override func mouseDown(with event: NSEvent) {
        pressed = true
        pulled = false
        pressX = event.locationInWindow.x
    }

    override func mouseDragged(with event: NSEvent) {
        guard !pulled, event.locationInWindow.x - pressX < -16 else { return }
        pulled = true
        pressed = false
        onRestore?()
    }

    override func mouseUp(with event: NSEvent) {
        let wasPressed = pressed
        pressed = false
        guard wasPressed, !pulled, bounds.contains(convert(event.locationInWindow, from: nil)) else { return }
        onRestore?()
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        let counted = count > 1
        let outer = bounds.maxX
        let inner = outer - (counted ? 18 : 14)
        let height: CGFloat = 64
        let slope: CGFloat = 10
        let top = (bounds.height - height) / 2
        let bottom = top + height
        let path = CGMutablePath()
        path.move(to: CGPoint(x: outer, y: top))
        path.addArc(
            tangent1End: CGPoint(x: inner, y: top + slope),
            tangent2End: CGPoint(x: inner, y: bottom - slope), radius: 3)
        path.addArc(
            tangent1End: CGPoint(x: inner, y: bottom - slope),
            tangent2End: CGPoint(x: outer, y: bottom), radius: 3)
        path.addLine(to: CGPoint(x: outer, y: bottom))
        path.closeSubpath()

        context.saveGState()
        context.setShadow(
            offset: CGSize(width: -1, height: 0), blur: 4,
            color: NSColor.black.withAlphaComponent(0.16).cgColor)
        context.addPath(path)
        context.setFillColor((pressed ? NSColor.controlBackgroundColor : NSColor.windowBackgroundColor).cgColor)
        context.fillPath()
        context.restoreGState()
        context.addPath(path)
        context.setStrokeColor(NSColor.separatorColor.cgColor)
        context.setLineWidth(0.75)
        context.strokePath()

        let cx = (inner + outer) / 2
        let cy = bounds.midY - (counted ? 7 : 0)
        let chevron = NSBezierPath()
        chevron.move(to: NSPoint(x: cx + 2.5, y: cy - 4))
        chevron.line(to: NSPoint(x: cx - 2.5, y: cy))
        chevron.line(to: NSPoint(x: cx + 2.5, y: cy + 4))
        chevron.lineWidth = 1.6
        chevron.lineCapStyle = .round
        chevron.lineJoinStyle = .round
        accent.withAlphaComponent(pressed ? 1 : 0.75).setStroke()
        chevron.stroke()

        guard counted else { return }
        let label = count > 9 ? "9+" : String(count)
        let text = NSAttributedString(string: label, attributes: [
            .font: NSFont.systemFont(ofSize: 11, weight: .bold),
            .foregroundColor: accent,
        ])
        let size = text.size()
        text.draw(at: NSPoint(x: cx - size.width / 2, y: bounds.midY + 11 - size.height / 2))
    }
}
#endif
