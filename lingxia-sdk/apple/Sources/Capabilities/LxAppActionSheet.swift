import Foundation
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

#if os(iOS)
/// Colors of the custom action sheet, resolved from the scheme the overlay
/// must adopt (the current lxapp's, else the host's).
@MainActor
private struct ActionSheetPalette {
    let scrim: UIColor
    let surface: UIColor
    let label: UIColor
    let separator: UIColor
    let gap: UIColor

    static func resolve() -> ActionSheetPalette {
        if LxAppAppearanceRegistry.overlayIsDark() {
            return ActionSheetPalette(
                scrim: UIColor.black.withAlphaComponent(0.55),
                surface: UIColor(red: 0.11, green: 0.11, blue: 0.12, alpha: 1.0),
                label: .white,
                separator: UIColor(white: 1.0, alpha: 0.12),
                gap: UIColor.black.withAlphaComponent(0.4))
        }
        return ActionSheetPalette(
            scrim: UIColor.black.withAlphaComponent(0.4),
            surface: .white,
            label: .black,
            separator: UIColor(red: 0.88, green: 0.88, blue: 0.88, alpha: 1.0),
            gap: UIColor(red: 0.95, green: 0.95, blue: 0.95, alpha: 1.0))
    }
}
#endif

class LxAppActionSheet {

    static func showActionSheet(options: ActionSheetOptions, callback_id: UInt64) {
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

    static func showActionSheet(_ options: [String: Any], callback_id: UInt64) {
        #if os(iOS)
        let optionsArray = options["options"] as? [String] ?? []
        let cancelText = options["cancelText"] as? String ?? ""
        let itemColor = options["itemColor"] as? String ?? "#007AFF"

        DispatchQueue.main.async {
            showIOSActionSheet(options: optionsArray, cancelText: cancelText, itemColor: itemColor, callback_id: callback_id)
        }
        #elseif os(macOS)
        let optionsArray = options["options"] as? [String] ?? []
        let cancelText = options["cancelText"] as? String ?? ""
        DispatchQueue.main.async {
            showMacActionSheet(
                options: optionsArray,
                cancelText: cancelText,
                callback_id: callback_id
            )
        }
        #endif
    }

    /// A dismissal is business code 2000; everything that merely *failed* —
    /// no presenter, serialization — reports the generic failure code, so
    /// `canceled: true` on the JS side can only ever mean the user said no.
    private static let actionSheetFailureCode = "1000"

    internal static func sendResult(callback_id: UInt64, tapIndex: Int) {
        if tapIndex < 0 {
            _ = onCallback(callback_id, false, "2000")
            return
        }
        let result = ["tapIndex": tapIndex]
        if let jsonData = try? JSONSerialization.data(withJSONObject: result),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            _ = onCallback(callback_id, true, jsonString)
        } else {
            _ = onCallback(callback_id, false, actionSheetFailureCode)
        }
    }

    /// The sheet could not be shown at all. Distinct from a dismissal.
    internal static func sendPresentationFailure(callback_id: UInt64) {
        _ = onCallback(callback_id, false, actionSheetFailureCode)
    }

    #if os(iOS)
    @MainActor
    private static func showIOSActionSheet(options: [String], cancelText: String, itemColor: String, callback_id: UInt64) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
              let rootViewController = window.rootViewController else {
            LXLog.error("Could not find root view controller", category: "ActionSheet")
            sendPresentationFailure(callback_id: callback_id)
            return
        }

        var topViewController = rootViewController
        while let presentedViewController = topViewController.presentedViewController {
            topViewController = presentedViewController
        }

