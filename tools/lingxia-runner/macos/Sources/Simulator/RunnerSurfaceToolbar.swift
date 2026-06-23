import AppKit

/// The pad/desktop runner's top toolbar, mounted above the surface-shell content
/// (via `LxAppShell.setTopAccessory`) so it reads like the iPhone simulator's
/// toolbar: a dark strip with the window dots on the left and the device selector
/// centered. The shell window stays frameless (no real traffic lights); these
/// dots drive close/minimize instead.
@MainActor
final class RunnerSurfaceToolbar: NSView {
    static let height: CGFloat = 36

    let deviceSelector: RunnerDeviceSelectorControl
    var onClose: (() -> Void)?
    var onMinimize: (() -> Void)?

    init(selector: RunnerDeviceSelectorControl) {
        self.deviceSelector = selector
        super.init(frame: NSRect(x: 0, y: 0, width: 480, height: Self.height))
        setup()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setup() {
        wantsLayer = true
        layer?.backgroundColor = NSColor(white: 0.18, alpha: 1.0).cgColor

        let close = makeDot(color: NSColor(red: 1.0, green: 0.38, blue: 0.35, alpha: 1.0))
        close.action = #selector(closeClicked)
        close.toolTip = "Close"
        let minimize = makeDot(color: NSColor(red: 1.0, green: 0.74, blue: 0.22, alpha: 1.0))
        minimize.action = #selector(minimizeClicked)
        minimize.toolTip = "Minimize"

        addSubview(close)
        addSubview(minimize)
        addSubview(deviceSelector)

        NSLayoutConstraint.activate([
            close.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            close.centerYAnchor.constraint(equalTo: centerYAnchor),
            close.widthAnchor.constraint(equalToConstant: 12),
            close.heightAnchor.constraint(equalToConstant: 12),

            minimize.leadingAnchor.constraint(equalTo: close.trailingAnchor, constant: 8),
            minimize.centerYAnchor.constraint(equalTo: centerYAnchor),
            minimize.widthAnchor.constraint(equalToConstant: 12),
            minimize.heightAnchor.constraint(equalToConstant: 12),

            deviceSelector.centerXAnchor.constraint(equalTo: centerXAnchor),
            deviceSelector.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    private func makeDot(color: NSColor) -> NSButton {
        let button = NSButton()
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.title = ""
        button.wantsLayer = true
        button.layer?.backgroundColor = color.cgColor
        button.layer?.cornerRadius = 6
        button.target = self
        return button
    }

    @objc private func closeClicked() { onClose?() }
    @objc private func minimizeClicked() { onMinimize?() }
}
