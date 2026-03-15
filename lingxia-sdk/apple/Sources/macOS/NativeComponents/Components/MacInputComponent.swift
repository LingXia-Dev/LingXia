#if os(macOS)
import Foundation
import AppKit

@MainActor
final class MacInputComponentFactory: MacNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> MacNativeComponent {
        MacInputComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class MacInputComponent: NSObject, MacNativeComponent, NSTextFieldDelegate {
    let id: String
    let view: NSView

    private var textField: NSTextField
    private let eventSink: ([String: Any]) -> Void
    private var suppressInputEvent = false
    private var maxLength: Int = -1
    private var confirmHold = false
    private var holdKeyboard = false
    private var confirmType = "done"
    private var lastFocusedValue: String = ""
    private var pendingSelection: NSRange?
    private var secureMode = false
    private var semanticType: InputSemanticType = .text
    private var resolvedProps: [String: Any] = [:]
    private var isMounted = false
    private var lastFocusPropValue = false

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.eventSink = eventSink
        self.view = MacInputContainerView(frame: .zero)
        self.textField = MacNativeTextField(frame: .zero)
        self.resolvedProps = initialProps
        super.init()

        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.clear.cgColor

        configureTextField(textField)
        view.addSubview(textField)

        update(props: initialProps)
    }

    func mount(in host: NSView) {
        if view.superview !== host {
            host.addSubview(view)
        }
        textField.frame = view.bounds
        isMounted = true
        if lastFocusPropValue {
            focus()
        }
    }

    func update(props: [String: Any]) {
        if !props.isEmpty {
            for (key, value) in props {
                resolvedProps[key] = value
            }
        }
        let state = resolvedProps

        if let value = state.string("value"), value != currentTextValue() {
            suppressInputEvent = true
            textField.stringValue = value
            suppressInputEvent = false
        }

        if let placeholder = state.string("placeholder") {
            textField.placeholderString = placeholder
        }
        let placeholderStyle = state.string("placeholderStyle") ?? state.string("placeholder-style")
        applyPlaceholderStyle(placeholderStyle)

        if let textColorRaw = state.string("textColor"), let textColor = NativeComponentColorStyle.parseColor(textColorRaw) {
            textField.textColor = textColor
            if let editor = textField.currentEditor() as? NSTextView {
                editor.textColor = textColor
                editor.insertionPointColor = textColor
            }
        } else {
            textField.textColor = .labelColor
            if let editor = textField.currentEditor() as? NSTextView {
                editor.textColor = .labelColor
                editor.insertionPointColor = .labelColor
            }
        }

        if let cursorColorRaw = state.string("cursorColor") ?? state.string("cursor-color"),
           let cursorColor = NativeComponentColorStyle.parseColor(cursorColorRaw) {
            if let editor = textField.currentEditor() as? NSTextView {
                editor.insertionPointColor = cursorColor
            }
        }

        let disabled = state.bool("disabled", default: false)
        textField.isEnabled = !disabled

        maxLength = state.int("maxlength", default: -1)
        confirmHold = state.bool("confirmHold", default: state.bool("confirm-hold", default: false))
        holdKeyboard = state.bool("holdKeyboard", default: state.bool("hold-keyboard", default: false))
        confirmType = (state.string("confirmType") ?? state.string("confirm-type") ?? "done").lowercased()

        let type = (state.string("type") ?? "text").lowercased()
        let password = state.bool("password", default: false) || type == "password" || type == "safe-password"
        semanticType = password ? .password : Self.semanticType(for: type)
        if password != secureMode {
            switchInputField(secure: password)
        }
        sanitizeCurrentValueIfNeeded()

        if let selectionStart = Self.optionalInt(from: state["selectionStart"] ?? state["selection-start"]) {
            let end = Self.optionalInt(from: state["selectionEnd"] ?? state["selection-end"]) ?? selectionStart
            if selectionStart >= 0 && end >= selectionStart {
                setSelection(start: selectionStart, end: end)
            }
        } else if let cursor = Self.optionalInt(from: state["cursor"]), cursor >= 0 {
            let location = min(cursor, textField.stringValue.utf16.count)
            setSelection(start: location, end: location)
        }

        if let cornerRadius = state.double("cornerRadius") {
            applyCornerRadius(cornerRadius)
        }

        let shouldFocus = state.bool("focus", default: false) || state.bool("autoFocus", default: state.bool("auto-focus", default: false))
        if isMounted {
            if shouldFocus && !lastFocusPropValue {
                focus()
            } else if !shouldFocus && lastFocusPropValue {
                blur()
            }
        }
        lastFocusPropValue = shouldFocus
    }

