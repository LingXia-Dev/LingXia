#if os(iOS)
import ObjectiveC
import SwiftUI
import UIKit

/// A host chrome element SwiftUI draws without a UIKit view (a bar button, a tab, the capsule, a
/// toast), made readable to automation: a view behind it in the hierarchy that carries what it
/// says and what a tap on it runs. It takes no touches and is not an accessibility element.
@objc(LxChromeProbe)
final class ChromeProbeView: UIView {
    var label = ""
    var role = "button"
    var selected = false
    var action: (() -> Void)?

    @objc var chromeLabel: NSString { label as NSString }
    @objc var chromeRole: NSString { role as NSString }
    @objc var chromeSelected: Bool { selected }

    /// Runs the element's tap; false when it has none.
    @objc func press() -> Bool {
        guard let action else { return false }
        action()
        return true
    }
}

private struct ChromeProbe: UIViewRepresentable {
    let label: String
    let role: String
    let selected: Bool
    let press: (() -> Void)?

    func makeUIView(context: Context) -> ChromeProbeView {
        let view = ChromeProbeView()
        view.isUserInteractionEnabled = false
        view.isAccessibilityElement = false
        view.backgroundColor = .clear
        return view
    }

    func updateUIView(_ view: ChromeProbeView, context: Context) {
        view.label = label
        view.role = role
        view.selected = selected
        view.action = press
    }
}

extension View {
    /// Make this host chrome element readable to automation; `press` is what a tap runs.
    func hostChrome(_ label: String, role: String = "button", selected: Bool = false,
                    press: (() -> Void)? = nil) -> some View {
        background(ChromeProbe(label: label, role: role, selected: selected, press: press))
    }
}

/// System alerts as automation presses them: UIKit takes no touches from code, so an action
/// made here keeps its handler, and a press dismisses the alert and runs it as a tap would.
@MainActor
@objc(LxHostChrome)
final class HostChrome: NSObject {
    private final class Handler {
        let run: () -> Void
        init(_ run: @escaping () -> Void) { self.run = run }
    }
    private static var handlerKey: UInt8 = 0

    /// A `UIAlertAction` whose handler automation can run.
    static func alertAction(title: String, style: UIAlertAction.Style,
                            handler: (() -> Void)? = nil) -> UIAlertAction {
        let action = UIAlertAction(title: title, style: style) { _ in handler?() }
        if let handler {
            objc_setAssociatedObject(action, &handlerKey, Handler(handler), .OBJC_ASSOCIATION_RETAIN)
        }
        return action
    }

    /// Press the action titled `title` of the alert on screen; false when there is none.
    @objc static func pressAlertAction(_ title: NSString) -> Bool {
        for scene in UIApplication.shared.connectedScenes {
            guard let scene = scene as? UIWindowScene else { continue }
            for window in scene.windows {
                var top = window.rootViewController
                while let presented = top?.presentedViewController {
                    top = presented
                }
                guard let alert = top as? UIAlertController,
                      let action = alert.actions.first(where: { $0.title == title as String }) else {
                    continue
                }
                let handler = objc_getAssociatedObject(action, &handlerKey) as? Handler
                // As a tap: the handler runs once the alert is gone, so a dialog it shows next
                // is not presented over one still leaving.
                alert.dismiss(animated: true) { handler?.run() }
                return true
            }
        }
        return false
    }
}
#else
import SwiftUI

extension View {
    /// macOS reads its chrome through accessibility.
    func hostChrome(_ label: String, role: String = "button", selected: Bool = false,
                    press: (() -> Void)? = nil) -> some View { self }
}
#endif
