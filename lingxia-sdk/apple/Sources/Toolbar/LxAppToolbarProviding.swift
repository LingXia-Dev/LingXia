#if os(macOS)
import AppKit
/// Protocol for providing a custom toolbar view (macOS).
@MainActor
public protocol LxAppToolbarProviding: AnyObject {
    /// Create the toolbar view. Called once when the shell mounts.
    func makeToolbarView() -> NSView
}
#elseif os(iOS)
import UIKit
/// Protocol for providing a custom toolbar view (iOS).
@MainActor
public protocol LxAppToolbarProviding: AnyObject {
    func makeToolbarView() -> UIView
}
#endif
