import Foundation

#if os(iOS)
import UIKit

@MainActor
final class TextareaComponentFactory: LxNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> LxNativeComponent {
        TextareaComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class TextareaComponent: NSObject, LxNativeComponent, UITextViewDelegate {
    let id: String
    let view: UIView

    private let textView: UITextView
    private let placeholderLabel: UILabel
    private let eventSink: ([String: Any]) -> Void
    private var suppressInputEvent = false
    private var maxLength: Int = -1
    private var confirmHold = false
    private var holdKeyboard = false
    private var showConfirmBar = true
    private var lastLineCount: Int = 0
    private var lastContentHeight: CGFloat = 0
    private var autoHeight = false
    private var keyboardToolbar: UIToolbar?
    private var lastKnownKeyboardHeight: CGFloat = 0
    private var isMounted = false
    private var lastFocusPropValue = false
    private var lastFocusedValue = ""
    private var resolvedProps: [String: Any] = [:]

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.eventSink = eventSink
        self.view = UIView()
        self.textView = UITextView(frame: .zero)
        self.placeholderLabel = UILabel(frame: .zero)
        self.resolvedProps = initialProps
        super.init()

        view.backgroundColor = .clear
        textView.backgroundColor = .clear
        textView.delegate = self
        textView.autocorrectionType = .no
        textView.autocapitalizationType = .none
        textView.textContainerInset = UIEdgeInsets(top: 4, left: 4, bottom: 4, right: 4)
        textView.textContainer.lineFragmentPadding = 0

        placeholderLabel.textColor = UIColor(white: 0.65, alpha: 1)
        placeholderLabel.numberOfLines = 0
        placeholderLabel.font = textView.font ?? UIFont.systemFont(ofSize: 16)

        view.addSubview(textView)
        view.addSubview(placeholderLabel)

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(onKeyboardWillChangeFrame(_:)),
            name: UIResponder.keyboardWillChangeFrameNotification,
            object: nil
        )

