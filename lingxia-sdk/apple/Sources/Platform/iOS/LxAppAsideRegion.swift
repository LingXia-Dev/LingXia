#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// The side panel, as the core's plan places it: docked beside the main where the window's
/// short side is regular (an iPad), over it otherwise (an iPhone, or a maximized slot). A strip
/// switches and closes its children, a docked slot's divider drags its width, and a collapsed
/// slot leaves a tongue at the edge. The same model as Android's `AsideRegion` and the
/// desktop's aside slot.
@MainActor
final class LxAppAsideRegion {
    /// The host's region; surfaces adopt into it.
    static weak var current: LxAppAsideRegion?

    /// The core's `panel` aside slot.
    struct Slot {
        let children: [String]
        let active: String?
        let visible: Bool
        let overlay: Bool
        let collapsed: Bool
        let maximized: Bool
    }

    /// A child the surface layer owns and closes, such as a URL surface.
    private final class Child {
        let view: UIView
        let title: () -> String
        let close: () -> Void
        let isURL: Bool

        init(view: UIView, title: @escaping () -> String, close: @escaping () -> Void, isURL: Bool) {
            self.view = view
            self.title = title
            self.close = close
            self.isURL = isURL
        }
    }

    private weak var owner: LxAppViewController?
    private var children: [String: Child] = [:]
    /// Children a plan has listed: one missing from a later plan was taken out of the slot.
    private var placed = Set<String>()
    private(set) var slot: Slot?

    private let panel = UIView()
    private let strip = UIView()
    private let stripSeparator = UIView()
    private let content = UIView()
    private let headerRow = UIStackView()
    private let tabScroll = UIScrollView()
    private let tabs = UIStackView()
    private let maximizeButton = UIButton(type: .system)
    private let collapseButton = UIButton(type: .system)
    private var maximizeShowsExit = false
    private let divider = DividerView()
    private let tongue = TongueView()

    private var ratio: CGFloat = {
        let saved = UserDefaults.standard.double(forKey: LxAppAsideRegion.ratioKey)
        return saved > 0 ? CGFloat(saved) : LxAppAsideRegion.defaultRatio
    }()

