import Foundation
import os.log
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

public class LxAppActionSheet {

    private static let log = OSLog(subsystem: "LingXia", category: "ActionSheet")

    public static func showActionSheet(options: ActionSheetOptions, callback_id: UInt64) {
        var optionsArray: [String] = []
        for i in 0..<options.options.len() {
            if let item = options.options.get(index: UInt(i)) {
                optionsArray.append(item.as_str().toString())
            }
        }
        showActionSheet([
            "options": optionsArray,
            "cancelText": options.cancel_text.toString(),
            "itemColor": options.item_color.toString()
        ], callback_id: callback_id)
    }

    public static func showActionSheet(_ options: [String: Any], callback_id: UInt64) {
        let optionsArray = options["options"] as? [String] ?? []
        let cancelText = options["cancelText"] as? String ?? ""
        let itemColor = options["itemColor"] as? String ?? "#007AFF"

        DispatchQueue.main.async {
            #if os(macOS)
            showMacOSActionSheet(options: optionsArray, cancelText: cancelText, itemColor: itemColor, callback_id: callback_id)
            #else
            showIOSActionSheet(options: optionsArray, cancelText: cancelText, itemColor: itemColor, callback_id: callback_id)
            #endif
        }
    }

    internal static func sendResult(callback_id: UInt64, tapIndex: Int) {
        let result = ["tapIndex": tapIndex]
        if let jsonData = try? JSONSerialization.data(withJSONObject: result),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            _ = onCallback(callback_id, true, jsonString)
        }
    }

    #if os(macOS)
    @MainActor
    private static func showMacOSActionSheet(options: [String], cancelText: String, itemColor: String, callback_id: UInt64) {
        let alert = NSAlert()
        alert.messageText = ""

        for option in options {
            alert.addButton(withTitle: option)
        }
        alert.addButton(withTitle: cancelText)

        let response = alert.runModal()
        let buttonIndex = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        sendResult(callback_id: callback_id, tapIndex: buttonIndex < options.count ? buttonIndex : -1)
    }
    #endif

    #if os(iOS)
    @MainActor
    private static func showIOSActionSheet(options: [String], cancelText: String, itemColor: String, callback_id: UInt64) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
              let rootViewController = window.rootViewController else {
            os_log("Could not find root view controller", log: log, type: .error)
            return
        }

        var topViewController = rootViewController
        while let presentedViewController = topViewController.presentedViewController {
            topViewController = presentedViewController
        }

        let actionSheetView = createCustomActionSheet(options: options, cancelText: cancelText, itemColor: itemColor, callback_id: callback_id)
        presentCustomActionSheet(actionSheetView, on: topViewController)
    }

    @MainActor
    private static func createCustomActionSheet(options: [String], cancelText: String, itemColor: String, callback_id: UInt64) -> UIView {
        let backgroundView = UIView(frame: UIScreen.main.bounds)
        backgroundView.backgroundColor = UIColor.black.withAlphaComponent(0.4)
        backgroundView.alpha = 0

        let containerView = UIView()
        containerView.backgroundColor = UIColor.white
        containerView.layer.cornerRadius = 16
        containerView.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        containerView.translatesAutoresizingMaskIntoConstraints = false

        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.spacing = 0
        stackView.translatesAutoresizingMaskIntoConstraints = false

        for (index, option) in options.enumerated() {
            let button = createOptionButton(title: option, color: itemColor, isFirst: index == 0) {
                dismissActionSheet(backgroundView) {
                    sendResult(callback_id: callback_id, tapIndex: index)
                }
            }
            stackView.addArrangedSubview(button)

            if index < options.count - 1 {
                stackView.addArrangedSubview(createSeparator())
            }
        }

        stackView.addArrangedSubview(createThickSeparator())

        let cancelButton = createCancelButton(title: cancelText) {
            dismissActionSheet(backgroundView) {
                sendResult(callback_id: callback_id, tapIndex: -1)
            }
        }
        stackView.addArrangedSubview(cancelButton)

        containerView.addSubview(stackView)
        backgroundView.addSubview(containerView)

        // Setup constraints
        NSLayoutConstraint.activate([
            // Stack view constraints
            stackView.topAnchor.constraint(equalTo: containerView.topAnchor),
            stackView.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            stackView.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            stackView.bottomAnchor.constraint(equalTo: containerView.bottomAnchor),

            // Container constraints
            containerView.leadingAnchor.constraint(equalTo: backgroundView.leadingAnchor),
            containerView.trailingAnchor.constraint(equalTo: backgroundView.trailingAnchor),
            containerView.bottomAnchor.constraint(equalTo: backgroundView.bottomAnchor)
        ])

        // Add tap gesture to background to dismiss
        let tapGesture = UITapGestureRecognizer(target: nil, action: nil)
        tapGesture.addTarget(backgroundView, action: #selector(UIView.handleBackgroundTap))
        backgroundView.addGestureRecognizer(tapGesture)

        // Store callback for background tap
        backgroundView.tag = Int(callback_id)

        return backgroundView
    }

    @MainActor
    private static func createOptionButton(title: String, color: String, isFirst: Bool = false, action: @escaping () -> Void) -> UIButton {
        let button = UIButton(type: .system)
        button.setTitle(title, for: .normal)

        // Parse color from hex string, use a softer black as fallback (similar to Android)
        let buttonColor = parseColor(color) ?? UIColor(red: 0.2, green: 0.2, blue: 0.2, alpha: 1.0)
        button.setTitleColor(buttonColor, for: .normal)

        button.titleLabel?.font = UIFont.systemFont(ofSize: 18)
        button.backgroundColor = isFirst ? UIColor.white : UIColor.clear
        button.contentHorizontalAlignment = .center
        button.translatesAutoresizingMaskIntoConstraints = false

        if isFirst {
            button.layer.cornerRadius = 16
            button.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        }

        button.heightAnchor.constraint(equalToConstant: 56).isActive = true
        button.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return button
    }

    /// Parse hex color string to UIColor
    @MainActor
    private static func parseColor(_ hexString: String) -> UIColor? {
        var hex = hexString.trimmingCharacters(in: .whitespacesAndNewlines)
        if hex.hasPrefix("#") {
            hex.removeFirst()
        }

        guard hex.count == 6 else { return nil }

        var rgbValue: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&rgbValue)

        return UIColor(
            red: CGFloat((rgbValue & 0xFF0000) >> 16) / 255.0,
            green: CGFloat((rgbValue & 0x00FF00) >> 8) / 255.0,
            blue: CGFloat(rgbValue & 0x0000FF) / 255.0,
            alpha: 1.0
        )
    }

    /// Create cancel button matching Android style
    @MainActor
    private static func createCancelButton(title: String, action: @escaping () -> Void) -> UIView {
        let containerView = UIView()
        containerView.backgroundColor = UIColor.white
        containerView.translatesAutoresizingMaskIntoConstraints = false

        let button = UIButton(type: .system)
        button.setTitle(title, for: .normal)
        button.setTitleColor(UIColor.black, for: .normal)
        button.titleLabel?.font = UIFont.systemFont(ofSize: 18, weight: .medium)
        button.backgroundColor = UIColor.white
        button.contentHorizontalAlignment = .center
        button.translatesAutoresizingMaskIntoConstraints = false

        button.addAction(UIAction { _ in action() }, for: .touchUpInside)

        containerView.addSubview(button)

        // Calculate safe area bottom inset (but limit it to reasonable amount)
        let safeAreaBottom = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first?.windows.first?.safeAreaInsets.bottom ?? 0
        let limitedSafeArea = min(safeAreaBottom, 20) // Limit to max 20pt

        NSLayoutConstraint.activate([
            button.topAnchor.constraint(equalTo: containerView.topAnchor),
            button.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            button.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            button.heightAnchor.constraint(equalToConstant: 56),
            containerView.bottomAnchor.constraint(equalTo: button.bottomAnchor, constant: limitedSafeArea)
        ])

        return containerView
    }

    @MainActor
    private static func createSeparator() -> UIView {
        let separator = UIView()
        separator.backgroundColor = UIColor(red: 0.88, green: 0.88, blue: 0.88, alpha: 1.0)
        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.heightAnchor.constraint(equalToConstant: 1).isActive = true
        return separator
    }

    @MainActor
    private static func createThickSeparator() -> UIView {
        let separator = UIView()
        separator.backgroundColor = UIColor(red: 0.95, green: 0.95, blue: 0.95, alpha: 1.0)
        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.heightAnchor.constraint(equalToConstant: 8).isActive = true
        return separator
    }

    @MainActor
    private static func presentCustomActionSheet(_ actionSheetView: UIView, on viewController: UIViewController) {
        viewController.view.addSubview(actionSheetView)
        guard let containerView = actionSheetView.subviews.first else { return }

        actionSheetView.layoutIfNeeded()
        let offscreenY = max(containerView.frame.height, 200) + 100
        containerView.transform = CGAffineTransform(translationX: 0, y: offscreenY)

        UIView.animate(withDuration: 0.3, delay: 0, options: .curveEaseOut) {
            actionSheetView.alpha = 1
            containerView.transform = .identity
        }
    }

    @MainActor
    internal static func dismissActionSheet(_ actionSheetView: UIView, completion: @escaping () -> Void) {
        guard let containerView = actionSheetView.subviews.first else {
            completion()
            return
        }

        let offscreenY = max(containerView.frame.height, 200) + 100
        UIView.animate(withDuration: 0.3, delay: 0, options: .curveEaseIn) {
            actionSheetView.alpha = 0
            containerView.transform = CGAffineTransform(translationX: 0, y: offscreenY)
        } completion: { _ in
            actionSheetView.removeFromSuperview()
            completion()
        }
    }
    #endif

}

#if os(iOS)
extension UIView {
    @objc func handleBackgroundTap() {
        guard let callback_id = UInt64(exactly: self.tag) else { return }
        LxAppActionSheet.dismissActionSheet(self) {
            LxAppActionSheet.sendResult(callback_id: callback_id, tapIndex: -1)
        }
    }
}
#endif