    func setFrame(_ frame: CGRect) {
        view.frame = frame
        textField.frame = view.bounds
    }

    func focus() {
        view.window?.makeFirstResponder(textField)
    }

    func blur() {
        guard let window = view.window else { return }
        if textField.currentEditor() != nil || window.firstResponder === textField {
            window.makeFirstResponder(nil)
        }
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
        textField.delegate = nil
        view.removeFromSuperview()
    }

    func controlTextDidBeginEditing(_ obj: Notification) {
        applyPendingSelectionIfNeeded()
        if let editor = textField.currentEditor() as? NSTextView {
            let color = textField.textColor ?? .labelColor
            editor.textColor = color
            editor.insertionPointColor = color
        }
        let value = currentTextValue()
        lastFocusedValue = value
        emit("focus", detail: ["value": value, "cursor": currentCursor()])
    }

    func controlTextDidChange(_ obj: Notification) {
        guard !suppressInputEvent else { return }
        var value = currentTextValue()

        // Sanitize for number/digit types. Handles IME committed text and paste.
        let sanitized = Self.sanitizedValue(value, for: semanticType)
        if sanitized != value {
            suppressInputEvent = true
            if let editor = textField.currentEditor() as? NSTextView {
                editor.string = sanitized
            }
            textField.stringValue = sanitized
            suppressInputEvent = false
            setSelection(start: sanitized.count, end: sanitized.count)
            value = sanitized
        }

        if maxLength >= 0 && value.count > maxLength {
            let clamped = String(value.prefix(maxLength))
            suppressInputEvent = true
            if let editor = textField.currentEditor() as? NSTextView {
                editor.string = clamped
            }
            textField.stringValue = clamped
            suppressInputEvent = false
            setSelection(start: clamped.count, end: clamped.count)
            value = clamped
        }
        emit("input", detail: detailWithValue(value))
    }

    func controlTextDidEndEditing(_ obj: Notification) {
        let value = currentTextValue()
        emit("blur", detail: ["value": value, "cursor": currentCursor()])
        if value != lastFocusedValue {
            emit("change", detail: detailWithValue(value))
        }
    }

    func control(
        _ control: NSControl,
        textView: NSTextView,
        doCommandBy commandSelector: Selector
    ) -> Bool {
        guard commandSelector == #selector(NSResponder.insertNewline(_:)) else { return false }
        if confirmType == "return" {
            return false
        }
        emit("confirm", detail: detailWithValue(currentTextValue()))
        if shouldBlurAfterConfirm() {
            blur()
        }
        return true
    }

    private func shouldBlurAfterConfirm() -> Bool {
        return !confirmHold && !holdKeyboard
    }

    func control(
        _ control: NSControl,
        textView: NSTextView,
        shouldChangeCharactersIn range: NSRange,
        replacementString string: String?
    ) -> Bool {
        guard let replacement = string else { return true }
        if replacement.isEmpty {
            return true
        }
        let current = currentTextValue()
        guard let textRange = Range(range, in: current) else { return true }
        let updated = current.replacingCharacters(in: textRange, with: replacement)
        if maxLength >= 0 && updated.count > maxLength {
            return false
        }
        return Self.sanitizedValue(updated, for: semanticType) == updated
    }

    private func configureTextField(_ field: NSTextField) {
        field.frame = view.bounds
        field.autoresizingMask = [.width, .height]
        field.isBordered = false
        field.isBezeled = false
        field.drawsBackground = false
        field.isEditable = true
        field.isSelectable = true
        field.textColor = .labelColor
        field.focusRingType = .none
        field.delegate = self
        field.lineBreakMode = .byTruncatingTail
        field.usesSingleLineMode = true
        field.maximumNumberOfLines = 1
    }

    private func switchInputField(secure: Bool) {
        let previous = textField
        let previousValue = previous.stringValue
        let previousPlaceholder = previous.placeholderString
        let previousEnabled = previous.isEnabled
        let wasEditing = previous.currentEditor() != nil
        if let editor = previous.currentEditor() {
            pendingSelection = editor.selectedRange
        }

        let next: NSTextField = secure
            ? MacNativeSecureTextField(frame: view.bounds)
            : MacNativeTextField(frame: view.bounds)
        configureTextField(next)
        next.stringValue = previousValue
        next.placeholderString = previousPlaceholder
        next.isEnabled = previousEnabled
        next.textColor = previous.textColor

        previous.delegate = nil
        previous.removeFromSuperview()
        view.addSubview(next)

        textField = next
        secureMode = secure

        if wasEditing {
            focus()
        }
    }

