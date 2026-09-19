#if os(macOS)
import AppKit

/// Product-drawn top-right card. Not an OS notification; does not become key.
@MainActor
final class DesktopBannerController {
    static let shared = DesktopBannerController()
    static let cardWidth: CGFloat = 328

    private var panel: NSPanel?
    private var currentId = ""

    fileprivate struct Action {
        let id: String
        let label: String
        let style: String
    }

    func show(
        id: String,
        title: String,
        body: String,
        actionsJSON: String,
        background: String,
        dismissible: Bool
    ) {
        hide()
        currentId = id
        let actions = Self.parseActions(actionsJSON)
        let panel = makePanel()
        let content = BannerView(
            title: title,
            body: body,
            actions: actions,
            background: BannerBackground.parse(background),
            dismissible: dismissible,
            onAction: { [weak self] actionId in
                self?.finish(kind: 0, action: actionId)
            },
            onDismiss: { [weak self] in
                self?.finish(kind: 1, action: "")
            }
        )
        content.layoutSubtreeIfNeeded()
        panel.contentView = content
        position(panel, fitting: content.fittingSize)
        reveal(panel)
        self.panel = panel
    }

    func hide() {
        panel?.alphaValue = 0
        panel?.orderOut(nil)
        panel = nil
        currentId = ""
    }

    private func finish(kind: Int32, action: String) {
        let id = currentId
        hide()
        onDesktopBannerOutcome(id, kind, action)
    }

    private func makePanel() -> NSPanel {
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: Self.cardWidth, height: 88),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isFloatingPanel = true
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = true
        panel.isMovableByWindowBackground = false
        panel.animationBehavior = .none
        return panel
    }

    private func position(_ panel: NSPanel, fitting size: NSSize) {
        let width = Self.cardWidth
        let height = size.height
        panel.setContentSize(NSSize(width: width, height: height))
        guard let screen = NSScreen.main ?? NSScreen.screens.first else { return }
        let visible = screen.visibleFrame
        let x = visible.maxX - width - 16
        let y = visible.maxY - height - 12
        panel.setFrameOrigin(NSPoint(x: x, y: y))
    }

    private func reveal(_ panel: NSPanel) {
        let final = panel.frame
        panel.setFrame(final.offsetBy(dx: 18, dy: 0), display: false)
        panel.alphaValue = 0
        panel.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.22
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().setFrame(final, display: true)
            panel.animator().alphaValue = 1
        }
    }

    private static func parseActions(_ json: String) -> [Action] {
        guard let data = json.data(using: .utf8),
              let raw = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else {
            return []
        }
        return raw.compactMap { item in
            guard let id = item["id"] as? String, !id.isEmpty,
                  let label = item["label"] as? String, !label.isEmpty
            else {
                return nil
            }
            return Action(
                id: id,
                label: label,
                style: item["style"] as? String ?? "default"
            )
        }
    }
}

private enum BannerBackground {
    case system
    case light
    case dark
    case color(NSColor)

    static func parse(_ token: String) -> BannerBackground {
        let value = token.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if value.isEmpty || value == "system" {
            return .system
        }
        if value == "light" {
            return .light
        }
        if value == "dark" {
            return .dark
        }
        if let color = NSColor(bannerHex: value) {
            return .color(color)
        }
        return .system
    }

    var appearance: NSAppearance? {
        switch self {
        case .system:
            return nil
        case .light:
            return NSAppearance(named: .aqua)
        case .dark:
            return NSAppearance(named: .darkAqua)
        case .color(let color):
            return NSAppearance(named: color.bannerPrefersDarkContent ? .aqua : .darkAqua)
        }
    }
}

private final class BannerView: NSView {
    private let callbacks: BannerCallbacks

    init(
        title: String,
        body: String,
        actions: [DesktopBannerController.Action],
        background: BannerBackground,
        dismissible: Bool,
        onAction: @escaping (String) -> Void,
        onDismiss: @escaping () -> Void
    ) {
        self.callbacks = BannerCallbacks(onAction: onAction, onDismiss: onDismiss)
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 12
        layer?.masksToBounds = true
        appearance = background.appearance

        switch background {
        case .system, .light, .dark:
            let effect = NSVisualEffectView()
            effect.material = .popover
            effect.blendingMode = .behindWindow
            effect.state = .active
            effect.translatesAutoresizingMaskIntoConstraints = false
            addSubview(effect, positioned: .below, relativeTo: nil)
            NSLayoutConstraint.activate([
                effect.leadingAnchor.constraint(equalTo: leadingAnchor),
                effect.trailingAnchor.constraint(equalTo: trailingAnchor),
                effect.topAnchor.constraint(equalTo: topAnchor),
                effect.bottomAnchor.constraint(equalTo: bottomAnchor),
            ])
        case .color(let color):
            layer?.backgroundColor = color.cgColor
        }
        layer?.borderWidth = 0.5
        layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.55).cgColor

