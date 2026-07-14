#if os(macOS)
import AppKit

/// One tab in an aside slot's header strip.
struct AsideSlotTab: Equatable {
    let id: String
    let title: String
}

/// Header tab strip for an aside slot (the lxapp slot's counterpart to the
/// browser slot's title tabs): children in open order, active highlight, a
/// close glyph per tab. Always shown while the slot has content — even one
/// child keeps its tab, since the strip is the slot's management surface
/// (switch and close both live here).
@MainActor
final class AsideSlotTabStripView: NSView {
    static let stripHeight: CGFloat = 34

    var onSelect: ((String) -> Void)?
    var onClose: ((String) -> Void)?

    private let stack = NSStackView()
    private var tabs: [AsideSlotTab] = []
    private var activeId: String?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = 4
        stack.edgeInsets = NSEdgeInsets(top: 4, left: 8, bottom: 4, right: 8)
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func update(tabs: [AsideSlotTab], activeId: String?) {
        guard self.tabs != tabs || self.activeId != activeId else { return }
        self.tabs = tabs
        self.activeId = activeId
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for tab in tabs {
            let item = AsideSlotTabItemView(tab: tab, active: tab.id == activeId)
            item.onSelect = { [weak self] in self?.onSelect?(tab.id) }
            item.onClose = { [weak self] in self?.onClose?(tab.id) }
            stack.addArrangedSubview(item)
        }
    }
}

/// A single slot tab: title + close glyph, active-state background.
@MainActor
private final class AsideSlotTabItemView: NSView {
    var onSelect: (() -> Void)?
    var onClose: (() -> Void)?

    private let titleLabel: NSTextField
    private let closeButton = NSButton()

    init(tab: AsideSlotTab, active: Bool) {
        titleLabel = NSTextField(labelWithString: tab.title)
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 6
        layer?.backgroundColor = active
            ? NSColor.controlAccentColor.withAlphaComponent(0.18).cgColor
            : NSColor.clear.cgColor

        titleLabel.font = NSFont.systemFont(ofSize: 12, weight: active ? .medium : .regular)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        closeButton.bezelStyle = .inline
        closeButton.isBordered = false
        closeButton.title = "✕"
        closeButton.font = NSFont.systemFont(ofSize: 10)
        closeButton.target = self
        closeButton.action = #selector(closeTapped)
        closeButton.translatesAutoresizingMaskIntoConstraints = false

        addSubview(titleLabel)
        addSubview(closeButton)
        NSLayoutConstraint.activate([
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            titleLabel.widthAnchor.constraint(lessThanOrEqualToConstant: 140),
            closeButton.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 4),
            closeButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -6),
            closeButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            heightAnchor.constraint(equalToConstant: 26),
        ])

        let click = NSClickGestureRecognizer(target: self, action: #selector(selectTapped))
        addGestureRecognizer(click)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    @objc private func selectTapped() { onSelect?() }
    @objc private func closeTapped() { onClose?() }
}
#endif
