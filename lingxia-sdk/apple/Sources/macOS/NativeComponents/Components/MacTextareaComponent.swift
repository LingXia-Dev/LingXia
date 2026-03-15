#if os(macOS)
import Foundation
import AppKit

@MainActor
final class MacTextareaComponentFactory: MacNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> MacNativeComponent {
        MacTextareaComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class MacTextareaComponent: NSObject, MacNativeComponent, NSTextViewDelegate {
    let id: String
    let view: NSView

    private let scrollView: NSScrollView
    private let textView: NSTextView
    private let placeholderLabel: PassThroughTextField
    private let eventSink: ([String: Any]) -> Void
    private var suppressInputEvent = false
    private var maxLength: Int = -1
    private var confirmHold = false
    private var holdKeyboard = false
    private var confirmType = "return"
    private var autoHeight = false
    private var lastLineCount = 1
    private var lastContentHeight: CGFloat = 0
    private var pendingSelection: NSRange?
    private var resolvedProps: [String: Any] = [:]
    private var isMounted = false
    private var lastFocusPropValue = false
    private var lastFocusedValue = ""

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.eventSink = eventSink
        self.view = MacTextareaContainerView(frame: .zero)
        self.scrollView = NSScrollView(frame: .zero)
        self.textView = MacNativeTextView(frame: .zero)
        self.placeholderLabel = PassThroughTextField(labelWithString: "")
        self.resolvedProps = initialProps
        super.init()

        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.clear.cgColor

        configureTextView()
        configureScrollView()
        configurePlaceholder()

        view.addSubview(scrollView)
        view.addSubview(placeholderLabel)

        update(props: initialProps)
    }

    func mount(in host: NSView) {
        if view.superview !== host {
            host.addSubview(view)
        }
        layoutSubviews()
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

        if let value = state.string("value"), value != textView.string {
            suppressInputEvent = true
            textView.string = value
            suppressInputEvent = false
            refreshTextLayout()
            updatePlaceholderVisibility()
            emitLinechangeIfNeeded()
        }

        if let placeholder = state.string("placeholder") {
            placeholderLabel.stringValue = placeholder
        }
        let placeholderStyle = state.string("placeholderStyle") ?? state.string("placeholder-style")
        applyPlaceholderStyle(placeholderStyle)

        if let textColorRaw = state.string("textColor"), let textColor = NativeComponentColorStyle.parseColor(textColorRaw) {
            textView.textColor = textColor
            textView.insertionPointColor = textColor
        } else {
            textView.textColor = .labelColor
            textView.insertionPointColor = .labelColor
        }

        if let cursorColorRaw = state.string("cursorColor") ?? state.string("cursor-color"),
           let cursorColor = NativeComponentColorStyle.parseColor(cursorColorRaw) {
            textView.insertionPointColor = cursorColor
        }

        let disabled = state.bool("disabled", default: false)
        textView.isEditable = !disabled
        textView.isSelectable = !disabled

        maxLength = state.int("maxlength", default: -1)
        confirmHold = state.bool("confirmHold", default: state.bool("confirm-hold", default: false))
        holdKeyboard = state.bool("holdKeyboard", default: state.bool("hold-keyboard", default: false))
        confirmType = (state.string("confirmType") ?? state.string("confirm-type") ?? "return").lowercased()
        autoHeight = state.bool("autoHeight", default: state.bool("auto-height", default: false))
        applyScrollBehavior()

        if let selectionStart = Self.optionalInt(from: state["selectionStart"] ?? state["selection-start"]) {
            let end = Self.optionalInt(from: state["selectionEnd"] ?? state["selection-end"]) ?? selectionStart
            if selectionStart >= 0 && end >= selectionStart {
                setSelection(start: selectionStart, end: end)
            }
        } else if let cursor = Self.optionalInt(from: state["cursor"]), cursor >= 0 {
            let location = min(cursor, textView.string.utf16.count)
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

        refreshTextLayout()
        updatePlaceholderVisibility()
        emitLinechangeIfNeeded()
    }

    func setFrame(_ frame: CGRect) {
        view.frame = frame
        layoutSubviews()
    }

    func focus() {
        view.window?.makeFirstResponder(textView)
    }

    func blur() {
        guard let window = view.window, window.firstResponder === textView else { return }
        window.makeFirstResponder(nil)
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
        textView.delegate = nil
        view.removeFromSuperview()
    }

    func textDidBeginEditing(_ notification: Notification) {
        lastFocusedValue = textView.string
        applyPendingSelectionIfNeeded()
        emit("focus", detail: currentDetail())
    }

    func textDidEndEditing(_ notification: Notification) {
        let detail = currentDetail()
        emit("blur", detail: detail)
        if (detail["value"] as? String ?? "") != lastFocusedValue {
            emit("change", detail: detail)
        }
    }

    func textDidChange(_ notification: Notification) {
        guard !suppressInputEvent else { return }

        var value = textView.string
        if maxLength >= 0 && value.count > maxLength {
            let clamped = String(value.prefix(maxLength))
            suppressInputEvent = true
            textView.string = clamped
            suppressInputEvent = false
            value = clamped
            setSelection(start: clamped.count, end: clamped.count)
        }

        updatePlaceholderVisibility()
        refreshTextLayout()
        if autoHeight {
            textView.scrollRangeToVisible(textView.selectedRange())
        }
        emit("input", detail: currentDetail())
        emitLinechangeIfNeeded()
    }

    func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        guard commandSelector == #selector(NSResponder.insertNewline(_:)) else { return false }
        if confirmType == "return" {
            return false
        }
        emit("confirm", detail: currentDetail())
        if shouldBlurAfterConfirm() {
            blur()
        }
        return true
    }