        let actionSheetView = createCustomActionSheet(options: options, cancelText: cancelText, itemColor: itemColor, callback_id: callback_id)
        guard presentCustomActionSheet(actionSheetView, on: topViewController) else {
            LXLog.error("Could not attach action sheet to a visible presenter", category: "ActionSheet")
            sendPresentationFailure(callback_id: callback_id)
            return
        }
    }

    @MainActor
    private static func createCustomActionSheet(options: [String], cancelText: String, itemColor: String, callback_id: UInt64) -> UIView {
        let palette = ActionSheetPalette.resolve()
        let backgroundView = UIView(frame: UIScreen.main.bounds)
        backgroundView.backgroundColor = palette.scrim
        backgroundView.alpha = 0

        let containerView = UIView()
        containerView.backgroundColor = palette.surface
        containerView.layer.cornerRadius = 16
        containerView.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        containerView.translatesAutoresizingMaskIntoConstraints = false

        let stackView = UIStackView()
        stackView.axis = .vertical
        stackView.spacing = 0
        stackView.translatesAutoresizingMaskIntoConstraints = false

        for (index, option) in options.enumerated() {
            let button = createOptionButton(
                title: option, color: itemColor, palette: palette, isFirst: index == 0) {
                dismissActionSheet(backgroundView) {
                    sendResult(callback_id: callback_id, tapIndex: index)
                }
            }
            stackView.addArrangedSubview(button)

            if index < options.count - 1 {
                stackView.addArrangedSubview(createSeparator(palette: palette))
            }
        }

        stackView.addArrangedSubview(createThickSeparator(palette: palette))

        let cancelButton = createCancelButton(title: cancelText, palette: palette) {
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
    private static func createOptionButton(
        title: String,
        color: String,
        palette: ActionSheetPalette,
        isFirst: Bool = false,
        action: @escaping () -> Void
    ) -> UIButton {
        let button = UIButton(type: .system)
        button.setTitle(title, for: .normal)

        // Parse color from hex string; without one the sheet's own label color
        // keeps the row legible in either scheme.
        let buttonColor = parseColor(color) ?? palette.label
        button.setTitleColor(buttonColor, for: .normal)

        button.titleLabel?.font = UIFont.systemFont(ofSize: 18)
        button.backgroundColor = isFirst ? palette.surface : UIColor.clear
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
    private static func createCancelButton(
        title: String,
        palette: ActionSheetPalette,
        action: @escaping () -> Void
    ) -> UIView {
        let containerView = UIView()
        containerView.backgroundColor = palette.surface
        containerView.translatesAutoresizingMaskIntoConstraints = false

        let button = UIButton(type: .system)
        button.setTitle(title, for: .normal)
        button.setTitleColor(palette.label, for: .normal)
        button.titleLabel?.font = UIFont.systemFont(ofSize: 18, weight: .medium)
        button.backgroundColor = palette.surface
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
    private static func createSeparator(palette: ActionSheetPalette) -> UIView {
        let separator = UIView()
        separator.backgroundColor = palette.separator
        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.heightAnchor.constraint(equalToConstant: 1).isActive = true
        return separator
    }

    @MainActor
    private static func createThickSeparator(palette: ActionSheetPalette) -> UIView {
        let separator = UIView()
        separator.backgroundColor = palette.gap
        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.heightAnchor.constraint(equalToConstant: 8).isActive = true
        return separator
    }

    @MainActor
    private static func presentCustomActionSheet(_ actionSheetView: UIView, on viewController: UIViewController) -> Bool {
        guard viewController.view.window != nil,
              let containerView = actionSheetView.subviews.first else {
            return false
        }
        viewController.view.addSubview(actionSheetView)

        actionSheetView.layoutIfNeeded()
        let offscreenY = max(containerView.frame.height, 200) + 100
        containerView.transform = CGAffineTransform(translationX: 0, y: offscreenY)

        UIView.animate(withDuration: 0.3, delay: 0, options: .curveEaseOut) {
            actionSheetView.alpha = 1
            containerView.transform = .identity
        }
        return true
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

    #if os(macOS)
    @MainActor
    private static func showMacActionSheet(
        options: [String],
        cancelText: String,
        callback_id: UInt64
    ) {
        let alert = NSAlert()
        alert.window.appearance =
            NSAppearance(named: LxAppAppearanceRegistry.overlayIsDark() ? .darkAqua : .aqua)
        for option in options {
            alert.addButton(withTitle: option)
        }
        alert.addButton(withTitle: cancelText)

        let firstButton = NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        let tapIndex = alert.runModal().rawValue - firstButton
        guard tapIndex >= 0, tapIndex < options.count else {
            sendResult(callback_id: callback_id, tapIndex: -1)
            return
        }
        sendResult(callback_id: callback_id, tapIndex: tapIndex)
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