        let icon = NSImageView()
        icon.image = NSApp.applicationIconImage
        icon.imageScaling = .scaleProportionallyUpOrDown
        icon.wantsLayer = true
        icon.layer?.cornerRadius = 8
        icon.layer?.masksToBounds = true
        icon.translatesAutoresizingMaskIntoConstraints = false

        let titleField = NSTextField(labelWithString: title)
        titleField.font = .systemFont(ofSize: 13, weight: .semibold)
        titleField.textColor = .labelColor
        titleField.lineBreakMode = .byTruncatingTail
        titleField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        titleField.translatesAutoresizingMaskIntoConstraints = false

        let bodyField = NSTextField(wrappingLabelWithString: body)
        bodyField.font = .systemFont(ofSize: 12, weight: .regular)
        bodyField.textColor = .secondaryLabelColor
        bodyField.maximumNumberOfLines = 2
        bodyField.lineBreakMode = .byWordWrapping
        bodyField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        bodyField.translatesAutoresizingMaskIntoConstraints = false
        bodyField.isHidden = body.isEmpty

        addSubview(icon)
        addSubview(titleField)
        addSubview(bodyField)

        NSLayoutConstraint.activate([
            icon.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            icon.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            icon.widthAnchor.constraint(equalToConstant: 36),
            icon.heightAnchor.constraint(equalToConstant: 36),

            titleField.leadingAnchor.constraint(equalTo: icon.trailingAnchor, constant: 10),
            // A lone title sits on the icon's midline instead of hugging the top.
            body.isEmpty && actions.isEmpty
                ? titleField.centerYAnchor.constraint(equalTo: icon.centerYAnchor)
                : titleField.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            titleField.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: dismissible ? -34 : -12),

            bodyField.leadingAnchor.constraint(equalTo: titleField.leadingAnchor),
            bodyField.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            bodyField.topAnchor.constraint(equalTo: titleField.bottomAnchor, constant: 2),
        ])

        if dismissible {
            let close = NSButton(
                image: Self.closeImage,
                target: callbacks,
                action: #selector(BannerCallbacks.dismiss)
            )
            close.isBordered = false
            close.imagePosition = .imageOnly
            close.contentTintColor = .tertiaryLabelColor
            close.translatesAutoresizingMaskIntoConstraints = false
            addSubview(close)
            NSLayoutConstraint.activate([
                close.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -6),
                close.topAnchor.constraint(equalTo: topAnchor, constant: 6),
                close.widthAnchor.constraint(equalToConstant: 20),
                close.heightAnchor.constraint(equalToConstant: 20),
            ])
        }

        var last: NSView = body.isEmpty ? titleField : bodyField
        if !actions.isEmpty {
            let row = NSStackView()
            row.orientation = .horizontal
            row.alignment = .centerY
            row.spacing = 8
            row.translatesAutoresizingMaskIntoConstraints = false
            addSubview(row)
            for action in actions {
                let button = BannerActionButton(
                    title: action.label,
                    style: action.style,
                    target: callbacks,
                    action: #selector(BannerCallbacks.act(_:))
                )
                button.identifier = NSUserInterfaceItemIdentifier(action.id)
                row.addArrangedSubview(button)
            }
            NSLayoutConstraint.activate([
                row.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
                row.topAnchor.constraint(equalTo: last.bottomAnchor, constant: 10),
                row.leadingAnchor.constraint(greaterThanOrEqualTo: titleField.leadingAnchor),
            ])
            last = row
        }

        // The icon sets the floor: a short text column must not pull the card
        // under it and clip the icon.
        let hug = last.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12)
        hug.priority = .defaultLow
        NSLayoutConstraint.activate([
            hug,
            last.bottomAnchor.constraint(lessThanOrEqualTo: bottomAnchor, constant: -12),
            icon.bottomAnchor.constraint(lessThanOrEqualTo: bottomAnchor, constant: -12),
            widthAnchor.constraint(equalToConstant: DesktopBannerController.cardWidth),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private static var closeImage: NSImage {
        let symbol = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close")
            ?? NSImage()
        return symbol.withSymbolConfiguration(
            NSImage.SymbolConfiguration(pointSize: 9, weight: .medium)
        ) ?? symbol
    }
}

private final class BannerActionButton: NSButton {
    private let visualStyle: String

    init(title: String, style: String, target: AnyObject, action: Selector) {
        self.visualStyle = style
        super.init(frame: .zero)
        self.target = target
        self.action = action
        self.title = title
        isBordered = false
        focusRingType = .none
        font = .systemFont(ofSize: 12, weight: .medium)
        wantsLayer = true
        layer?.cornerRadius = 6
        layer?.masksToBounds = true
        translatesAutoresizingMaskIntoConstraints = false
        contentTintColor = textColor
        attributedTitle = NSAttributedString(
            string: title,
            attributes: [
                .font: font ?? .systemFont(ofSize: 12, weight: .medium),
                .foregroundColor: textColor,
            ]
        )
        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 24),
            widthAnchor.constraint(greaterThanOrEqualToConstant: 56),
        ])
        applyFill(hover: false)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var intrinsicContentSize: NSSize {
        let size = super.intrinsicContentSize
        return NSSize(width: max(56, size.width + 20), height: 24)
    }

    override func updateLayer() {
        super.updateLayer()
        applyFill(hover: isHighlighted)
        contentTintColor = textColor
    }

    override var isHighlighted: Bool {
        didSet { applyFill(hover: isHighlighted) }
    }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .pointingHand)
    }

    private var textColor: NSColor {
        visualStyle == "default" ? .labelColor : .white
    }

    private func applyFill(hover: Bool) {
        layer?.backgroundColor = fillColor(hover: hover).cgColor
    }

    private func fillColor(hover: Bool) -> NSColor {
        let boost: CGFloat = hover ? 0.08 : 0
        switch visualStyle {
        case "primary":
            return NSColor.controlAccentColor.blended(withFraction: boost, of: .white)
                ?? NSColor.controlAccentColor
        case "destructive":
            return NSColor.systemRed.blended(withFraction: boost, of: .white)
                ?? NSColor.systemRed
        default:
            return NSColor(name: nil) { appearance in
                let dark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
                let alpha: CGFloat = dark ? (hover ? 0.18 : 0.12) : (hover ? 0.10 : 0.06)
                return (dark ? NSColor.white : NSColor.black).withAlphaComponent(alpha)
            }
        }
    }
}

