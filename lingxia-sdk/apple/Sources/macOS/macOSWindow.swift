#if os(macOS)
import Cocoa
import Foundation

/// Custom NSWindow class for LxApp with style configuration
class macOSLxAppWindow: NSWindow {
    private var windowStyle: LxAppWindowStyle = .systemDefault

    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)
    }

    func configureForStyle(_ style: LxAppWindowStyle) {
        self.windowStyle = style
        macOSWindowSupport.configureWindow(self, style: style)
    }

    override var canBecomeKey: Bool {
        return true
    }

    override var canBecomeMain: Bool {
        return true
    }
}

/// macOS-specific window management utilities
@MainActor
public class macOSWindowSupport {

    /// Configures window for the specified style
    public static func configureWindow(_ window: NSWindow, style: LxAppWindowStyle) {
        switch style {
        case .systemDefault:
            window.styleMask.remove(.fullSizeContentView)
            window.titlebarAppearsTransparent = false
            window.titleVisibility = .visible
            window.isMovableByWindowBackground = false
        case .customCapsule, .borderless:
            // Both styles use full-size content view with transparent title bar
            window.styleMask.insert(.fullSizeContentView)
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
            window.isMovableByWindowBackground = true
        }
    }

    /// Gets the top margin for content based on window style
    public static func getTopMarginForStyle(_ style: LxAppWindowStyle) -> CGFloat {
        switch style {
        case .customCapsule:
            return 32  // Custom capsule style needs space for title bar
        case .systemDefault:
            return 0   // System default style uses system title bar
        case .borderless:
            return 0   // Content fills entire window, system buttons float on top
        }
    }

    /// Creates capsule buttons for custom window style
    public static func createCapsuleButtons(for titleBarView: NSView, windowWidth: CGFloat, target: AnyObject, actions: (more: Selector, minimize: Selector, close: Selector)) {
        // Use the dedicated capsule button functionality
        // This is a simple delegation to keep window support focused on window management
        let buttonWidth: CGFloat = 87 / 3
        let buttonHeight: CGFloat = 28
        let buttonY: CGFloat = 2
        let rightMargin: CGFloat = 7

        // Create buttons
        let moreButton = createButton(target: target, action: actions.more)
        let minimizeButton = createButton(target: target, action: actions.minimize)
        let closeButton = createButton(target: target, action: actions.close)

        // Position buttons
        let startX = windowWidth - 87 - rightMargin
        moreButton.frame = NSRect(x: startX, y: buttonY, width: buttonWidth, height: buttonHeight)
        minimizeButton.frame = NSRect(x: startX + buttonWidth, y: buttonY, width: buttonWidth, height: buttonHeight)
        closeButton.frame = NSRect(x: startX + buttonWidth * 2, y: buttonY, width: buttonWidth, height: buttonHeight)

        // Add to view
        titleBarView.addSubview(moreButton)
        titleBarView.addSubview(minimizeButton)
        titleBarView.addSubview(closeButton)
    }

    private static func createButton(target: AnyObject, action: Selector) -> NSButton {
        let button = NSButton()
        button.target = target
        button.action = action
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.translatesAutoresizingMaskIntoConstraints = true
        return button
    }
}

#endif
