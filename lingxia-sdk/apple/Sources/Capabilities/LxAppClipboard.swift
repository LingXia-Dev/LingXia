import Foundation

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

enum LxAppClipboard {
    @MainActor
    static func write(kind: String, payload: String) -> String {
        switch kind {
        case "text":
            setText(payload)
            return ok()
        case "image":
            guard setImage(from: payload) else {
                return fail(1002, "clipboard image filePath is not a readable image")
            }
            return ok()
        default:
            return fail(1002, "unknown clipboard type")
        }
    }

    /// The OS may gate a read behind a paste prompt (iOS 16+, macOS 15.4+).
    /// The pasteboard never says "declined": the payload accessor simply
    /// returns nil. The type peek is prompt-free, so "a type is present but its
    /// payload came back nil" is how a dismissed prompt is observed; a
    /// standing "never allow" choice on macOS is a permission error instead.
    @MainActor
    static func read(kind: String, imageOutputPath: String) -> String {
        if accessAlwaysDenied() {
            return fail(3008, "clipboard permission denied")
        }
        let wantText = kind.isEmpty || kind == "text"
        let wantImage = kind.isEmpty || kind == "image"
        var text: String?
        var imagePath: String?
        if wantText {
            text = currentText()
            if text == nil, hasText() {
                return canceled()
            }
        }
        if wantImage, let dest = imageOutputPath.isEmpty ? nil : imageOutputPath {
            imagePath = writeCurrentImage(to: dest)
            if imagePath == nil, hasImage(), !imageEncodingFailed {
                return canceled()
            }
        }
        return encodeRead(text: text, imagePath: imagePath)
    }

    /// Set by `writeCurrentImage` when the payload was readable but could not
    /// be encoded or written, so that case is not mistaken for a dismissal.
    @MainActor
    private static var imageEncodingFailed = false

    private static func accessAlwaysDenied() -> Bool {
#if os(macOS)
        if #available(macOS 15.4, *) {
            return NSPasteboard.general.accessBehavior == .alwaysDeny
        }
#endif
        return false
    }

    @MainActor
    static func clear() -> String {
        clearContents()
        return ok()
    }

    @MainActor
    static func types() -> String {
        var types: [String] = []
        if hasText() {
            types.append("text")
        }
        if hasImage() {
            types.append("image")
        }
        return encodeTypes(types)
    }

#if os(iOS)
    private static func setText(_ text: String) {
        UIPasteboard.general.string = text
    }

    private static func currentText() -> String? {
        UIPasteboard.general.string
    }

    private static func setImage(from path: String) -> Bool {
        guard let image = UIImage(contentsOfFile: path) else { return false }
        UIPasteboard.general.image = image
        return true
    }

    @MainActor
    private static func writeCurrentImage(to dest: String) -> String? {
        imageEncodingFailed = false
        guard let image = UIPasteboard.general.image else { return nil }
        guard let data = image.pngData() else {
            imageEncodingFailed = true
            return nil
        }
        return writeData(data, to: dest)
    }

    private static func clearContents() {
        UIPasteboard.general.items = []
    }

    private static func hasText() -> Bool {
        UIPasteboard.general.hasStrings
    }

    private static func hasImage() -> Bool {
        UIPasteboard.general.hasImages
    }
#elseif os(macOS)
    private static func setText(_ text: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    private static func currentText() -> String? {
        NSPasteboard.general.string(forType: .string)
    }

    private static func setImage(from path: String) -> Bool {
        guard let image = NSImage(contentsOfFile: path) else { return false }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        return pasteboard.writeObjects([image])
    }

    @MainActor
    private static func writeCurrentImage(to dest: String) -> String? {
        imageEncodingFailed = false
        let pasteboard = NSPasteboard.general
        if let data = pasteboard.data(forType: .png) {
            return writeData(data, to: dest)
        }
        guard let image = NSImage(pasteboard: pasteboard) else { return nil }
        guard let tiff = image.tiffRepresentation,
              let rep = NSBitmapImageRep(data: tiff),
              let data = rep.representation(using: .png, properties: [:]) else {
            imageEncodingFailed = true
            return nil
        }
        return writeData(data, to: dest)
    }

    private static func clearContents() {
        NSPasteboard.general.clearContents()
    }

    private static func hasText() -> Bool {
        NSPasteboard.general.canReadItem(withDataConformingToTypes: [NSPasteboard.PasteboardType.string.rawValue])
    }

    private static func hasImage() -> Bool {
        NSPasteboard.general.canReadItem(withDataConformingToTypes: [
            NSPasteboard.PasteboardType.png.rawValue,
            NSPasteboard.PasteboardType.tiff.rawValue,
        ])
    }
#endif

    @MainActor
    private static func writeData(_ data: Data, to dest: String) -> String? {
        let url = URL(fileURLWithPath: dest)
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try data.write(to: url, options: .atomic)
            return dest
        } catch {
            LXLog.error("clipboard image write failed", category: "Clipboard", error: error)
            imageEncodingFailed = true
            return nil
        }
    }

    private static func ok() -> String {
        #"{"ok":true}"#
    }

    private static func canceled() -> String {
        #"{"ok":true,"canceled":true}"#
    }

    private static func fail(_ code: UInt32, _ detail: String) -> String {
        "{\"ok\":false,\"error\":\(code),\"detail\":\(jsonString(detail))}"
    }

    private static func encodeRead(text: String?, imagePath: String?) -> String {
        var parts = ["\"ok\":true", "\"canceled\":false"]
        if let text {
            parts.append("\"text\":\(jsonString(text))")
        }
        if let imagePath {
            parts.append("\"imagePath\":\(jsonString(imagePath))")
            parts.append("\"imageMime\":\"image/png\"")
        }
        return "{\(parts.joined(separator: ","))}"
    }

    private static func encodeTypes(_ types: [String]) -> String {
        let items = types.map { "\"\($0)\"" }.joined(separator: ",")
        return "{\"ok\":true,\"canceled\":false,\"types\":[\(items)]}"
    }

    private static func jsonString(_ value: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
        return data.flatMap { String(data: $0, encoding: .utf8) } ?? "\"\""
    }
}
