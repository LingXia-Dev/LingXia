import Foundation

#if os(iOS)
import UIKit

@_silgen_name("lingxia_dev_service_banner")
private func lingxiaDevServiceBanner() -> Int32

/// Prod build on the dev service. Guest lxapps and the auth sheet live in
/// the app window (and some host surfaces sit at `.alert + 1`), so the chip
/// is its own window above those. It does not become key and does not take
/// touches.
@MainActor
enum DevServiceMarkWindow {
    private static var windows: [ObjectIdentifier: UIWindow] = [:]
    private static var watching = false

    static func attachIfNeeded() {
        guard lingxiaDevServiceBanner() != 0 else { return }
        watchScenes()
        for scene in UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }) {
            attach(to: scene)
        }
    }

    private static func watchScenes() {
        guard !watching else { return }
        watching = true
        let center = NotificationCenter.default
        center.addObserver(forName: UIScene.didActivateNotification, object: nil, queue: .main) { note in
            guard let scene = note.object as? UIWindowScene else { return }
            Task { @MainActor in attach(to: scene) }
        }
        center.addObserver(forName: UIScene.didDisconnectNotification, object: nil, queue: .main) { note in
            guard let scene = note.object as? UIWindowScene else { return }
            Task { @MainActor in
                windows.removeValue(forKey: ObjectIdentifier(scene))?.isHidden = true
            }
        }
    }

    private static func attach(to scene: UIWindowScene) {
        let id = ObjectIdentifier(scene)
        guard windows[id] == nil else { return }
        let mark = PassThroughWindow(windowScene: scene)
        mark.windowLevel = .alert + 2
        mark.backgroundColor = .clear
        mark.rootViewController = DevServiceMarkViewController()
        mark.isHidden = false
        windows[id] = mark
    }
}

private final class PassThroughWindow: UIWindow {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        nil
    }
}

private final class DevServiceMarkViewController: UIViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear
        view.isUserInteractionEnabled = false

        let chip = UIView()
        chip.translatesAutoresizingMaskIntoConstraints = false
        chip.backgroundColor = UIColor(red: 198.0 / 255.0, green: 40.0 / 255.0, blue: 40.0 / 255.0, alpha: 1)
        chip.layer.cornerRadius = 12
        chip.layer.maskedCorners = [.layerMaxXMinYCorner, .layerMaxXMaxYCorner]
        chip.isUserInteractionEnabled = false

        let dot = UIView()
        dot.translatesAutoresizingMaskIntoConstraints = false
        dot.backgroundColor = .white
        dot.layer.cornerRadius = 3
        dot.isUserInteractionEnabled = false

        let label = UILabel()
        label.translatesAutoresizingMaskIntoConstraints = false
        label.text = "DEV"
        label.font = .systemFont(ofSize: 11, weight: .medium)
        label.textColor = .white
        label.transform = CGAffineTransform(rotationAngle: -.pi / 2)
        label.isUserInteractionEnabled = false

        chip.addSubview(dot)
        chip.addSubview(label)
        view.addSubview(chip)

        NSLayoutConstraint.activate([
            chip.widthAnchor.constraint(equalToConstant: 24),
            chip.heightAnchor.constraint(equalToConstant: 60),
            dot.widthAnchor.constraint(equalToConstant: 6),
            dot.heightAnchor.constraint(equalToConstant: 6),
            dot.centerXAnchor.constraint(equalTo: chip.centerXAnchor),
            dot.topAnchor.constraint(equalTo: chip.topAnchor, constant: 8),
            label.widthAnchor.constraint(equalToConstant: 36),
            label.heightAnchor.constraint(equalToConstant: 16),
            label.centerXAnchor.constraint(equalTo: chip.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: chip.centerYAnchor, constant: 7),
            chip.leftAnchor.constraint(equalTo: view.leftAnchor),
            chip.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
    }
}
#endif

#if os(macOS)
import AppKit

@_silgen_name("lingxia_dev_service_banner")
private func lingxiaDevServiceBanner() -> Int32

/// Prod build on the dev service. One chip per top-level window, ordered
/// above that window's sheets and floats. It moves with the window and does
/// not take clicks, so it never sits over another app.
@MainActor
enum DevServiceMarkWindow {
    private static var marks: [ObjectIdentifier: NSPanel] = [:]
    private static var watching = false