        update(props: initialProps)
    }

    func mount(in host: UIView) {
        if view.superview !== host {
            host.addSubview(view)
        }
        layoutSubviews()
        isMounted = true
        if lastFocusPropValue && !textView.isFirstResponder {
            textView.becomeFirstResponder()
        }
    }

    func update(props: [String: Any]) {
        if !props.isEmpty {
            for (key, value) in props {
                resolvedProps[key] = value
            }
        }
        let state = resolvedProps

        maxLength = int(from: state["maxlength"], default: int(from: state["maxLength"], default: -1))

        if let value = string(from: state["value"]), value != textView.text {
            suppressInputEvent = true
            if maxLength >= 0 && value.count > maxLength {
                textView.text = String(value.prefix(maxLength))
            } else {
                textView.text = value
            }
            suppressInputEvent = false
            updatePlaceholderVisibility()
            emitLinechangeIfNeeded()
        } else {
            enforceMaxLengthIfNeeded()
        }

        if let placeholder = string(from: state["placeholder"]) {
            placeholderLabel.text = placeholder
        }
        let placeholderStyle = string(from: state["placeholderStyle"]) ?? string(from: state["placeholder-style"])
        applyPlaceholderStyle(placeholderStyle)

        if let placeholderColor = string(from: state["placeholderColor"]) ?? string(from: state["placeholder-color"]),
           let color = NativeComponentColorStyle.parseColor(placeholderColor) {
            placeholderLabel.textColor = color
        }

        if let textColorHex = string(from: state["textColor"]) ?? string(from: state["text-color"]),
           let textColor = NativeComponentColorStyle.parseColor(textColorHex) {
            textView.textColor = textColor
        }

        if let cursorColorHex = string(from: state["cursorColor"]) ?? string(from: state["cursor-color"]),
           let cursorColor = NativeComponentColorStyle.parseColor(cursorColorHex) {
            textView.tintColor = cursorColor
        }

        let disabled = bool(from: state["disabled"], default: false)
        textView.isEditable = !disabled
        textView.isSelectable = !disabled

        let confirmType = (string(from: state["confirmType"]) ?? string(from: state["confirm-type"]) ?? "return").lowercased()
        textView.returnKeyType = returnKeyType(confirmType)
        confirmHold = bool(from: state["confirmHold"], default: bool(from: state["confirm-hold"], default: false))
        holdKeyboard = bool(from: state["holdKeyboard"], default: bool(from: state["hold-keyboard"], default: false))
        showConfirmBar = bool(from: state["showConfirmBar"], default: bool(from: state["show-confirm-bar"], default: true))
        let previousAccessory = textView.inputAccessoryView
        textView.inputAccessoryView = showConfirmBar ? getKeyboardToolbar() : nil
        if previousAccessory !== textView.inputAccessoryView && textView.isFirstResponder {
            textView.reloadInputViews()
        }

        autoHeight = bool(from: state["autoHeight"], default: bool(from: state["auto-height"], default: false))
        textView.isScrollEnabled = !autoHeight
        if autoHeight {
            textView.setContentOffset(.zero, animated: false)
        }

        let selectionStart = int(from: state["selectionStart"], default: int(from: state["selection-start"], default: -1))
        let selectionEnd = int(from: state["selectionEnd"], default: int(from: state["selection-end"], default: -1))
        if selectionStart >= 0 && selectionEnd >= selectionStart {
            let length = textView.text.count
            let start = min(max(0, selectionStart), length)
            let end = min(max(start, selectionEnd), length)
            textView.selectedRange = NSRange(location: start, length: end - start)
        } else {
            let cursor = int(from: state["cursor"], default: -1)
            if cursor >= 0 {
                let length = textView.text.count
                let location = min(max(0, cursor), length)
                textView.selectedRange = NSRange(location: location, length: 0)
            }
        }

        let focus = bool(from: state["focus"], default: false) ||
            bool(from: state["autoFocus"], default: bool(from: state["auto-focus"], default: false))
        if isMounted {
            if focus && !lastFocusPropValue && !textView.isFirstResponder {
                textView.becomeFirstResponder()
            } else if !focus && lastFocusPropValue && textView.isFirstResponder {
                textView.resignFirstResponder()
            }
        }
        lastFocusPropValue = focus
    }

    private func enforceMaxLengthIfNeeded() {
        guard maxLength >= 0 else { return }
        let current = textView.text ?? ""
        guard current.count > maxLength else { return }
        suppressInputEvent = true
        textView.text = String(current.prefix(maxLength))
        suppressInputEvent = false
        updatePlaceholderVisibility()
    }

    func setFrame(_ frame: CGRect) {
        view.frame = frame.integral
        layoutSubviews()
    }

    func focus() {
        textView.becomeFirstResponder()
    }

    func blur() {
        textView.resignFirstResponder()
    }

    func handleCommand(name: String, params: [String: Any]?) {
        switch name {
        case "focus":
            focus()
        case "blur":
            blur()
        default:
            break
        }
    }

    func unmount() {
        NotificationCenter.default.removeObserver(self)
        textView.delegate = nil
        view.removeFromSuperview()
    }

    private func layoutSubviews() {
        textView.frame = view.bounds
        let labelWidth = max(0, view.bounds.width - 8)
        let labelHeight = placeholderLabel.sizeThatFits(CGSize(width: labelWidth, height: .greatestFiniteMagnitude)).height
        placeholderLabel.frame = CGRect(x: 4, y: 4, width: labelWidth, height: labelHeight)
        emitLinechangeIfNeeded()
    }

    private func updatePlaceholderVisibility() {
        placeholderLabel.isHidden = !(textView.text?.isEmpty ?? true)
    }

    private func applyPlaceholderStyle(_ rawStyle: String?) {
        guard let style = rawStyle?.trimmingCharacters(in: .whitespacesAndNewlines),
              !style.isEmpty else {
            placeholderLabel.textColor = UIColor(white: 0.65, alpha: 1)
            return
        }
        let colorExpr = NativeComponentColorStyle.extractColorFromStyle(style) ?? style
        if let color = NativeComponentColorStyle.parseColor(colorExpr) {
            placeholderLabel.textColor = color
        }
    }

    private func currentDetail() -> [String: Any] {
        let selectionStart = max(0, textView.selectedRange.location)
        let selectionEnd = max(selectionStart, selectionStart + textView.selectedRange.length)
        let detail: [String: Any] = [
            "value": textView.text ?? "",
            "cursor": selectionStart,
            "selectionStart": selectionStart,
            "selectionEnd": selectionEnd
        ]
        return detail
    }

    private func emit(_ event: String, detail: [String: Any]) {
        eventSink([
            "event": event,
            "detail": detail
        ])
    }

    private func emitLinechangeIfNeeded() {
        let lineCount = max(1, estimatedLineCount())
        let contentHeight = measuredContentHeight()
        if lineCount == lastLineCount && abs(contentHeight - lastContentHeight) < 0.5 { return }
        lastLineCount = lineCount
        lastContentHeight = contentHeight
        emit("linechange", detail: [
            "lineCount": lineCount,
            "height": contentHeight,
            "heightRpx": contentHeight
        ])
    }

    private func measuredContentHeight() -> CGFloat {
        let width = max(1, textView.bounds.width)
        let fitted = textView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height
        return max(textView.font?.lineHeight ?? 1, fitted)
    }

    private func estimatedLineCount() -> Int {
        guard let font = textView.font else { return 1 }
        let text = textView.text ?? ""
        if text.isEmpty { return 1 }
        let width = max(textView.bounds.width, 1)
        let bounding = (text as NSString).boundingRect(
            with: CGSize(width: width, height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: font],
            context: nil
        )
        let lineHeight = max(font.lineHeight, 1)
        return Int(ceil(bounding.height / lineHeight))
    }

    @objc
    private func onKeyboardWillChangeFrame(_ notification: Notification) {
        guard textView.isFirstResponder,
              let userInfo = notification.userInfo,
              let endFrame = (userInfo[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue,
              let duration = userInfo[UIResponder.keyboardAnimationDurationUserInfoKey] as? NSNumber,
              let window = textView.window else {
            return
        }
        let frameInWindow = window.convert(endFrame, from: nil)
        let height = max(0, window.bounds.maxY - frameInWindow.minY)
        lastKnownKeyboardHeight = height
        emit("keyboardheightchange", detail: [
            "height": height,
            "duration": duration.doubleValue
        ])
    }

    func textViewDidChange(_ textView: UITextView) {
        guard !suppressInputEvent else { return }
        if maxLength >= 0, let text = textView.text, text.count > maxLength {
            let clamped = String(text.prefix(maxLength))
            suppressInputEvent = true
            textView.text = clamped
            textView.selectedRange = NSRange(location: clamped.count, length: 0)
            suppressInputEvent = false
        }
        updatePlaceholderVisibility()
        var inputDetail = currentDetail()
        inputDetail["lineCount"] = max(1, estimatedLineCount())
        emit("input", detail: inputDetail)
        emitLinechangeIfNeeded()
        DispatchQueue.main.async { [weak self] in
            self?.emitLinechangeIfNeeded()
        }
    }

    func textViewDidBeginEditing(_ textView: UITextView) {
        lastFocusedValue = textView.text ?? ""
        var detail = currentDetail()
        detail["height"] = lastKnownKeyboardHeight
        emit("focus", detail: detail)
    }

    func textViewDidEndEditing(_ textView: UITextView) {
        // Reset so the next JS focus:true prop re-triggers becomeFirstResponder.
        lastFocusPropValue = false
        let detail = currentDetail()
        emit("blur", detail: detail)
        if (detail["value"] as? String ?? "") != lastFocusedValue {
            emit("change", detail: detail)
        }
    }

    func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
        if text == "\n" && textView.returnKeyType != .default {
            emit("confirm", detail: currentDetail())
            if shouldDismissKeyboardOnConfirm() {
                textView.resignFirstResponder()
            }
            return false
        }
        guard maxLength >= 0 else { return true }
        let current = textView.text ?? ""
        guard let stringRange = Range(range, in: current) else { return true }
        let updated = current.replacingCharacters(in: stringRange, with: text)
        return updated.count <= maxLength
    }

    private func string(from value: Any?) -> String? {
        value as? String
    }

    private func bool(from value: Any?, default defaultValue: Bool) -> Bool {
        guard let value else { return defaultValue }
        if let boolValue = value as? Bool { return boolValue }
        if let number = value as? NSNumber { return number.boolValue }
        if let string = value as? String {
            let normalized = string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if normalized == "true" || normalized == "1" { return true }
            if normalized == "false" || normalized == "0" { return false }
        }
        return defaultValue
    }

    private func int(from value: Any?, default defaultValue: Int) -> Int {
        guard let value else { return defaultValue }
        if let intValue = value as? Int { return intValue }
        if let number = value as? NSNumber { return number.intValue }
        if let string = value as? String, let intValue = Int(string) { return intValue }
        return defaultValue
    }

    private func returnKeyType(_ confirmType: String) -> UIReturnKeyType {
        switch confirmType {
        case "send":
            return .send
        case "search":
            return .search
        case "next":
            return .next
        case "go":
            return .go
        case "return":
            return .default
        default:
            return .done
        }
    }

    private func getKeyboardToolbar() -> UIToolbar {
        if let toolbar = keyboardToolbar { return toolbar }
        let toolbar = UIToolbar(frame: CGRect(x: 0, y: 0, width: 0, height: 44))
        toolbar.sizeToFit()
        let flexSpace = UIBarButtonItem(barButtonSystemItem: .flexibleSpace, target: nil, action: nil)
        let doneButton = UIBarButtonItem(barButtonSystemItem: .done, target: self, action: #selector(onKeyboardDoneTapped))
        toolbar.items = [flexSpace, doneButton]
        keyboardToolbar = toolbar
        return toolbar
    }

    @objc
    private func onKeyboardDoneTapped() {
        emit("confirm", detail: currentDetail())
        if shouldDismissKeyboardOnConfirm() {
            textView.resignFirstResponder()
        }
    }

    private func shouldDismissKeyboardOnConfirm() -> Bool {
        return !confirmHold && !holdKeyboard
    }

}

#endif
