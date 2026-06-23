import AppKit

/// The native device picker shared by both runner modes, so the selector looks
/// and behaves identically whatever device frame is active: the iPhone simulator
/// toolbar and the pad/desktop surface-shell strip both mount this same control.
///
/// It's a plain device popup (matching the iPhone toolbar). Hosts that have no
/// other chrome for it — the pad/desktop strip has no rotate button or capsule —
/// pass `extras` to append orientation / lxapp-lifecycle actions below the device
/// list; the phone toolbar leaves them empty because it has both.
@MainActor
final class RunnerDeviceSelectorControl: NSPopUpButton {
    /// An action appended below the device list. Reference type so it can ride
    /// along as a menu item's `representedObject`.
    final class ExtraItem {
        let title: String
        let systemImage: String?
        let separatorBefore: Bool
        let handler: () -> Void

        init(title: String, systemImage: String? = nil, separatorBefore: Bool = false, handler: @escaping () -> Void) {
            self.title = title
            self.systemImage = systemImage
            self.separatorBefore = separatorBefore
            self.handler = handler
        }
    }

    var onDeviceSelected: ((MobileDeviceSize) -> Void)?

    private let extras: [ExtraItem]
    private var currentDevice: MobileDeviceSize?

    init(extras: [ExtraItem] = []) {
        self.extras = extras
        super.init(frame: .zero, pullsDown: false)
        configure()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func configure() {
        translatesAutoresizingMaskIntoConstraints = false
        bezelStyle = .texturedRounded
        isBordered = false
        font = NSFont.systemFont(ofSize: 12, weight: .medium)
        contentTintColor = NSColor.white.withAlphaComponent(0.9)
        (cell as? NSPopUpButtonCell)?.arrowPosition = .arrowAtBottom
        autoenablesItems = false
        target = self
        action = #selector(selectionChanged)

        let menu = NSMenu()
        var previousShape: RunnerDeviceShape?
        for device in MobileDeviceSize.allCases {
            if let previousShape, previousShape != device.shape {
                menu.addItem(.separator())
            }
            let item = NSMenuItem()
            item.title = device.displayName
            item.representedObject = device
            menu.addItem(item)
            previousShape = device.shape
        }
        for extra in extras {
            if extra.separatorBefore { menu.addItem(.separator()) }
            let item = NSMenuItem()
            item.title = extra.title
            item.representedObject = extra
            if let symbol = extra.systemImage {
                item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
            }
            menu.addItem(item)
        }
        self.menu = menu
    }

    func setCurrentDevice(_ device: MobileDeviceSize) {
        currentDevice = device
        if let item = itemArray.first(where: { ($0.representedObject as? MobileDeviceSize)?.id == device.id }) {
            select(item)
        }
    }

    @objc private func selectionChanged() {
        switch selectedItem?.representedObject {
        case let device as MobileDeviceSize:
            currentDevice = device
            onDeviceSelected?(device)
        case let extra as ExtraItem:
            // Keep the popup titled with the device, not the action label.
            if let currentDevice { setCurrentDevice(currentDevice) }
            extra.handler()
        default:
            break
        }
    }
}
