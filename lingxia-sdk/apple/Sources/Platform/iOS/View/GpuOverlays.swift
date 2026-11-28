import Foundation
#if os(iOS)
import CLingXiaRustAPI
import UIKit

/// Window overlays: a see-through GPU page in a view over the active scene's topmost LingXia
/// presentation (the lxapp, or a float surface's window), below the viewers that presentation
/// shows. Rust knows the scene as window 1. Rust calls these on the main thread.
public enum GpuOverlays {
    @MainActor private static var hosts: [Int64: GpuOverlayHost] = [:]
    @MainActor private static var observers: [NSObjectProtocol] = []
    static let window: UInt64 = 1

    /// Tell Rust the scene holds every lxapp, and follow windows coming and going; once.
    static func start() {
        MainActor.assumeIsolated {
            guard observers.isEmpty else { return }
            gpuOverlayPresents("", window, false)
            let center = NotificationCenter.default
            for name in [UIWindow.didBecomeVisibleNotification, UIWindow.didBecomeHiddenNotification] {
                observers.append(center.addObserver(forName: name, object: nil, queue: .main) { _ in
                    MainActor.assumeIsolated { restack() }
                })
            }
        }
    }

    /// A LingXia presentation came or went: every overlay moves over the topmost one.
    @MainActor static func restack() {
        hosts.values.forEach { $0.restack() }
    }

    static func open(token: Int64, window: UInt64) -> Bool {
        MainActor.assumeIsolated {
            guard window == Self.window, let host = GpuOverlayHost(token: token) else { return false }
            hosts[token] = host
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

    /// The view the overlay sits on: the topmost LingXia controller in the active scene's top
    /// window that holds one. System viewers presented from it stay above.
    @MainActor static func presentation() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        guard let scene = scenes.first(where: { $0.activationState == .foregroundActive }) ?? scenes.first else {
            return nil
        }
        // Later windows of a level are above earlier ones; system windows (the keyboard's) are
        // skipped, and so are windows that take no touches, where a dialog could not be pressed.
        let windows = scene.windows.enumerated()
            .filter {
                !$0.element.isHidden && $0.element.isUserInteractionEnabled
                    && $0.element.rootViewController != nil && isAppWindow($0.element)
            }
            .sorted { ($0.element.windowLevel.rawValue, $0.offset) < ($1.element.windowLevel.rawValue, $1.offset) }
            .map(\.element)
        guard var top = windows.last?.rootViewController else { return nil }
        while let presented = top.presentedViewController, !presented.isBeingDismissed, isLingXia(presented) {
            top = presented
        }
        return top
    }

    private static func isAppWindow(_ window: UIWindow) -> Bool {
        type(of: window) == UIWindow.self || !NSStringFromClass(type(of: window)).hasPrefix("UI")
    }

    /// An lxapp presented over the host; any other controller (a picker, a viewer) stays above.
    private static func isLingXia(_ controller: UIViewController) -> Bool {
        let shown = (controller as? UINavigationController)?.viewControllers.first ?? controller
        return shown is LxAppViewController
    }
}

/// Holds the overlay's page: touches pass through unless it blocks.
@MainActor
final class GpuOverlayView: UIView {
    var blocking = false
    weak var host: GpuOverlayHost?

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard blocking else { return nil }
        return super.hitTest(point, with: event) ?? self
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        // The presentation it sat on went away: it moves to the one now on top.
        if window == nil, let host {
            DispatchQueue.main.async { [weak host] in host?.restack() }
        }
    }
}

@MainActor
final class GpuOverlayHost: NSObject {
    let token: Int64
    let page: GpuPageView
    let view = GpuOverlayView()
    /// The responder a dialog took focus from, given it back when the dialog goes.
    private weak var previous: UIResponder?
    private var closed = false

    init?(token: Int64) {
        guard GpuOverlays.presentation() != nil else { return nil }
        self.token = token
        page = GpuPages.view(for: token)
        page.seeThrough = true
        super.init()
        view.host = self
        view.backgroundColor = .clear
        view.isOpaque = false
        page.frame = view.bounds
        page.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(page)
        restack()
    }

    func restack() {
        guard !closed, let parent = GpuOverlays.presentation()?.view else { return }
        if view.superview !== parent {
            view.removeFromSuperview()
            view.frame = parent.bounds
            view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            parent.addSubview(view)
        } else {
            parent.bringSubviewToFront(view)
        }
    }

    func block(_ block: Bool) {
        view.blocking = block
        // A dialog hides what is under it from VoiceOver, as any modal does.
        view.accessibilityViewIsModal = block
        restack()
        UIAccessibility.post(notification: .screenChanged, argument: nil)
    }

    func focus(_ focused: Bool) {
        if focused {
            previous = UIResponder.lxCurrentFirstResponder()
            // Taking focus hides the soft keyboard; a hardware keyboard's Escape reaches the page.
            previous?.resignFirstResponder()
            _ = page.becomeFirstResponder()
        } else {
            if page.isFirstResponder { _ = page.resignFirstResponder() }
            previous?.becomeFirstResponder()
            previous = nil
        }
    }

    func close() {
        closed = true
        if page.isFirstResponder { focus(false) }
        view.host = nil
        view.removeFromSuperview()
        GpuPages.dispose(token: token)
    }
}

extension UIResponder {
    private static weak var lxFound: UIResponder?

    /// The window's first responder: the one that answers an action sent to nobody.
    @MainActor static func lxCurrentFirstResponder() -> UIResponder? {
        lxFound = nil
        UIApplication.shared.sendAction(#selector(lxReportFirstResponder), to: nil, from: nil, for: nil)
        return lxFound
    }

    @objc private func lxReportFirstResponder() {
        UIResponder.lxFound = self
    }
}
#endif
