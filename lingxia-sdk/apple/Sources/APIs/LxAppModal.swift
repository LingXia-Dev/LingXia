import Foundation
import os.log
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// Modal dialog management for LingXia applications
public class LxAppModal {

    /// Shared logger for modal operations
    private static let log = OSLog(subsystem: "LingXia", category: "Modal")

    /// Show modal with ModalOptions (FFI interface)
    public static func showModal(options: ModalOptions, callback_id: UInt64) {
        showModal([
            "title": options.title.toString(),
            "content": options.content.toString(),
            "showCancel": options.show_cancel,
            "cancelText": options.cancel_text.toString(),
            "confirmText": options.confirm_text.toString()
        ], callback_id: callback_id)
    }

    /// Show modal with callback (main interface)
    public static func showModal(_ options: [String: Any], callback_id: UInt64) {
        // Extract options
        let title = options["title"] as? String ?? ""
        let content = options["content"] as? String ?? ""
        let showCancel = options["showCancel"] as? Bool ?? true
        let cancelText = options["cancelText"] as? String ?? ""
        let confirmText = options["confirmText"] as? String ?? ""

        #if os(macOS)
        // Show macOS modal asynchronously with callback
        DispatchQueue.main.async {
            showMacOSModal(
                title: title,
                content: content,
                showCancel: showCancel,
                cancelText: cancelText,
                confirmText: confirmText,
                callback_id: callback_id
            )
        }

        #else
        // Show iOS modal asynchronously with callback
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
        #endif
    }

    #if os(macOS)
    /// Show macOS modal using NSAlert
    @MainActor
    private static func showMacOSModal(
    title: String,
    content: String,
    showCancel: Bool,
    cancelText: String,
    confirmText: String,
    callback_id: UInt64
) {
    let alert = NSAlert()
    if !title.isEmpty && title.trimmingCharacters(in: .whitespacesAndNewlines) != "" {
        alert.messageText = title
    } else {
        // When title is empty, set messageText to content and clear informativeText
        alert.messageText = content
        alert.informativeText = ""
    }

    // Only set informativeText if we have a title
    if !title.isEmpty && title.trimmingCharacters(in: .whitespacesAndNewlines) != "" {
        alert.informativeText = content
    }
    alert.addButton(withTitle: confirmText)

    if showCancel {
        alert.addButton(withTitle: cancelText)
    }

    let response = alert.runModal()
    let confirmed = response == .alertFirstButtonReturn

    // Call callback with result
    let result: [String: Any] = [
        "confirm": confirmed,
        "cancel": !confirmed
    ]

    if let jsonData = try? JSONSerialization.data(withJSONObject: result),
       let jsonString = String(data: jsonData, encoding: .utf8) {
        _ = onCallback(callback_id, true, jsonString)
    }
    }
    #endif

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
    guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
          let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
          let rootViewController = window.rootViewController else {
        os_log("Could not find root view controller", log: LxAppModal.log, type: .error)
        return
    }

    // Find the topmost view controller
    var topViewController = rootViewController
    while let presentedViewController = topViewController.presentedViewController {
        topViewController = presentedViewController
    }

    let alertTitle = title.isEmpty ? nil : title
    let alert = UIAlertController(title: alertTitle, message: content, preferredStyle: .alert)

    // Add confirm action
    let confirmAction = UIAlertAction(title: confirmText, style: .default) { _ in
        // Call callback with confirm result
        let result: [String: Any] = [
            "confirm": true,
            "cancel": false
        ]

        if let jsonData = try? JSONSerialization.data(withJSONObject: result),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            _ = onCallback(callback_id, true, jsonString)
        }
    }
    alert.addAction(confirmAction)

    // Add cancel action if needed
    if showCancel {
        let cancelAction = UIAlertAction(title: cancelText, style: .cancel) { _ in
            // User cancelled = error 2000
            _ = onCallback(callback_id, false, "2000")
        }
        alert.addAction(cancelAction)
    }

    // Present the alert
    topViewController.present(alert, animated: true)
}
#endif

}