    init(owner: LxAppViewController) {
        self.owner = owner
        let host = owner.view!

        panel.isHidden = true
        panel.backgroundColor = .systemBackground
        panel.clipsToBounds = true
        content.clipsToBounds = true
        panel.addSubview(content)
        panel.addSubview(strip)
        strip.addSubview(stripSeparator)
        stripSeparator.backgroundColor = .separator

        headerRow.axis = .horizontal
        headerRow.alignment = .center
        headerRow.spacing = 6
        tabs.axis = .horizontal
        tabs.alignment = .center
        tabs.spacing = 4
        tabScroll.showsHorizontalScrollIndicator = false
        tabScroll.addSubview(tabs)

        configure(maximizeButton, icon: "icon_fullscreen_enter", label: "lx_aside_maximize") { [weak self] in
            self?.toggleMaximized()
        }
        configure(collapseButton, icon: "icon_aside_collapse", label: "lx_aside_collapse") { [weak self] in
            self?.collapse()
        }
        let controls = UIStackView(arrangedSubviews: [maximizeButton, collapseButton])
        controls.axis = .horizontal
        controls.alignment = .center
        let row = UIStackView(arrangedSubviews: [headerRow, tabScroll, controls])
        row.axis = .horizontal
        row.alignment = .fill
        row.spacing = 4
        row.translatesAutoresizingMaskIntoConstraints = false
        strip.addSubview(row)
        for button in [maximizeButton, collapseButton] {
            button.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                button.widthAnchor.constraint(equalToConstant: Self.controlSize),
                button.heightAnchor.constraint(equalToConstant: Self.controlSize),
            ])
        }
        headerRow.setContentHuggingPriority(.defaultLow, for: .horizontal)
        tabScroll.setContentHuggingPriority(.defaultLow, for: .horizontal)
        controls.setContentHuggingPriority(.required, for: .horizontal)
        controls.setContentCompressionResistancePriority(.required, for: .horizontal)
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: strip.leadingAnchor, constant: 8),
            row.trailingAnchor.constraint(equalTo: strip.trailingAnchor, constant: -6),
            row.topAnchor.constraint(equalTo: strip.topAnchor),
            row.bottomAnchor.constraint(equalTo: strip.bottomAnchor),
        ])

        divider.isHidden = true
        divider.onDrag = { [weak self] x in self?.dragDivider(to: x) }
        divider.onDragEnd = { [weak self] in self?.saveRatio() }
        divider.onReset = { [weak self] in
            guard let self else { return }
            self.ratio = Self.defaultRatio
            self.saveRatio()
            self.layout()
        }
        tongue.isHidden = true
        tongue.onRestore = { [weak self] in self?.restore() }

        host.addSubview(panel)
        host.addSubview(divider)
        host.addSubview(tongue)
        Self.current = self
    }

    // MARK: - State

    /// Whether the window keeps the slot beside the main; a phone shows it over the main.
    private var canDock: Bool {
        guard let host = owner?.view else { return false }
        return min(host.bounds.width, host.bounds.height) >= Self.compactMax
    }

    private func hosts(_ id: String?) -> Bool {
        guard let id else { return false }
        return children[id] != nil
    }

    private var showing: Bool {
        guard let slot, slot.visible, !slot.collapsed, let active = slot.active else { return false }
        return hosts(active)
    }

    /// Shown over the main: a phone's slot, or a maximized one.
    var overlay: Bool {
        guard showing, let slot else { return false }
        return slot.overlay || slot.maximized || !canDock
    }

    var docked: Bool { showing && !overlay }

    // MARK: - Children

    /// Hosts a surface's [view] as the child [id]; the core's next plan places it.
    func adopt(id: String, view: UIView, isURL: Bool, title: @escaping () -> String, close: @escaping () -> Void) {
        release(id: id)
        view.isHidden = true
        view.frame = content.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        content.addSubview(view)
        children[id] = Child(view: view, title: title, close: close, isURL: isURL)
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

    // MARK: - Plan

    func apply(plan slots: [LxAppLayoutReconcileriOS.PlanAsideSlot]?) {
        let panelSlot = slots?.first { $0.kind == "panel" }
        apply(panelSlot.map {
            Slot(
                children: $0.children,
                active: $0.activeChild,
                visible: $0.visible,
                overlay: $0.overlay,
                collapsed: $0.collapsed,
                maximized: $0.maximized
            )
        })
    }

    private func apply(_ next: Slot?) {
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

    /// Places the region, its divider, and the tongue of a collapsed slot for the window as it is.
    func layout() {
        guard let owner, let host = owner.view else { return }
        let bounds = host.bounds
        let safe = host.safeAreaInsets
        let top = safe.top
        let shown = showing
        let over = overlay

        panel.isHidden = !shown
        if shown {
            let width = over ? bounds.width : dockedWidth()
            panel.frame = CGRect(x: bounds.maxX - width, y: 0, width: width, height: bounds.height)
            // The panel's fill reaches the screen edge; its strip and content stay clear of the
            // sensor housing in landscape.
            let leading = over ? safe.left : 0
            let inner = max(0, width - leading - safe.right)
            strip.frame = CGRect(x: leading, y: top, width: inner, height: Self.stripHeight)
            stripSeparator.frame = CGRect(
                x: 0, y: Self.stripHeight - Self.hairline, width: inner, height: Self.hairline)
            content.frame = CGRect(
                x: leading, y: top + Self.stripHeight, width: inner,
                height: max(0, bounds.height - top - Self.stripHeight))
            panel.layer.shadowOpacity = over ? 0.18 : 0
            panel.layer.shadowRadius = 12
            panel.layer.shadowOffset = .zero
            panel.clipsToBounds = !over
        }
        let maximized = slot?.maximized == true
        maximizeButton.isHidden = !((canDock && slot?.overlay != true) || maximized)
        // A new image each pass would invalidate the host's layout and loop.
        if maximizeShowsExit != maximized {
            maximizeShowsExit = maximized
            maximizeButton.setImage(
                Self.icon(maximized ? "icon_fullscreen_exit" : "icon_fullscreen_enter", Self.controlIconSize),
                for: .normal)
        }
        maximizeButton.accessibilityLabel = L10n.string(maximized ? "lx_aside_restore" : "lx_aside_maximize")

        let isDocked = shown && !over
        divider.isHidden = !isDocked
        if isDocked {
            let x = bounds.maxX - dockedWidth()
            divider.frame = CGRect(
                x: x - Self.dividerHit / 2, y: top, width: Self.dividerHit,
                height: bounds.height - top)
            divider.draggable = canResize
        }
        owner.setAsideInset(isDocked ? dockedWidth() : 0)

        let tongueShown = !shown && slot?.active != nil && (slot?.collapsed == true || slot?.visible == false)
        tongue.isHidden = !tongueShown
        if tongueShown {
            let title = (slot?.active ?? slot?.children.last).map(titleOf) ?? ""
            tongue.accessibilityLabel = L10n.string("lx_aside_show_content", title)
            tongue.accent = owner.asideAccentColor
            tongue.count = slot?.children.filter { hosts($0) }.count ?? 0
            let width = tongue.count > 1 ? Self.tongueHitWidthCounted : Self.tongueHitWidth
            tongue.frame = CGRect(
                x: bounds.maxX - safe.right - width, y: (bounds.height - Self.tongueHitHeight) / 2,
                width: width, height: Self.tongueHitHeight)
        }
        host.bringSubviewToFront(panel)
        host.bringSubviewToFront(divider)
        host.bringSubviewToFront(tongue)
    }

    private var windowWidth: CGFloat { owner?.view.bounds.width ?? 0 }

    /// The docked width: [ratio] of the window, leaving each side at least `sideMin`.
    private func dockedWidth() -> CGFloat { clampWidth(windowWidth * ratio) }

    /// Whether the window leaves the divider any travel; a narrow one splits it fixed in half.
    private var canResize: Bool { windowWidth - 2 * Self.sideMin >= Self.minTravel }

    private func clampWidth(_ width: CGFloat) -> CGFloat {
        let window = windowWidth
        if window < 2 * Self.sideMin { return (window / 2).rounded() }
        return min(max(width, Self.sideMin), window - Self.sideMin).rounded()
    }

    /// A drag puts the divider at [x] in the host; it snaps to the middle near it.
    private func dragDivider(to x: CGFloat) {
        let window = windowWidth
        guard window > 0 else { return }
        var width = clampWidth(window - x)
        if abs(width - window / 2) < Self.snap { width = (window / 2).rounded() }
        ratio = width / window
        layout()
    }

    private func saveRatio() {
        UserDefaults.standard.set(Double(ratio), forKey: Self.ratioKey)
    }

    // MARK: - Strip

    /// One tab per child. A single child reads as the panel's header (its name, no chip); a
    /// second child turns the strip into a tab row at the same height.
    private func bindTabs() {
        headerRow.arrangedSubviews.forEach { $0.removeFromSuperview() }
        tabs.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let ids = (slot?.children ?? []).filter { hosts($0) }
        let header = ids.count == 1
        headerRow.isHidden = !header
        tabScroll.isHidden = header
        let active = slot?.active
        var selectedTab: UIView?
        for id in ids {
            let selected = id == active
            let tab = makeTab(id: id, selected: selected, header: header)
            if header {
                headerRow.addArrangedSubview(tab)
            } else {
                tabs.addArrangedSubview(tab)
                if selected { selectedTab = tab }
            }
        }
        guard !header else { return }
        tabs.layoutIfNeeded()
        let size = tabs.systemLayoutSizeFitting(
            CGSize(width: UIView.layoutFittingCompressedSize.width, height: Self.tabHeight))
        tabs.frame = CGRect(x: 0, y: (Self.stripHeight - Self.tabHeight) / 2, width: size.width, height: Self.tabHeight)
        tabScroll.contentSize = CGSize(width: size.width, height: Self.stripHeight)
        if let selectedTab {
            DispatchQueue.main.async { [weak self] in self?.reveal(selectedTab) }
        }
    }

    /// Scrolls the strip only as far as the active chip needs, stopping at a chip's start so the
    /// first chip in view is whole, except where the strip's end leaves less than a chip to scroll.
    private func reveal(_ tab: UIView) {
        let visible = tabScroll.bounds.width
        let room = tabScroll.contentSize.width - visible
        guard visible > 0, room > 0 else {
            tabScroll.contentOffset.x = 0
            return
        }
        let frame = tab.frame
        var offset = tabScroll.contentOffset.x
        if frame.minX < offset {
            offset = frame.minX
        } else if frame.maxX > offset + visible {
            let needed = frame.maxX - visible
            let starts = tabs.arrangedSubviews.map(\.frame.minX).filter { $0 >= needed }
            offset = min(starts.min() ?? frame.minX, frame.minX)
        }
        tabScroll.contentOffset.x = min(max(0, offset), room)
    }

    private func makeTab(id: String, selected: Bool, header: Bool) -> UIView {
        let tab = TabChip()
        tab.axis = .horizontal
        tab.alignment = .center
        tab.spacing = 6
        tab.isLayoutMarginsRelativeArrangement = true
        tab.directionalLayoutMargins = NSDirectionalEdgeInsets(
            top: 0, leading: header ? 2 : 10, bottom: 0, trailing: 0)
        tab.layer.cornerRadius = 8
        tab.backgroundColor = selected && !header ? .secondarySystemFill : .clear
        if children[id]?.isURL == true {
            let globe = UIImageView(image: Self.icon("icon_globe", Self.globeSize))
            globe.tintColor = .secondaryLabel
            globe.contentMode = .scaleAspectFit
            globe.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                globe.widthAnchor.constraint(equalToConstant: Self.globeSize),
                globe.heightAnchor.constraint(equalToConstant: Self.globeSize),
            ])
            tab.addArrangedSubview(globe)
        }
        let label = UILabel()
        label.text = titleOf(id)
        label.lineBreakMode = .byTruncatingTail
        label.font = header || selected
            ? .systemFont(ofSize: header ? 15 : 13, weight: .semibold)
            : .systemFont(ofSize: 13)
        label.textColor = selected ? .label : .secondaryLabel
        label.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        if !header {
            label.widthAnchor.constraint(lessThanOrEqualToConstant: 120).isActive = true
        }
        tab.addArrangedSubview(label)
        let close = UIButton(type: .system)
        close.setImage(Self.icon("icon_close_x", Self.closeIconSize), for: .normal)
        close.tintColor = .secondaryLabel
        close.alpha = selected ? 1 : 0.65
        close.accessibilityLabel = L10n.string("lx_common_close")
        close.addAction(UIAction { [weak self] _ in self?.close(id: id) }, for: .touchUpInside)
        close.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            close.widthAnchor.constraint(equalToConstant: 28),
            close.heightAnchor.constraint(equalToConstant: 28),
        ])
        tab.addArrangedSubview(close)
        tab.translatesAutoresizingMaskIntoConstraints = false
        tab.heightAnchor.constraint(equalToConstant: Self.tabHeight).isActive = true
        if !selected {
            tab.onTap = { [weak self] in self?.focus(id: id) }
        }
        return tab
    }

    /// A strip icon from the design set, at Android's size: the control's side inset by a 4.4th on each edge.
    private static func icon(_ name: String, _ side: CGFloat) -> UIImage? {
        LxIcon.image(named: name, size: CGSize(width: side, height: side))
    }

    private func configure(
        _ button: UIButton, icon: String, label: String, action: @escaping () -> Void
    ) {
        button.setImage(Self.icon(icon, Self.controlIconSize), for: .normal)
        button.tintColor = .secondaryLabel
        button.accessibilityLabel = L10n.string(label)
        button.addAction(UIAction { _ in action() }, for: .touchUpInside)
    }

    private func titleOf(_ id: String) -> String {
        if let child = children[id] { return child.title() }
        let name = getLxAppInfo(id).app_name.toString()
        return name.isEmpty ? id : name
    }

    // MARK: - Commands

    /// Closes [id]; the surface layer lets it go and the slot drops it.
    func close(id: String) {
        children[id]?.close()
    }

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

    private func toggleMaximized() {
        guard let graph = graphAppId else { return }
        _ = setAsideSlotMaximized(graph, "panel", slot?.maximized != true)
    }

    /// The window's graph, which any of its lxapps reaches.
    private var graphAppId: String? {
        guard let appId = LxAppCore.currentAppId, !appId.isEmpty else { return nil }
        return appId
    }

    // MARK: - Metrics

    private static let compactMax: CGFloat = 600
    private static let stripHeight: CGFloat = 44
    private static let tabHeight: CGFloat = 32
    private static let controlSize: CGFloat = 36
    private static let controlIconSize: CGFloat = 20
    private static let closeIconSize: CGFloat = 15
    private static let globeSize: CGFloat = 16
    private static let sideMin: CGFloat = 320
    private static let defaultRatio: CGFloat = 0.5
    private static let snap: CGFloat = 24
    private static let minTravel: CGFloat = 48
    private static let dividerHit: CGFloat = 24
    private static let tongueHitWidth: CGFloat = 28
    private static let tongueHitWidthCounted: CGFloat = 32
    private static let tongueHitHeight: CGFloat = 88
    private static let ratioKey = "lingxia_aside.dock_ratio"
    private static var hairline: CGFloat { 1 / UIScreen.main.scale }
}

