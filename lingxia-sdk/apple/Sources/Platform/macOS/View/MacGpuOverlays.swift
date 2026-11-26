import Foundation
#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Window overlays: a see-through GPU page in a borderless transparent child window over the
/// window holding the caller's presentation, above every LingXia surface in it. Rust calls these
/// on the main thread.
public enum GpuOverlays {
    @MainActor private static var hosts: [Int64: GpuOverlayHost] = [:]
    @MainActor private static let tracker = GpuPresentationTracker()

    /// Start telling Rust which windows hold which lxapps; once, after `lingxiaInit`.
    static func start() {
        MainActor.assumeIsolated { tracker.start() }
    }

    /// `window` came forward (`touched`) or on screen: Rust learns whose presentation it holds.
    /// False when the window holds no lxapp (yet).
    @MainActor @discardableResult
    static func presents(_ window: NSWindow, touched: Bool) -> Bool {
        if window is GpuOverlayWindow { return false }
        let number = UInt64(max(window.windowNumber, 0))
        if let appId = LxAppSurface.appId(of: window) {
            gpuOverlayPresents(appId, number, touched)
        } else if isShell(window) {
            gpuOverlayPresents("", number, touched)
        } else {
            return false
        }
        return true
    }

    /// The shell window, or with none (a custom host) the window showing the host's pages.
    @MainActor private static func isShell(_ window: NSWindow) -> Bool {
        if let shell = macOSLxApp.activeShell() { return shell.window === window }
        return window.canBecomeMain && !(window is NSPanel)
    }

    static func open(token: Int64, window: UInt64) -> Bool {
        MainActor.assumeIsolated {
            guard let parent = NSApp.window(withWindowNumber: Int(window)) else { return false }
            hosts[token] = GpuOverlayHost(token: token, parent: parent)
            return true
        }
    }

    static func input(token: Int64, block: Bool) {
        MainActor.assumeIsolated { hosts[token]?.block(block) }
    }

    static func focus(token: Int64, focused: Bool) {
        MainActor.assumeIsolated { hosts[token]?.focus(focused) }
    }

    static func close(token: Int64) {
        MainActor.assumeIsolated { hosts.removeValue(forKey: token)?.close() }
    }

    /// Draw the overlays over `contentView`'s window into a capture of that content view.
    @MainActor static func capture(over contentView: NSView, into context: CGContext) {
        for host in hosts.values where host.parent === contentView.window && host.window.isVisible {
            guard let image = host.view.lastFrame() else { continue }
            // The overlay covers the whole window; the capture starts at the content view.
            let origin = contentView.frame.origin
            let size = host.window.frame.size
            context.draw(image, in: CGRect(x: -origin.x, y: -origin.y, width: size.width, height: size.height))
        }
    }
}

/// Every window's focus, first showing and closing, for `GpuOverlays.presents`.
@MainActor
final class GpuPresentationTracker: NSObject {
    private var started = false

