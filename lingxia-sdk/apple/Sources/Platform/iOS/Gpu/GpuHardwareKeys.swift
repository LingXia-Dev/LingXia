import Foundation
#if os(iOS)
import UIKit
import GameController
import CLingXiaRustAPI

/// A hardware keyboard (iPad, a paired keyboard) walks focus and activates controls
/// through the core while no editor has focus.
extension GpuPageView {
    /// True while a hardware keyboard is attached: the page then takes key presses.
    static var hardwareKeyboard: Bool { GCKeyboard.coalesced != nil }

    /// The core's key for a hardware key, as `gpuPageKey` numbers it; a printable character
    /// travels as 0x1_0000 plus its code point (time digits, a select's type-ahead).
    private static func key(_ code: UIKeyboardHIDUsage, _ characters: String) -> Int64? {
        switch code {
        case .keyboardTab: 0
        case .keyboardUpArrow: 1
        case .keyboardDownArrow: 2
        case .keyboardLeftArrow: 3
        case .keyboardRightArrow: 4
        case .keyboardHome: 5
        case .keyboardEnd: 6
        case .keyboardReturnOrEnter, .keypadEnter, .keyboardSpacebar: 7
        case .keyboardEscape: 8
        case .keyboardPageUp: 9
        case .keyboardPageDown: 10
        default: characters.unicodeScalars.first.map { 0x1_0000 + Int64($0.value) }
        }
    }

    /// Unhandled presses go on to UIKit (the editor, the responder chain).
    func pressesToPage(_ presses: Set<UIPress>) -> Set<UIPress> {
        guard !gpuPageInputBlocked(token) else { return presses }
        var rest = presses
        for press in presses {
            guard let pressed = press.key, let key = Self.key(pressed.keyCode, pressed.charactersIgnoringModifiers)
            else { continue }
            let plain = pressed.modifierFlags.isDisjoint(with: [.command, .control, .alternate])
            // An editor keeps its keys; Tab leaves it for focus traversal.
            if ime.editor != nil {
                guard key == 0 else { continue }
                ime.clear(blur: true)
            }
            if plain, gpuPageKey(token, key, pressed.modifierFlags.contains(.shift), false) {
                rest.remove(press)
            }
        }
        return rest
    }
}
#endif