/// A strip tab: a tap focuses it.
private final class TabChip: UIStackView {
    var onTap: (() -> Void)? {
        didSet {
            if onTap != nil, gestureRecognizers?.isEmpty ?? true {
                addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped)))
            }
        }
    }

    @objc private func tapped() { onTap?() }
}

/// The docked divider: a hairline with a grip, dragged to size the slot.
private final class DividerView: UIView {
    var onDrag: ((CGFloat) -> Void)?
    var onDragEnd: (() -> Void)?
    var onReset: (() -> Void)?
    /// Without travel there is no grip, and touches fall through to the pages.
    var draggable = true {
        didSet {
            isUserInteractionEnabled = draggable
            setNeedsDisplay()
        }
    }
    private var dragging = false { didSet { setNeedsDisplay() } }

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        contentMode = .redraw
        let pan = UIPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        addGestureRecognizer(pan)
        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(reset))
        doubleTap.numberOfTapsRequired = 2
        addGestureRecognizer(doubleTap)
    }

    required init?(coder: NSCoder) { nil }

    @objc private func panned(_ gesture: UIPanGestureRecognizer) {
        guard let host = superview else { return }
        switch gesture.state {
        case .began:
            dragging = true
        case .changed:
            onDrag?(gesture.location(in: host).x)
        case .ended, .cancelled, .failed:
            dragging = false
            onDragEnd?()
        default:
            break
        }
    }

    @objc private func reset() { onReset?() }

    override func draw(_ rect: CGRect) {
        let hairline = 1 / UIScreen.main.scale
        UIColor.separator.setFill()
        UIRectFill(CGRect(x: bounds.midX - hairline / 2, y: 0, width: hairline, height: bounds.height))
        guard draggable else { return }
        UIColor.secondaryLabel.withAlphaComponent(dragging ? 0.8 : 0.45).setFill()
        let grip = CGRect(x: bounds.midX - 2, y: bounds.midY - 18, width: 4, height: 36)
        UIBezierPath(roundedRect: grip, cornerRadius: 2).fill()
    }
}