    func start() {
        guard !started else { return }
        started = true
        let center = NotificationCenter.default
        center.addObserver(self, selector: #selector(becameKey), name: NSWindow.didBecomeKeyNotification, object: nil)
        center.addObserver(self, selector: #selector(occlusion), name: NSWindow.didChangeOcclusionStateNotification, object: nil)
        center.addObserver(self, selector: #selector(closing), name: NSWindow.willCloseNotification, object: nil)
        center.addObserver(self, selector: #selector(updated), name: NSApplication.didUpdateNotification, object: nil)
    }

    private var seen = Set<Int>()

    /// A window first shown without focus or a visibility change (an app launched in the
    /// background, a window on another Space) still holds its lxapp.
    @objc private func updated(_ note: Notification) {
        for window in NSApp.windows where window.isVisible && !seen.contains(window.windowNumber) {
            if GpuOverlays.presents(window, touched: false) { seen.insert(window.windowNumber) }
        }
    }

    @objc private func becameKey(_ note: Notification) {
        guard let window = note.object as? NSWindow else { return }
        GpuOverlays.presents(window, touched: true)
    }

    /// A window shown without focus still holds its lxapp, covered or on another Space too.
    @objc private func occlusion(_ note: Notification) {
        guard let window = note.object as? NSWindow, window.isVisible else { return }
        GpuOverlays.presents(window, touched: false)
    }

    @objc private func closing(_ note: Notification) {
        guard let window = note.object as? NSWindow, !(window is GpuOverlayWindow) else { return }
        seen.remove(window.windowNumber)
        gpuOverlayWindowClosed(UInt64(max(window.windowNumber, 0)))
    }
}

@MainActor
final class GpuOverlayWindow: NSWindow {
    /// Key only while a dialog holds focus, so a toast never takes the keyboard.
    var focusable = false
    override var canBecomeKey: Bool { focusable }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class GpuOverlayHost: NSObject {
    let token: Int64
    weak var parent: NSWindow?
    let window: GpuOverlayWindow
    let view: GpuPageView
    private var appearance: NSKeyValueObservation?
    /// While blocking, clicks on the window controls go to the window below, as under a sheet.
    private var controls: Any?
    private var forwarding = false

    init(token: Int64, parent: NSWindow) {
        self.token = token
        self.parent = parent
        view = GpuPages.view(for: token)
        view.seeThrough = true
        window = GpuOverlayWindow(
            contentRect: Self.cover(parent), styleMask: [.borderless], backing: .buffered, defer: false)
        super.init()
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = false
        window.ignoresMouseEvents = true
        window.isReleasedWhenClosed = false
        window.animationBehavior = .none
        window.collectionBehavior = [.fullScreenAuxiliary, .ignoresCycle]
        window.contentView = view
        // The scrim follows the window's rounded corners (AppKit's standard 10 pt when the frame
        // view does not say).
        let radius = parent.contentView?.superview?.layer?.cornerRadius ?? 0
        view.layer?.cornerRadius = radius > 0 ? radius : 10
        view.layer?.masksToBounds = true
        // The lxapp's scheme reaches its window's appearance; the overlay follows it.
        window.appearance = parent.effectiveAppearance
        appearance = parent.observe(\.effectiveAppearance) { [weak self] _, _ in
            Task { @MainActor [weak self] in
                guard let self, let parent = self.parent else { return }
                self.window.appearance = parent.effectiveAppearance
            }
        }
        parent.addChildWindow(window, ordered: .above)
        let center = NotificationCenter.default
        for name in [NSWindow.didResizeNotification, NSWindow.didEndLiveResizeNotification,
                     NSWindow.didEnterFullScreenNotification, NSWindow.didExitFullScreenNotification] {
            center.addObserver(self, selector: #selector(fit), name: name, object: parent)
        }
    }

    /// The whole parent window: the two share window coordinates, so its controls can be
    /// handed clicks unchanged.
    private static func cover(_ parent: NSWindow) -> NSRect {
        parent.frame
    }

    @objc private func fit() {
        guard let parent else { return }
        window.setFrame(Self.cover(parent), display: false)
    }

    func block(_ block: Bool) {
        window.ignoresMouseEvents = !block
        if block, controls == nil {
            controls = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp]) {
                [weak self] event in
                // NSEvent isn't Sendable, so it can't be assumeIsolated's return value.
                var out: NSEvent? = event
                MainActor.assumeIsolated { out = self?.handOver(event) ?? event }
                return out
            }
        } else if !block, let controls {
            NSEvent.removeMonitor(controls)
            self.controls = nil
            forwarding = false
        }
    }

    /// A press that starts on the parent's traffic lights goes to the parent until it lifts.
    private func handOver(_ event: NSEvent) -> NSEvent? {
        guard event.window === window, let parent else { return event }
        if event.type == .leftMouseDown {
            forwarding = Self.windowControls(of: parent).contains(event.locationInWindow)
        }
        guard forwarding else { return event }
        if event.type == .leftMouseUp { forwarding = false }
        guard let moved = NSEvent.mouseEvent(
            with: event.type, location: event.locationInWindow, modifierFlags: event.modifierFlags,
            timestamp: event.timestamp, windowNumber: parent.windowNumber, context: nil,
            eventNumber: event.eventNumber, clickCount: event.clickCount, pressure: event.pressure)
        else { return event }
        parent.sendEvent(moved)
        return nil
    }

    private static func windowControls(of parent: NSWindow) -> NSRect {
        [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton]
            .compactMap { parent.standardWindowButton($0) }
            .filter { !$0.isHiddenOrHasHiddenAncestor }
            .reduce(NSRect.null) { $0.union($1.convert($1.bounds, to: nil)) }
    }

    func focus(_ focused: Bool) {
        window.focusable = focused
        if focused {
            // Only within the app: an inactive app's overlay takes focus when the app does.
            window.makeKey()
            window.makeFirstResponder(view)
        } else if window.isKeyWindow {
            parent?.makeKey()
        }
    }

    func close() {
        block(false)
        NotificationCenter.default.removeObserver(self)
        appearance = nil
        focus(false)
        parent?.removeChildWindow(window)
        window.orderOut(nil)
        window.contentView = nil
        GpuPages.dispose(token: token)
    }
}
#endif