    private func sanitizeCurrentValueIfNeeded() {
        let current = currentTextValue()
        let sanitized = Self.sanitizedValue(current, for: semanticType)
        if current == sanitized { return }
        suppressInputEvent = true
        if let editor = textField.currentEditor() as? NSTextView {
            editor.string = sanitized
        }
        textField.stringValue = sanitized
        suppressInputEvent = false
        setSelection(start: sanitized.utf16.count, end: sanitized.utf16.count)
    }

    private func applyPlaceholderStyle(_ rawStyle: String?) {
        let placeholder = textField.placeholderString ?? ""
        guard !placeholder.isEmpty else {
            textField.placeholderAttributedString = nil
            return
        }
        guard let style = rawStyle?.trimmingCharacters(in: .whitespacesAndNewlines),
              !style.isEmpty else {
            textField.placeholderAttributedString = nil
            textField.placeholderString = placeholder
            return
        }
        let colorExpr = NativeComponentColorStyle.extractColorFromStyle(style) ?? style
        guard let color = NativeComponentColorStyle.parseColor(colorExpr) else {
            textField.placeholderAttributedString = nil
            textField.placeholderString = placeholder
            return
        }
        textField.placeholderAttributedString = NSAttributedString(
            string: placeholder,
            attributes: [.foregroundColor: color]
        )
    }

    private func setSelection(start: Int, end: Int) {
        let length = textField.stringValue.utf16.count
        let lower = max(0, min(start, end))
        let upper = max(lower, max(start, end))
        let clampedLower = min(lower, length)
        let clampedUpper = min(upper, length)
        let range = NSRange(location: clampedLower, length: clampedUpper - clampedLower)
        if let editor = textField.currentEditor() {
            editor.selectedRange = range
        } else {
            pendingSelection = range
        }
    }

    private func applyPendingSelectionIfNeeded() {
        guard let range = pendingSelection else { return }
        pendingSelection = nil
        if let editor = textField.currentEditor() {
            editor.selectedRange = range
        }
    }

    private func currentCursor() -> Int {
        if let editor = textField.currentEditor() {
            return max(0, editor.selectedRange.location)
        }
        return currentTextValue().utf16.count
    }

    private func currentTextValue() -> String {
        if let editor = textField.currentEditor() as? NSTextView {
            return editor.string
        }
        return textField.stringValue
    }

    private func detailWithValue(_ value: String) -> [String: Any] {
        ["value": value, "cursor": currentCursor()]
    }

    private func emit(_ event: String, detail: [String: Any]) {
        eventSink(["event": event, "detail": detail])
    }

    private func applyCornerRadius(_ radius: Double) {
        view.wantsLayer = true
        view.layer?.cornerRadius = CGFloat(max(0, radius))
        view.layer?.masksToBounds = true
        // Also clip the textField's own layer so the field-editor cursor
        // (rendered in a separate sublayer) is clipped at the same boundary.
        textField.wantsLayer = true
        textField.layer?.masksToBounds = true
    }

    private static func optionalInt(from value: Any?) -> Int? {
        guard let value else { return nil }
        if value is NSNull { return nil }
        if let intValue = value as? Int { return intValue }
        if let number = value as? NSNumber { return number.intValue }
        if let string = value as? String {
            let normalized = string.trimmingCharacters(in: .whitespacesAndNewlines)
            if normalized.isEmpty { return nil }
            return Int(normalized)
        }
        return nil
    }

    private static func semanticType(for rawType: String) -> InputSemanticType {
        switch rawType {
        case "number": return .number
        case "digit": return .digit
        case "password", "safe-password": return .password
        default: return .text
        }
    }

    private static func sanitizedValue(_ value: String, for type: InputSemanticType) -> String {
        switch type {
        case .number:
            return value.filter { isAsciiDigit($0) }
        case .digit:
            var seenDot = false
            var result = String()
            result.reserveCapacity(value.count)
            for char in value {
                if isAsciiDigit(char) {
                    result.append(char)
                } else if char == ".", !seenDot {
                    seenDot = true
                    result.append(char)
                }
            }
            return result
        case .text, .password:
            return value
        }
    }

    private static func isAsciiDigit(_ char: Character) -> Bool {
        guard let scalar = char.unicodeScalars.first, char.unicodeScalars.count == 1 else {
            return false
        }
        return scalar.value >= 48 && scalar.value <= 57
    }
}

private enum InputSemanticType {
    case text, number, digit, password
}

// MARK: - Vertically centred text field cell

