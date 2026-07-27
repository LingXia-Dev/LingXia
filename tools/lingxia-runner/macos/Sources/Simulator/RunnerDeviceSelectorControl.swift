import AppKit

/// The device picker shared by both runner modes (iPhone simulator toolbar and
/// the pad/desktop surface-shell toolbar), so the selector looks and behaves
/// identically whatever device frame is active.
///
/// A plain button (device name + chevron) that pops its menu **downward** below
/// the button — unlike `NSPopUpButton`, which centers the selected item over the
/// button and pushes the menu up off the top of the window. The menu lists
/// devices only, grouped by shape (iPhone / iPad / Desktop).
@MainActor
final class RunnerDeviceSelectorControl: NSButton {
    var onDeviceSelected: ((MobileDeviceSize) -> Void)?

    private var currentDevice: MobileDeviceSize?

    init() {
        super.init(frame: .zero)
        configure()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func configure() {
        translatesAutoresizingMaskIntoConstraints = false
        isBordered = false
        bezelStyle = .texturedRounded
        setButtonType(.momentaryChange)
        contentTintColor = NSColor.white.withAlphaComponent(0.9)
        // Chevron as a trailing SF Symbol so it baselines cleanly next to the name
        // (a text "⌄" sits unevenly).
        let config = NSImage.SymbolConfiguration(pointSize: 9, weight: .semibold)
        image = NSImage(systemSymbolName: "chevron.down", accessibilityDescription: nil)?
            .withSymbolConfiguration(config)
        imagePosition = .imageTrailing
        imageHugsTitle = true
        target = self
        action = #selector(showMenu)
    }

    func setCurrentDevice(_ device: MobileDeviceSize) {
        currentDevice = device
        attributedTitle = NSAttributedString(
            string: device.displayName,
            attributes: [
                .foregroundColor: NSColor.white.withAlphaComponent(0.9),
                .font: NSFont.systemFont(ofSize: 12, weight: .medium),
            ]
        )
    }

    @objc private func showMenu() {
        let menu = NSMenu()
        menu.autoenablesItems = false
        var previousShape: RunnerDeviceShape?
        for device in MobileDeviceSize.allCases {
            if previousShape != device.shape {
                if previousShape != nil { menu.addItem(.separator()) }
                let header = NSMenuItem(title: Self.groupTitle(device.shape), action: nil, keyEquivalent: "")
                header.isEnabled = false
                menu.addItem(header)
                previousShape = device.shape
            }
            let item = NSMenuItem(title: device.displayName, action: #selector(deviceSelected(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = device
            item.indentationLevel = 1
            item.state = device.id == currentDevice?.id ? .on : .off
            menu.addItem(item)
        }
        // Pop downward: the menu's top-left lands at the button's bottom-left.
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: -2), in: self)
    }

    @objc private func deviceSelected(_ sender: NSMenuItem) {
        guard let device = sender.representedObject as? MobileDeviceSize else { return }
        setCurrentDevice(device)
        onDeviceSelected?(device)
    }

    private static func groupTitle(_ shape: RunnerDeviceShape) -> String {
        switch shape {
        case .phone: return "iPhone"
        case .pad: return "iPad"
        case .desktop: return "Desktop"
        }
    }
}