    static func attachIfNeeded() {
        guard lingxiaDevServiceBanner() != 0 else { return }
        watch()
        guard let app = NSApp else { return }
        for window in app.windows {
            attach(to: window)
        }
    }

    /// A sheet or float was just stacked on `window`. Keep the chip above it.
    static func noteChildWindow(of window: NSWindow?) {
        guard let window else { return }
        attachIfNeeded()
        raise(on: root(of: window))
    }

    private static func watch() {
        guard !watching else { return }
        watching = true
        let center = NotificationCenter.default
        for name in [
            NSWindow.didBecomeKeyNotification,
            NSWindow.didResizeNotification,
            NSWindow.didMoveNotification,
            NSWindow.willCloseNotification,
        ] {
            center.addObserver(forName: name, object: nil, queue: .main) { note in
                guard let window = note.object as? NSWindow else { return }
                let closing = name == NSWindow.willCloseNotification
                Task { @MainActor in
                    if window.contentView as? ChipView != nil { return }
                    if closing {
                        detach(window)
                        return
                    }
                    let host = root(of: window)
                    attach(to: host)
                    raise(on: host)
                }
            }
        }
    }

    private static func attach(to window: NSWindow) {
        guard lingxiaDevServiceBanner() != 0 else { return }
        guard hostsMark(window) else { return }
        let id = ObjectIdentifier(window)
        if marks[id] != nil {
            raise(on: window)
            return
        }
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 64, height: 22),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isFloatingPanel = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = true
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = true
        panel.animationBehavior = .none
        panel.collectionBehavior = [.fullScreenAuxiliary, .ignoresCycle]
        panel.isMovable = false
        let chip = ChipView()
        panel.contentView = chip
        panel.setContentSize(chip.fittingSize)
        marks[id] = panel
        window.addChildWindow(panel, ordered: .above)
        place(panel, on: window)
        NSLog("LingXia.DevMark: dev service mark shown")
    }

    private static func raise(on window: NSWindow) {
        guard let panel = marks[ObjectIdentifier(window)] else { return }
        window.addChildWindow(panel, ordered: .above)
        place(panel, on: window)
    }

    private static func detach(_ window: NSWindow) {
        guard let panel = marks.removeValue(forKey: ObjectIdentifier(window)) else { return }
        window.removeChildWindow(panel)
        panel.orderOut(nil)
    }

    private static func place(_ panel: NSPanel, on window: NSWindow) {
        let size = panel.frame.size
        let frame = window.frame
        let origin = NSPoint(
            x: frame.maxX - size.width - 16,
            y: frame.minY + 16
        )
        if panel.frame.origin == origin { return }
        panel.setFrameOrigin(origin)
    }

    private static func hostsMark(_ window: NSWindow) -> Bool {
        guard window.parent == nil else { return false }
        guard window.isVisible else { return false }
        guard window.level == .normal else { return false }
        guard (window.contentView as? ChipView) == nil else { return false }
        guard window.frame.width >= 200, window.frame.height >= 200 else { return false }
        return window.styleMask.contains(.titled) || window.styleMask.contains(.resizable)
    }

    private static func root(of window: NSWindow) -> NSWindow {
        var current = window
        while let parent = current.parent {
            current = parent
        }
        return current
    }
}

private final class ChipView: NSView {
    private let label = NSTextField(labelWithString: "DEV")

    init() {
        super.init(frame: NSRect(x: 0, y: 0, width: 64, height: 22))
        wantsLayer = true
        layer?.backgroundColor = CGColor(red: 198.0 / 255.0, green: 40.0 / 255.0, blue: 40.0 / 255.0, alpha: 1)
        layer?.cornerRadius = 10

        let dot = NSView(frame: NSRect(x: 8, y: 8, width: 6, height: 6))
        dot.wantsLayer = true
        dot.layer?.backgroundColor = NSColor.white.cgColor
        dot.layer?.cornerRadius = 3
        addSubview(dot)

        label.font = .systemFont(ofSize: 11, weight: .medium)
        label.textColor = .white
        label.drawsBackground = false
        label.isBezeled = false
        label.isEditable = false
        label.isSelectable = false
        label.sizeToFit()
        label.frame.origin = NSPoint(x: 18, y: (22 - label.frame.height) / 2)
        addSubview(label)

        let width = 8 + 6 + 4 + label.frame.width + 8
        frame.size = NSSize(width: width, height: 22)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var fittingSize: NSSize { frame.size }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
#endif
