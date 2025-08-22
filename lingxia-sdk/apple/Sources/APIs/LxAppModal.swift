import Foundation
import os.log
import CLingXiaFFI

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
    public static func showModal(options: ModalOptions) -> ModalResult {
        return showModal([
            "title": options.title.toString(),
            "content": options.content.toString(),
            "showCancel": options.show_cancel,
            "cancelText": options.cancel_text.toString(),
            "confirmText": options.confirm_text.toString(),
            "editable": options.editable,
            "placeholderText": options.placeholder_text.toString()
        ])
    }

    /// Show modal synchronously (main interface)
    public static func showModal(_ options: [String: Any]) -> ModalResult {
        // Extract options
        let title = options["title"] as? String ?? "Alert"
        let content = options["content"] as? String ?? ""
        let showCancel = options["showCancel"] as? Bool ?? true
        let cancelText = options["cancelText"] as? String ?? "Cancel"
        let confirmText = options["confirmText"] as? String ?? "OK"
        let editable = options["editable"] as? Bool ?? false
        let placeholderText = options["placeholderText"] as? String ?? ""

        #if os(macOS)
        // Show macOS modal asynchronously and return immediate result
        DispatchQueue.main.async {
            showMacOSModal(
                title: title,
                content: content,
                showCancel: showCancel,
                cancelText: cancelText,
                confirmText: confirmText,
                editable: editable,
                placeholderText: placeholderText
            )
        }

        // Return immediate result for FFI compatibility
        return ModalResult(
            confirm: true,
            cancel: false,
            content: RustString(editable ? "input" : "")
        )

        #else
        // Show iOS modal asynchronously and return immediate result
        DispatchQueue.main.async {
            showIOSModal(
                title: title,
                content: content,
                showCancel: showCancel,
                cancelText: cancelText,
                confirmText: confirmText,
                editable: editable,
                placeholderText: placeholderText
            )
        }

        // Return immediate result for FFI compatibility
        return ModalResult(
            confirm: true,
            cancel: false,
            content: RustString(editable ? "input" : "")
        )
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
    editable: Bool,
    placeholderText: String
) {
    let alert = NSAlert()
    alert.messageText = title
    alert.informativeText = content
    alert.addButton(withTitle: confirmText)

    if showCancel {
        alert.addButton(withTitle: cancelText)
    }

    var inputField: NSTextField?
    if editable {
        inputField = NSTextField(frame: NSRect(x: 0, y: 0, width: 300, height: 24))
        inputField?.placeholderString = placeholderText
        alert.accessoryView = inputField
    }

    let response = alert.runModal()
    let confirmed = response == .alertFirstButtonReturn
    let inputText = inputField?.stringValue ?? ""

    os_log("macOS modal result: confirm=%{public}@, content='%{public}@'", log: LxAppModal.log, type: .info, String(confirmed), inputText)
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
    editable: Bool,
    placeholderText: String
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

    let alert = UIAlertController(title: title, message: content, preferredStyle: .alert)

    var inputTextField: UITextField?
    if editable {
        alert.addTextField { textField in
            textField.placeholder = placeholderText
            inputTextField = textField
        }
    }

    // Add confirm action
    let confirmAction = UIAlertAction(title: confirmText, style: .default) { _ in
        let inputText = inputTextField?.text ?? ""
        os_log("iOS modal confirmed: content='%{public}@'", log: LxAppModal.log, type: .info, inputText)
    }
    alert.addAction(confirmAction)

    // Add cancel action if needed
    if showCancel {
        let cancelAction = UIAlertAction(title: cancelText, style: .cancel) { _ in
            os_log("iOS modal cancelled", log: LxAppModal.log, type: .info)
        }
        alert.addAction(cancelAction)
    }

    // Present the alert
    topViewController.present(alert, animated: true)
}
#endif

}
