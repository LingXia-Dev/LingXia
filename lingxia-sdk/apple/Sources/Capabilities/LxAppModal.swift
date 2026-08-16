import Foundation
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// Modal dialog management for LingXia applications
class LxAppModal {

    /// A dismissal is business code 2000; everything that merely *failed* —
    /// no presenter, serialization — reports the generic failure code, so
    /// `canceled: true` on the JS side can only ever mean the user said no.
    private static let modalFailureCode = LxAppDismissal.failureCode


    private final class CallbackOnce {
        private let callbackId: UInt64
        private let lock = NSLock()
        private var completed = false

        init(callbackId: UInt64) {
            self.callbackId = callbackId
        }

        func send(success: Bool, payload: String) {
            lock.lock()
            guard !completed else {
                lock.unlock()
                return
            }
            completed = true
            lock.unlock()
            _ = onCallback(callbackId, success, payload)
        }
    }

    /// Show modal with ModalOptions (FFI interface)
    static func showModal(options: ModalOptions, callback_id: UInt64) {
        showModal([
            "title": options.title.toString(),
            "content": options.content.toString(),
            "showCancel": options.show_cancel,
            "cancelText": options.cancel_text.toString(),
            "confirmText": options.confirm_text.toString()
        ], callback_id: callback_id)
    }

    /// Show modal with callback (main interface)
    static func showModal(_ options: [String: Any], callback_id: UInt64) {
        // Extract options
        let title = options["title"] as? String ?? ""
        let content = options["content"] as? String ?? ""
        let showCancel = options["showCancel"] as? Bool ?? true
        let cancelText = options["cancelText"] as? String ?? ""
        let confirmText = options["confirmText"] as? String ?? ""

        #if os(iOS)
        DispatchQueue.main.async {
            showIOSModal(
                title: title,
                content: content,
                showCancel: showCancel,
                cancelText: cancelText,
                confirmText: confirmText,
                callback_id: callback_id
            )
        }
        #elseif os(macOS)
        DispatchQueue.main.async {
            showMacModal(
                title: title,
                content: content,
                showCancel: showCancel,
                cancelText: cancelText,
                confirmText: confirmText,
                callback_id: callback_id
            )
        }
        #endif
    }

    #if os(iOS)
    /// Show iOS modal using UIAlertController
    @MainActor
    private static func showIOSModal(
    title: String,
    content: String,
    showCancel: Bool,
    cancelText: String,
    confirmText: String,
    callback_id: UInt64
) {
    let callback = CallbackOnce(callbackId: callback_id)
    guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
          let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
          let rootViewController = window.rootViewController else {
        LXLog.error("Could not find root view controller", category: "Modal")
        callback.send(success: false, payload: modalFailureCode)
        return
    }

    // Find the topmost view controller
    var topViewController = rootViewController
    while let presentedViewController = topViewController.presentedViewController {
        topViewController = presentedViewController
    }
    guard topViewController.view.window != nil,
          !topViewController.isBeingPresented,
          !topViewController.isBeingDismissed,
          topViewController.transitionCoordinator == nil else {
        LXLog.error("Could not find a stable modal presenter", category: "Modal")
        callback.send(success: false, payload: modalFailureCode)
        return
    }

    let alertTitle = title.isEmpty ? nil : title
    let alert = UIAlertController(title: alertTitle, message: content, preferredStyle: .alert)
    // A system alert follows the OS; the lxapp behind it may be pinned to the
    // opposite scheme.
    alert.overrideUserInterfaceStyle = LxAppAppearanceRegistry.overlayIsDark() ? .dark : .light

    // Add confirm action
    let confirmAction = UIAlertAction(title: confirmText, style: .default) { _ in
        // Call callback with confirm result
        let result: [String: Any] = [
            "confirm": true,
            "cancel": false
        ]

        guard let jsonData = try? JSONSerialization.data(withJSONObject: result),
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            callback.send(success: false, payload: modalFailureCode)
            return
        }
        callback.send(success: true, payload: jsonString)
    }
    alert.addAction(confirmAction)

    // Add cancel action if needed
    if showCancel {
        let cancelAction = UIAlertAction(title: cancelText, style: .cancel) { _ in
            // User cancelled = error 2000
            callback.send(success: false, payload: LxAppDismissal.userDismissedCode)
        }
        alert.addAction(cancelAction)
    }

    // Present the alert
    topViewController.present(alert, animated: true)
    DispatchQueue.main.async {
        guard alert.presentingViewController != nil || alert.viewIfLoaded?.window != nil else {
            LXLog.error("UIKit did not present modal alert", category: "Modal")
            callback.send(success: false, payload: modalFailureCode)
            return
        }
    }
}
#endif

    #if os(macOS)
    @MainActor
    private static func showMacModal(
        title: String,
        content: String,
        showCancel: Bool,
        cancelText: String,
        confirmText: String,
        callback_id: UInt64
    ) {
        let alert = NSAlert()
        alert.window.appearance =
            NSAppearance(named: LxAppAppearanceRegistry.overlayIsDark() ? .darkAqua : .aqua)
        alert.messageText = title
        alert.informativeText = content
        alert.addButton(withTitle: confirmText)
        if showCancel {
            alert.addButton(withTitle: cancelText)
        }

        guard alert.runModal() == .alertFirstButtonReturn else {
            _ = onCallback(callback_id, false, LxAppDismissal.userDismissedCode)
            return
        }

        let result: [String: Any] = ["confirm": true, "cancel": false]
        guard let jsonData = try? JSONSerialization.data(withJSONObject: result),
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            _ = onCallback(callback_id, false, modalFailureCode)
            return
        }
        _ = onCallback(callback_id, true, jsonString)
    }
    #endif

}
