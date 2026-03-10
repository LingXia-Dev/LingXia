#if os(macOS)
import SwiftUI
import Foundation
import CLingXiaRustAPI

/// NSWindow class for LxApp Tab mode
public class LxAppWindow: NSWindow {
    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)
    }

    func configureForTabStyle() {
        // Tab-style with native window controls and custom tab bar
        styleMask.insert(.fullSizeContentView)
        titlebarAppearsTransparent = true
        titleVisibility = .hidden
        isMovableByWindowBackground = true
        backgroundColor = NSColor.windowBackgroundColor
        adjustTrafficLightPositions()
    }

    public override func setFrame(_ frameRect: NSRect, display flag: Bool) {
        super.setFrame(frameRect, display: flag)
        adjustTrafficLightPositions()
    }

    public override func setFrame(_ frameRect: NSRect, display flag: Bool, animate shouldAnimate: Bool) {
        super.setFrame(frameRect, display: flag, animate: shouldAnimate)
        adjustTrafficLightPositions()
    }

    private func adjustTrafficLightPositions() {
        guard !styleMask.contains(.fullScreen) else { return }
        let targetY: CGFloat = 13
        for type: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
            guard let button = standardWindowButton(type) else { continue }
            var f = button.frame
            guard abs(f.origin.y - targetY) > 0.5 else { continue }
            f.origin.y = targetY
            button.setFrameOrigin(f.origin)
        }
    }

    public override var canBecomeKey: Bool {
        return true
    }

    public override var canBecomeMain: Bool {
        return true
    }

    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let isDevtoolsShortcut = modifiers == [.command, .option]
            && (event.keyCode == 34 || event.charactersIgnoringModifiers?.lowercased() == "i")
        if isDevtoolsShortcut {
            if let controller = windowController as? LxAppWindowController,
               controller.toggleActiveDevTools() {
                return true
            }
        }

        // Backspace (keyCode 51) for back navigation
        if event.keyCode == 51 && event.modifierFlags.intersection(.deviceIndependentFlagsMask) == [] {
            // Don't intercept if typing in a native text field
            if let responder = firstResponder, responder is NSText {
                return super.performKeyEquivalent(with: event)
            }
            // Only navigate back when back button is available
            if let state = NavigationBarStateManager.shared.currentState, state.show_back_button {
                if let appId = LxAppTabManager.shared.activeTab?.appId {
                    let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionBack)
                    return true
                }
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}

#endif