    private func shouldBlurAfterConfirm() -> Bool {
        return !confirmHold && !holdKeyboard
    }

    private func configureTextView() {
        textView.delegate = self
        textView.drawsBackground = false
        textView.isEditable = true
        textView.isSelectable = true
        textView.textColor = .labelColor
        textView.insertionPointColor = .labelColor
        textView.isRichText = false
        textView.importsGraphics = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isAutomaticDataDetectionEnabled = false
        textView.textContainerInset = NSSize(width: 4, height: 4)
        textView.textContainer?.lineFragmentPadding = 0
        textView.minSize = .zero
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.heightTracksTextView = false
        textView.textContainer?.containerSize = NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude)
    }

    private func configureScrollView() {
        scrollView.frame = view.bounds
        scrollView.autoresizingMask = [.width, .height]
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.documentView = textView
    }

    private func configurePlaceholder() {
        placeholderLabel.textColor = NSColor.placeholderTextColor
        placeholderLabel.backgroundColor = .clear
        placeholderLabel.isBezeled = false
        placeholderLabel.isEditable = false
        placeholderLabel.lineBreakMode = .byTruncatingTail
    }

    private func layoutSubviews() {
        scrollView.frame = view.bounds
        refreshTextLayout()
        placeholderLabel.frame = NSRect(x: 4, y: 4, width: max(0, view.bounds.width - 8), height: 18)
    }

    private func updatePlaceholderVisibility() {
        placeholderLabel.isHidden = !textView.string.isEmpty
    }

    private func applyPlaceholderStyle(_ rawStyle: String?) {
        guard let style = rawStyle?.trimmingCharacters(in: .whitespacesAndNewlines),
              !style.isEmpty else {
            placeholderLabel.textColor = NSColor.placeholderTextColor
            return
        }
        let colorExpr = NativeComponentColorStyle.extractColorFromStyle(style) ?? style
        if let color = NativeComponentColorStyle.parseColor(colorExpr) {
            placeholderLabel.textColor = color
        } else {
            placeholderLabel.textColor = NSColor.placeholderTextColor
        }
    }

    private func setSelection(start: Int, end: Int) {
        let length = textView.string.utf16.count
        let lower = max(0, min(start, end))
        let upper = max(lower, max(start, end))
        let clampedLower = min(lower, length)
        let clampedUpper = min(upper, length)
        let range = NSRange(location: clampedLower, length: clampedUpper - clampedLower)
        if view.window?.firstResponder as AnyObject? === textView {
            textView.setSelectedRange(range)
        } else {
            pendingSelection = range
        }
    }

    private func applyPendingSelectionIfNeeded() {
        guard let range = pendingSelection else { return }
        pendingSelection = nil
        textView.setSelectedRange(range)
    }

    private func currentDetail() -> [String: Any] {
        let range = textView.selectedRange()
        let start = max(0, range.location)
        let end = max(start, start + range.length)
        return [
            "value": textView.string,
            "cursor": start,
            "selectionStart": start,
            "selectionEnd": end
        ]
    }

    private func emitLinechangeIfNeeded() {
        let count = lineCount(for: textView.string)
        let height = measuredContentHeight()
        let heightChanged = abs(height - lastContentHeight) >= 0.5
        if count == lastLineCount && !heightChanged { return }
        lastLineCount = count
        lastContentHeight = height
        emit("linechange", detail: [
            "lineCount": count,
            "height": height,
            "heightRpx": height
        ])
    }

    private func lineCount(for text: String) -> Int {
        let width = max(textView.bounds.width, 1)
        if width <= 1 {
            return max(1, text.split(separator: "\n", omittingEmptySubsequences: false).count)
        }
        let font = textView.font ?? NSFont.systemFont(ofSize: NSFont.systemFontSize)
        let content = text.isEmpty ? " " : text
        let bounds = (content as NSString).boundingRect(
            with: NSSize(width: width, height: CGFloat.greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: font]
        )
        let lineHeight = max(font.boundingRectForFont.height, 1)
        return max(1, Int(ceil(bounds.height / lineHeight)))
    }

    private func measuredContentHeight() -> CGFloat {
        guard let textContainer = textView.textContainer,
              let layoutManager = textView.layoutManager else {
            return max(ceil(textView.bounds.height), 1)
        }
        layoutManager.ensureLayout(for: textContainer)
        let usedRect = layoutManager.usedRect(for: textContainer)
        let insets = textView.textContainerInset.height * 2
        let minLine = textView.font?.boundingRectForFont.height ?? 16
        return max(ceil(usedRect.height + insets), ceil(minLine))
    }

    private func refreshTextLayout() {
        let width = max(scrollView.bounds.width, 0)
        textView.textContainer?.containerSize = NSSize(width: width, height: CGFloat.greatestFiniteMagnitude)
        textView.frame = NSRect(x: 0, y: 0, width: width, height: max(scrollView.bounds.height, 1))
        let contentHeight = measuredContentHeight()
        let targetHeight = max(scrollView.bounds.height, contentHeight)
        textView.frame = NSRect(x: 0, y: 0, width: width, height: targetHeight)
    }

    private func applyScrollBehavior() {
        // isVerticallyResizable must stay true in all modes so NSTextView lays out
        // text properly and remains interactive. For autoHeight we just hide the
        // scroller; the JS side is responsible for resizing the component frame.
        textView.isVerticallyResizable = true
        if autoHeight {
            scrollView.hasVerticalScroller = false
        } else {
            scrollView.hasVerticalScroller = true
        }
    }

    private func emit(_ event: String, detail: [String: Any]) {
        eventSink([
            "event": event,
            "detail": detail
        ])
    }

    private func applyCornerRadius(_ radius: Double) {
        let r = CGFloat(max(0, radius))
        view.wantsLayer = true
        view.layer?.cornerRadius = r
        view.layer?.masksToBounds = true
        // The cursor is drawn inside NSClipView (scrollView.contentView).
        // Applying the same cornerRadius + masksToBounds there ensures the cursor
        // is clipped before it reaches the parent layer, fixing bleed at corners.
        scrollView.wantsLayer = true
        scrollView.layer?.masksToBounds = true
        scrollView.contentView.wantsLayer = true
        scrollView.contentView.layer?.cornerRadius = r
        scrollView.contentView.layer?.masksToBounds = true
    }

    private static func int(from value: Any?, default defaultValue: Int) -> Int {
        if let intValue = value as? Int { return intValue }
        if let number = value as? NSNumber { return number.intValue }
        if let string = value as? String, let intValue = Int(string) { return intValue }
        return defaultValue
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

}

/// A non-interactive label used for placeholder text.
/// Returning nil from hitTest lets clicks pass straight through to the NSTextView below.
private final class PassThroughTextField: NSTextField {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool { false }
}

private final class MacTextareaContainerView: NSView {
    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}

private final class MacNativeTextView: NSTextView {
    override var acceptsFirstResponder: Bool { true }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        super.mouseDown(with: event)
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

    func double(_ key: String) -> Double? {
        guard let value = self[key] else { return nil }
        if let doubleValue = value as? Double { return doubleValue }
        if let floatValue = value as? Float { return Double(floatValue) }
        if let intValue = value as? Int { return Double(intValue) }
        if let number = value as? NSNumber { return number.doubleValue }
        if let string = value as? String, let doubleValue = Double(string) { return doubleValue }
        return nil
    }
}

#endif
