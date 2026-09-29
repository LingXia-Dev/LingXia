import AppKit

/// Where the Runner window stood when it last closed.
///
/// The Runner closes its window and opens a new one whenever an app is opened
/// again (a profile switch in a test run, a host-shape switch). A new window
/// starts on the screen centre, so a window the user had moved kept jumping
/// back. The top-left is what is kept: a device of another height then grows
/// downward instead of shifting under the menu bar.
@MainActor
enum RunnerWindowPlacement {
    private static var lastTopLeft: NSPoint?

    /// Remember where `window` is. Call it as the window closes.
    static func remember(_ window: NSWindow) {
        lastTopLeft = NSPoint(x: window.frame.minX, y: window.frame.maxY)
    }

    /// Put a new window where the last one closed, if some of it would still
    /// be on a screen (a display may have gone since). Returns whether it did;
    /// otherwise the window stays where the caller placed it.
    @discardableResult
    static func restore(_ window: NSWindow) -> Bool {
        guard let topLeft = lastTopLeft else { return false }
        let frame = NSRect(
            x: topLeft.x,
            y: topLeft.y - window.frame.height,
            width: window.frame.width,
            height: window.frame.height
        )
        guard NSScreen.screens.contains(where: { $0.visibleFrame.intersects(frame) }) else {
            return false
        }
        window.setFrameTopLeftPoint(topLeft)
        return true
    }
}