/// The desktop's collapsed-slot tongue: a trapezoid whose long side lies on the edge, with a
/// chevron toward the main and, from two children, their count. A tap or a pull toward the
/// main brings the slot back.
private final class TongueView: UIView {
    private static let arrowSize: CGFloat = 18
    var onRestore: (() -> Void)?
    var accent: UIColor = .label { didSet { setNeedsDisplay() } }
    var count = 0 { didSet { setNeedsDisplay() } }
    private var pressed = false { didSet { setNeedsDisplay() } }

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        contentMode = .redraw
        isAccessibilityElement = true
        accessibilityTraits = .button
        addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped)))
        addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(pulled(_:))))
    }

    required init?(coder: NSCoder) { nil }

    override func accessibilityActivate() -> Bool {
        onRestore?()
        return true
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesBegan(touches, with: event)
        pressed = true
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesEnded(touches, with: event)
        pressed = false
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesCancelled(touches, with: event)
        pressed = false
    }

    @objc private func tapped() { onRestore?() }

    @objc private func pulled(_ gesture: UIPanGestureRecognizer) {
        guard gesture.state == .changed, gesture.translation(in: self).x < -16 else { return }
        gesture.isEnabled = false
        gesture.isEnabled = true
        pressed = false
        onRestore?()
    }

    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
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
        context.setShadow(offset: CGSize(width: -1, height: 0), blur: 4, color: UIColor.black.withAlphaComponent(0.16).cgColor)
        context.addPath(path)
        context.setFillColor((pressed ? UIColor.secondarySystemBackground : UIColor.systemBackground).cgColor)
        context.fillPath()
        context.restoreGState()
        context.addPath(path)
        context.setStrokeColor(UIColor.separator.cgColor)
        context.setLineWidth(0.75)
        context.strokePath()

        let cx = (inner + outer) / 2
        let cy = bounds.midY - (counted ? 7 : 0)
        let side = Self.arrowSize
        LxIcon.image(named: "icon_aside_expand", size: CGSize(width: side, height: side))?
            .withTintColor(accent.withAlphaComponent(pressed ? 1 : 0.75), renderingMode: .alwaysOriginal)
            .draw(in: CGRect(x: cx - side / 2, y: cy - side / 2, width: side, height: side))

        guard counted else { return }
        let label = count > 9 ? "9+" : String(count)
        let text = NSAttributedString(string: label, attributes: [
            .font: UIFont.systemFont(ofSize: 11, weight: .bold),
            .foregroundColor: accent,
        ])
        let size = text.size()
        text.draw(at: CGPoint(x: cx - size.width / 2, y: bounds.midY + 11 - size.height / 2))
    }
}
#endif