/// Centres text / placeholder / field-editor vertically within the cell bounds.
private final class VerticalCenteredTextFieldCell: NSTextFieldCell {
    private func centeredRect(forBounds rect: NSRect) -> NSRect {
        let original = super.drawingRect(forBounds: rect)
        let hInset: CGFloat = 4
        let ix = original.origin.x + hInset
        let iw = max(0, original.width - 2 * hInset)
        let textHeight = cellSize(forBounds: rect).height
        guard textHeight < original.height else {
            return NSRect(x: ix, y: original.origin.y, width: iw, height: original.height)
        }
        let yOffset = floor((original.height - textHeight) / 2)
        return NSRect(x: ix, y: original.origin.y + yOffset, width: iw, height: original.height - yOffset)
    }

    override func drawingRect(forBounds rect: NSRect) -> NSRect {
        centeredRect(forBounds: rect)
    }

    override func select(
        withFrame rect: NSRect, in controlView: NSView,
        editor textObj: NSText, delegate anObject: Any?,
        start selStart: Int, length selLength: Int
    ) {
        super.select(withFrame: centeredRect(forBounds: rect), in: controlView,
                     editor: textObj, delegate: anObject, start: selStart, length: selLength)
    }

    override func edit(
        withFrame rect: NSRect, in controlView: NSView,
        editor textObj: NSText, delegate anObject: Any?, event: NSEvent?
    ) {
        super.edit(withFrame: centeredRect(forBounds: rect), in: controlView,
                   editor: textObj, delegate: anObject, event: event)
    }
}

private final class VerticalCenteredSecureTextFieldCell: NSSecureTextFieldCell {
    private func centeredRect(forBounds rect: NSRect) -> NSRect {
        let original = super.drawingRect(forBounds: rect)
        let hInset: CGFloat = 4
        let ix = original.origin.x + hInset
        let iw = max(0, original.width - 2 * hInset)
        let textHeight = cellSize(forBounds: rect).height
        guard textHeight < original.height else {
            return NSRect(x: ix, y: original.origin.y, width: iw, height: original.height)
        }
        let yOffset = floor((original.height - textHeight) / 2)
        return NSRect(x: ix, y: original.origin.y + yOffset, width: iw, height: original.height - yOffset)
    }

    override func drawingRect(forBounds rect: NSRect) -> NSRect {
        centeredRect(forBounds: rect)
    }

    override func select(
        withFrame rect: NSRect, in controlView: NSView,
        editor textObj: NSText, delegate anObject: Any?,
        start selStart: Int, length selLength: Int
    ) {
        super.select(withFrame: centeredRect(forBounds: rect), in: controlView,
                     editor: textObj, delegate: anObject, start: selStart, length: selLength)
    }

    override func edit(
        withFrame rect: NSRect, in controlView: NSView,
        editor textObj: NSText, delegate anObject: Any?, event: NSEvent?
    ) {
        super.edit(withFrame: centeredRect(forBounds: rect), in: controlView,
                   editor: textObj, delegate: anObject, event: event)
    }
}

// MARK: - Private view / field subclasses

private final class MacInputContainerView: NSView {
    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}

private final class MacNativeTextField: NSTextField {
    override class var cellClass: AnyClass? {
        get { VerticalCenteredTextFieldCell.self }
        set {}
    }

    override var acceptsFirstResponder: Bool { true }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        super.mouseDown(with: event)
    }
}

private final class MacNativeSecureTextField: NSSecureTextField {
    override class var cellClass: AnyClass? {
        get { VerticalCenteredSecureTextFieldCell.self }
        set {}
    }

    override var acceptsFirstResponder: Bool { true }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        super.mouseDown(with: event)
    }
}

// MARK: - Dictionary helpers

private extension Dictionary where Key == String, Value == Any {
    func string(_ key: String) -> String? { self[key] as? String }

    func bool(_ key: String, default defaultValue: Bool) -> Bool {
        guard let value = self[key] else { return defaultValue }
        if let b = value as? Bool { return b }
        if let n = value as? NSNumber { return n.boolValue }
        if let s = value as? String {
            let v = s.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if v == "true" || v == "1" { return true }
            if v == "false" || v == "0" { return false }
        }
        return defaultValue
    }

    func int(_ key: String, default defaultValue: Int) -> Int {
        guard let value = self[key] else { return defaultValue }
        if let i = value as? Int { return i }
        if let n = value as? NSNumber { return n.intValue }
        if let s = value as? String, let i = Int(s) { return i }
        return defaultValue
    }

    func double(_ key: String) -> Double? {
        guard let value = self[key] else { return nil }
        if let d = value as? Double { return d }
        if let f = value as? Float { return Double(f) }
        if let i = value as? Int { return Double(i) }
        if let n = value as? NSNumber { return n.doubleValue }
        if let s = value as? String, let d = Double(s) { return d }
        return nil
    }
}

#endif
