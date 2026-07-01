#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// One place for the "smart" enabled/dim styling of a browser navigation
/// affordance (back / forward). Every browser chrome — the self/main browser,
/// the docked/full-screen aside, the in-app browser — routes its back/forward
/// buttons through here so the highlight is identical everywhere: fully opaque
/// when the action is available, dimmed when it is not.
enum NavButtonState {
    /// Opacity of a back/forward affordance when its action is unavailable.
    static let disabledAlpha: CGFloat = 0.35

    #if os(macOS)
    static func apply(_ button: NSButton, enabled: Bool) {
        button.isEnabled = enabled
        button.alphaValue = enabled ? 1.0 : disabledAlpha
    }
    #elseif os(iOS)
    static func apply(_ button: UIButton, enabled: Bool) {
        button.isEnabled = enabled
        button.alpha = enabled ? 1.0 : disabledAlpha
    }
    #endif
}