private final class BannerCallbacks: NSObject {
    let onAction: (String) -> Void
    let onDismiss: () -> Void

    init(onAction: @escaping (String) -> Void, onDismiss: @escaping () -> Void) {
        self.onAction = onAction
        self.onDismiss = onDismiss
    }

    @objc func dismiss() {
        onDismiss()
    }

    @objc func act(_ sender: NSButton) {
        onAction(sender.identifier?.rawValue ?? "")
    }
}

private extension NSColor {
    convenience init?(bannerHex token: String) {
        var hex = token
        if hex.hasPrefix("#") {
            hex.removeFirst()
        }
        let value = UInt32(hex, radix: 16)
        switch hex.count {
        case 3:
            guard let value else { return nil }
            let r = CGFloat((value >> 8) & 0xF) / 15
            let g = CGFloat((value >> 4) & 0xF) / 15
            let b = CGFloat(value & 0xF) / 15
            self.init(srgbRed: r, green: g, blue: b, alpha: 1)
        case 6:
            guard let value else { return nil }
            self.init(
                srgbRed: CGFloat((value >> 16) & 0xFF) / 255,
                green: CGFloat((value >> 8) & 0xFF) / 255,
                blue: CGFloat(value & 0xFF) / 255,
                alpha: 1
            )
        case 8:
            guard let value else { return nil }
            self.init(
                srgbRed: CGFloat((value >> 24) & 0xFF) / 255,
                green: CGFloat((value >> 16) & 0xFF) / 255,
                blue: CGFloat((value >> 8) & 0xFF) / 255,
                alpha: CGFloat(value & 0xFF) / 255
            )
        default:
            return nil
        }
    }

    var bannerPrefersDarkContent: Bool {
        guard let rgb = usingColorSpace(.sRGB) else { return true }
        let luminance = 0.2126 * rgb.redComponent + 0.7152 * rgb.greenComponent + 0.0722 * rgb.blueComponent
        return luminance > 0.55
    }
}
#endif
