#if os(macOS)
import Cocoa
import Foundation

/// macOS NavigationBar implementation
@MainActor
public class macOSNavigationBar: NSView {
    private static let HEIGHT: CGFloat = 32
    private var titleLabel: NSTextField!
    private var bottomBorder: NSView!

    public override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.white.cgColor

        bottomBorder = NSView()
        bottomBorder.wantsLayer = true
        bottomBorder.layer?.backgroundColor = NSColor.lightGray.withAlphaComponent(0.5).cgColor
        bottomBorder.translatesAutoresizingMaskIntoConstraints = false
        addSubview(bottomBorder)

        titleLabel = NSTextField(labelWithString: "")
        titleLabel.font = NSFont.systemFont(ofSize: 17, weight: .semibold)
        titleLabel.textColor = NSColor.black
        titleLabel.alignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(titleLabel)

        NSLayoutConstraint.activate([
            bottomBorder.leadingAnchor.constraint(equalTo: leadingAnchor),
            bottomBorder.trailingAnchor.constraint(equalTo: trailingAnchor),
            bottomBorder.bottomAnchor.constraint(equalTo: bottomAnchor),
            bottomBorder.heightAnchor.constraint(equalToConstant: 1),
            titleLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor)
        ])
    }

    // MARK: - NavigationBarProtocol Implementation

    public func updateWithConfig(
        pageConfig: NavigationBarConfig?,
        isBackNavigation: Bool,
        disableAnimation: Bool,
        onBackClickListener: @escaping () -> Void,
        onAnimationEnd: (() -> Void)?
    ) -> Bool {
        guard let config = pageConfig else {
            titleLabel.stringValue = ""
            titleLabel.textColor = NSColor.black
            layer?.backgroundColor = NSColor.white.cgColor
            onAnimationEnd?()
            return false
        }

        titleLabel.stringValue = config.title_text.toString()
        titleLabel.textColor = config.text_style.toString() == "white" ? NSColor.white : NSColor.black

        let backgroundColorString = config.background_color.toString().isEmpty ? NavigationBarConfig.DEFAULT_BACKGROUND_COLOR : config.background_color.toString()
        let backgroundColor = NSColor(hexString: backgroundColorString) ?? NSColor.white
        layer?.backgroundColor = backgroundColor.cgColor
        onAnimationEnd?()
        return true
    }

    public func setTitle(_ title: String?) {
        titleLabel.stringValue = title ?? ""
    }

    public func setBackButtonVisible(_ visible: Bool) {
        // macOS doesn't have back button in navigation bar
    }

    public func hide() {
        isHidden = true
    }

    public func getCalculatedContentHeight() -> CGFloat {
        return macOSNavigationBar.HEIGHT
    }

    // MARK: - macOS-specific methods

    public static func createForWindow(_ window: NSWindow) -> macOSNavigationBar? {
        guard let contentView = window.contentView else { return nil }

        let navigationBar = macOSNavigationBar(frame: NSRect(
            x: 0,
            y: contentView.frame.height - HEIGHT,
            width: contentView.frame.width,
            height: HEIGHT
        ))
        navigationBar.autoresizingMask = [.width, .minYMargin]
        return navigationBar
    }
}

/// macOS-specific NavigationBar support utilities
@MainActor
public class macOSNavigationBarSupport {

    /// Creates a NavigationBar for a specific window
    public static func createNavigationBarForWindow(_ window: NSWindow) -> macOSNavigationBar? {
        return macOSNavigationBar.createForWindow(window)
    }

    /// Gets the navigation bar height for macOS
    public static func getNavigationBarHeight() -> CGFloat {
        return 32
    }

    /// Updates window title bar appearance for custom navigation bar
    public static func configureWindowForCustomNavigationBar(_ window: NSWindow) {
        window.standardWindowButton(.closeButton)?.isHidden = true
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true
    }
}

#endif
