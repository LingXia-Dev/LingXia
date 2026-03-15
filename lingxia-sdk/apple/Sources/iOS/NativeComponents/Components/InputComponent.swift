import Foundation

#if os(iOS)
import UIKit

@MainActor
final class InputComponentFactory: LxNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> LxNativeComponent {
        InputComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class InputComponent: NSObject, LxNativeComponent, UITextFieldDelegate {
    let id: String
    let view: UIView

    private let textField: UITextField
    private let eventSink: ([String: Any]) -> Void
    private var suppressInputEvent = false
    private var maxLength: Int = -1
    private var confirmHold = false
    private var holdKeyboard = false
    private var showConfirmBar = true
    private var lastFocusedValue: String = ""
    private var keyboardToolbar: UIToolbar?
    private var semanticType: InputSemanticType = .text
    private var lastKnownKeyboardHeight: CGFloat = 0
    private var isMounted = false
    private var lastFocusPropValue = false
    private var resolvedProps: [String: Any] = [:]

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.eventSink = eventSink
        self.view = UIView()
        self.textField = LxNativeInputTextField(frame: .zero)
        self.resolvedProps = initialProps
        super.init()

        view.backgroundColor = .clear
        textField.backgroundColor = .clear
        textField.borderStyle = .none
        textField.autocorrectionType = .no
        textField.autocapitalizationType = .none
        textField.clearButtonMode = .never
        textField.delegate = self
        textField.addTarget(self, action: #selector(onEditingChanged), for: .editingChanged)
        textField.addTarget(self, action: #selector(onEditingDidBegin), for: .editingDidBegin)
        textField.addTarget(self, action: #selector(onEditingDidEnd), for: .editingDidEnd)
        view.addSubview(textField)

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
        layoutTextField()
        isMounted = true
        if lastFocusPropValue && !textField.isFirstResponder {
            textField.becomeFirstResponder()
        }
    }

    func update(props: [String: Any]) {
        if !props.isEmpty {
            for (key, value) in props {
                resolvedProps[key] = value
            }
        }
        let state = resolvedProps

        let disabled = state.bool("disabled", default: false)
        textField.isEnabled = !disabled

        let type = state.string("type")?.lowercased() ?? "text"
        let password = state.bool("password", default: false) || type == "password" || type == "safe-password"
        showConfirmBar = state.bool("showConfirmBar", default: state.bool("show-confirm-bar", default: true))
        configureKeyboard(type: type, password: password, showConfirmBar: showConfirmBar)
        semanticType = password ? .password : Self.semanticType(for: type)

        maxLength = state.int("maxlength", default: state.int("maxLength", default: -1))
        if let incomingValue = state.string("value") {
            var nextValue = Self.sanitizedValue(incomingValue, for: semanticType)
            if maxLength >= 0 && nextValue.count > maxLength {
                nextValue = String(nextValue.prefix(maxLength))
            }
            if nextValue != textField.text {
                suppressInputEvent = true
                textField.text = nextValue
                setSelection(start: nextValue.count, end: nextValue.count)
                suppressInputEvent = false
            }
        } else {
            sanitizeCurrentValueIfNeeded()
            enforceMaxLengthIfNeeded()
        }

        if let placeholder = state.string("placeholder") {
            textField.placeholder = placeholder
        }
        let placeholderStyle = state.string("placeholderStyle") ?? state.string("placeholder-style")
        applyPlaceholderStyle(placeholderStyle)

        confirmHold = state.bool("confirmHold", default: state.bool("confirm-hold", default: false))
        holdKeyboard = state.bool("holdKeyboard", default: state.bool("hold-keyboard", default: false))
        let confirmType = state.string("confirmType") ?? state.string("confirm-type") ?? "done"
        textField.returnKeyType = returnKeyType(confirmType.lowercased())

        if let cursorColorHex = state.string("cursorColor") ?? state.string("cursor-color"),
           let color = NativeComponentColorStyle.parseColor(cursorColorHex) {
            textField.tintColor = color
        }

        let selectionStart = state.int("selectionStart", default: state.int("selection-start", default: -1))
        let selectionEnd = state.int("selectionEnd", default: state.int("selection-end", default: -1))
        if selectionStart >= 0 && selectionEnd >= selectionStart {
            setSelection(start: selectionStart, end: selectionEnd)
        } else {
            let cursor = state.int("cursor", default: -1)
            if cursor >= 0 {
                setSelection(start: cursor, end: cursor)
            }
        }

        let focus = state.bool("focus", default: false) || state.bool("autoFocus", default: state.bool("auto-focus", default: false))
        if isMounted {
            if focus && !lastFocusPropValue && !textField.isFirstResponder {
                textField.becomeFirstResponder()
            } else if !focus && lastFocusPropValue && textField.isFirstResponder {
                textField.resignFirstResponder()
            }
        }
        lastFocusPropValue = focus
    }

    func setFrame(_ frame: CGRect) {
        view.frame = frame.integral
        layoutTextField()
    }

    func focus() {
        textField.becomeFirstResponder()
    }

    func blur() {
        textField.resignFirstResponder()
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
        textField.delegate = nil
        textField.removeTarget(self, action: #selector(onEditingChanged), for: .editingChanged)
        textField.removeTarget(self, action: #selector(onEditingDidBegin), for: .editingDidBegin)
        textField.removeTarget(self, action: #selector(onEditingDidEnd), for: .editingDidEnd)
        view.removeFromSuperview()
    }

    private func layoutTextField() {
        textField.frame = view.bounds
    }

    private func configureKeyboard(type: String, password: Bool, showConfirmBar: Bool) {
        let previousKeyboardType = textField.keyboardType
        let previousSecure = textField.isSecureTextEntry
        let previousAccessory = textField.inputAccessoryView

        if password {
            textField.keyboardType = .asciiCapable
            _ = setSecureTextEntryIfNeeded(true)
            textField.textContentType = .password
            textField.inputAccessoryView = showConfirmBar ? getKeyboardToolbar() : nil
        } else {
            _ = setSecureTextEntryIfNeeded(false)
            textField.textContentType = nil
            switch type {
            case "number":
                textField.keyboardType = .numberPad
                textField.inputAccessoryView = showConfirmBar ? getKeyboardToolbar() : nil
            case "digit":
                textField.keyboardType = .decimalPad
                textField.inputAccessoryView = showConfirmBar ? getKeyboardToolbar() : nil
            default:
                textField.keyboardType = .default
                textField.inputAccessoryView = showConfirmBar ? getKeyboardToolbar() : nil
            }
        }

        let keyboardChanged = previousKeyboardType != textField.keyboardType ||
            previousSecure != textField.isSecureTextEntry ||
            previousAccessory !== textField.inputAccessoryView
        if keyboardChanged && textField.isFirstResponder {
            textField.reloadInputViews()
        }
    }

    @discardableResult
    private func setSecureTextEntryIfNeeded(_ enabled: Bool) -> Bool {
        if textField.isSecureTextEntry == enabled {
            return false
        }
        let wasFirstResponder = textField.isFirstResponder
        let currentText = textField.text
        textField.isSecureTextEntry = enabled
        if wasFirstResponder {
            textField.text = currentText
            let end = textField.endOfDocument
            if let range = textField.textRange(from: end, to: end) {
                textField.selectedTextRange = range
            }
        }
        return true
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
        let value = textField.text ?? ""
        emit("confirm", detail: detailWithValue(value))
        if shouldDismissKeyboardOnConfirm() {
            textField.resignFirstResponder()
        }
    }

    private func shouldDismissKeyboardOnConfirm() -> Bool {
        return !confirmHold && !holdKeyboard
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
        default:
            return .done
        }
    }

    private func applyPlaceholderStyle(_ rawStyle: String?) {
        let placeholder = textField.placeholder ?? ""
        guard !placeholder.isEmpty else {
            textField.attributedPlaceholder = nil
            return
        }
        guard let style = rawStyle?.trimmingCharacters(in: .whitespacesAndNewlines),
              !style.isEmpty else {
            textField.attributedPlaceholder = nil
            textField.placeholder = placeholder
            return
        }
        let colorExpr = NativeComponentColorStyle.extractColorFromStyle(style) ?? style
        guard let color = NativeComponentColorStyle.parseColor(colorExpr) else {
            textField.attributedPlaceholder = nil
            textField.placeholder = placeholder
            return
        }
        textField.attributedPlaceholder = NSAttributedString(
            string: placeholder,
            attributes: [.foregroundColor: color]
        )
    }

    private func setSelection(start: Int, end: Int) {
        guard let text = textField.text else { return }
        let length = text.count
        let clampedStart = min(max(0, start), length)
        let clampedEnd = min(max(clampedStart, end), length)
        guard let startPos = textField.position(from: textField.beginningOfDocument, offset: clampedStart),
              let endPos = textField.position(from: textField.beginningOfDocument, offset: clampedEnd),
              let range = textField.textRange(from: startPos, to: endPos) else {
            return
        }
        textField.selectedTextRange = range
    }

    private func detailWithValue(_ value: String) -> [String: Any] {
        [
            "value": value,
            "cursor": currentCursor()
        ]
    }

    private func currentCursor() -> Int {
        guard let range = textField.selectedTextRange else { return textField.text?.count ?? 0 }
        return textField.offset(from: textField.beginningOfDocument, to: range.start)
    }

    private func emit(_ event: String, detail: [String: Any]) {
        eventSink([
            "event": event,
            "detail": detail
        ])
    }

    @objc
    private func onEditingChanged() {
        guard !suppressInputEvent else { return }
        var text = textField.text ?? ""
        let sanitized = Self.sanitizedValue(text, for: semanticType)
        if sanitized != text {
            suppressInputEvent = true
            textField.text = sanitized
            setSelection(start: sanitized.count, end: sanitized.count)
            suppressInputEvent = false
            text = sanitized
        }
        if maxLength >= 0 && text.count > maxLength {
            let clamped = String(text.prefix(maxLength))
            suppressInputEvent = true
            textField.text = clamped
            setSelection(start: clamped.count, end: clamped.count)
            suppressInputEvent = false
            emit("input", detail: [
                "value": clamped,
                "cursor": currentCursor()
            ])
            return
        }
        emit("input", detail: [
            "value": text,
            "cursor": currentCursor()
        ])
    }

    @objc
    private func onEditingDidBegin() {
        let value = textField.text ?? ""
        lastFocusedValue = value
        var detail = detailWithValue(value)
        detail["height"] = lastKnownKeyboardHeight
        emit("focus", detail: detail)
    }

    @objc
    private func onEditingDidEnd() {
        // Reset so the next JS focus:true prop re-triggers becomeFirstResponder.
        lastFocusPropValue = false
        let value = textField.text ?? ""
        emit("blur", detail: detailWithValue(value))
        if value != lastFocusedValue {
            emit("change", detail: detailWithValue(value))
        }
    }

    @objc
    private func onKeyboardWillChangeFrame(_ notification: Notification) {
        guard textField.isFirstResponder,
              let userInfo = notification.userInfo,
              let endFrame = (userInfo[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue,
              let duration = userInfo[UIResponder.keyboardAnimationDurationUserInfoKey] as? NSNumber,
              let window = textField.window else {
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

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        let value = textField.text ?? ""
        emit("confirm", detail: detailWithValue(value))
        if shouldDismissKeyboardOnConfirm() {
            textField.resignFirstResponder()
        }
        return false
    }

    func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        let current = textField.text ?? ""
        guard let stringRange = Range(range, in: current) else { return true }
        let updated = current.replacingCharacters(in: stringRange, with: string)
        if maxLength >= 0 && updated.count > maxLength {
            return false
        }
        return Self.sanitizedValue(updated, for: semanticType) == updated
    }

    private func sanitizeCurrentValueIfNeeded() {
        let current = textField.text ?? ""
        let sanitized = Self.sanitizedValue(current, for: semanticType)
        if current == sanitized { return }
        suppressInputEvent = true
        textField.text = sanitized
        setSelection(start: sanitized.count, end: sanitized.count)
        suppressInputEvent = false
    }

    private func enforceMaxLengthIfNeeded() {
        guard maxLength >= 0 else { return }
        let current = textField.text ?? ""
        guard current.count > maxLength else { return }
        let clamped = String(current.prefix(maxLength))
        suppressInputEvent = true
        textField.text = clamped
        setSelection(start: clamped.count, end: clamped.count)
        suppressInputEvent = false
    }

    private static func semanticType(for rawType: String) -> InputSemanticType {
        switch rawType {
        case "number":
            return .number
        case "digit":
            return .digit
        default:
            return .text
        }
    }

    private static func sanitizedValue(_ value: String, for type: InputSemanticType) -> String {
        switch type {
        case .number:
            return String(value.filter { $0.isNumber })
        case .digit:
            var seenDot = false
            var result = ""
            result.reserveCapacity(value.count)
            for ch in value {
                if ch.isNumber {
                    result.append(ch)
                } else if ch == "." && !seenDot {
                    seenDot = true
                    result.append(ch)
                }
            }
            return result
        case .text, .password:
            return value
        }
    }
}

private enum InputSemanticType {
    case text
    case number
    case digit
    case password
}

/// UITextField subclass that adds a small horizontal inset so the first character
/// is not flush against the component edge. Matches the default web <input> feel.
private final class LxNativeInputTextField: UITextField {
    private let hInset: CGFloat = 8
    override func textRect(forBounds bounds: CGRect) -> CGRect {
        bounds.insetBy(dx: hInset, dy: 0)
    }
    override func editingRect(forBounds bounds: CGRect) -> CGRect {
        bounds.insetBy(dx: hInset, dy: 0)
    }
    override func placeholderRect(forBounds bounds: CGRect) -> CGRect {
        bounds.insetBy(dx: hInset, dy: 0)
    }
}

private extension Dictionary where Key == String, Value == Any {
    func string(_ key: String) -> String? {
        self[key] as? String
    }

    func bool(_ key: String, default defaultValue: Bool) -> Bool {
        guard let value = self[key] else { return defaultValue }
        if let boolValue = value as? Bool { return boolValue }
        if let number = value as? NSNumber { return number.boolValue }
        if let string = value as? String {
            let normalized = string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if normalized == "true" || normalized == "1" { return true }
            if normalized == "false" || normalized == "0" { return false }
        }
        return defaultValue
    }

    func int(_ key: String, default defaultValue: Int) -> Int {
        guard let value = self[key] else { return defaultValue }
        if let intValue = value as? Int { return intValue }
        if let number = value as? NSNumber { return number.intValue }
        if let string = value as? String, let intValue = Int(string) { return intValue }
        return defaultValue
    }
}

#endif
