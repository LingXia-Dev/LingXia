#if os(macOS)
import AppKit
/// Protocol for providing a custom sidebar view (macOS).
///
/// Implement this to supply a fully custom sidebar via
/// `.swiftNative(handle)`. The SDK calls `makeSidebarView()` once
/// and mounts the returned view in the sidebar slot.
@MainActor
public protocol LxAppSidebarProviding: AnyObject {
    /// Create the sidebar view. Called once when the shell mounts.
    func makeSidebarView() -> NSView
}
#elseif os(iOS)
import UIKit
/// Protocol for providing a custom sidebar view controller (iOS).
@MainActor
public protocol LxAppSidebarProviding: AnyObject {
    /// Create the sidebar view controller. Called once when the shell mounts.
    func makeSidebarViewController() -> UIViewController
}
#endif
