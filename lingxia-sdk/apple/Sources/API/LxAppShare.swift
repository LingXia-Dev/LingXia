import Foundation
import CLingXiaRustAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

enum LxAppShare {
    @discardableResult
    @MainActor
    static func share(
        title: String,
        text: String,
        url: String,
        filesJson: String,
        callbackId: UInt64
    ) -> Bool {
        let files: [String]
        do {
            let data = Data(filesJson.utf8)
            files = try JSONDecoder().decode([String].self, from: data)
        } catch {
            let _ = onCallback(callbackId, false, "1002")
            return false
        }

        var items: [Any] = []
        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedText = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedURL = url.trimmingCharacters(in: .whitespacesAndNewlines)
        let shareURL = URL(string: trimmedURL).flatMap { parsedURL in
            parsedURL.scheme == nil ? nil : parsedURL
        }
        let hasFiles = !files.isEmpty
        #if os(iOS)
        let shouldShareText = !hasFiles && shareURL == nil
        #else
        let shouldShareText = !hasFiles
        #endif
        if shouldShareText, let shareText = combinedText(title: trimmedTitle, text: trimmedText) {
            #if os(iOS)
            items.append(shareText as NSString)
            #else
            items.append(shareText)
            #endif
        }
        if let shareURL {
            items.append(shareURL)
        }
        for path in files {
            let parsedURL = URL(string: path)
            let fileURL: URL
            if let parsedURL, parsedURL.isFileURL {
                fileURL = parsedURL
            } else {
                fileURL = URL(fileURLWithPath: path)
            }
            let isReadableFile = LxAppFile.withSecurityScopedAccess(path: fileURL.path) {
                var isDirectory: ObjCBool = false
                let exists = FileManager.default.fileExists(atPath: fileURL.path, isDirectory: &isDirectory)
                return exists && !isDirectory.boolValue
            }
            guard isReadableFile else {
                let _ = onCallback(callbackId, false, "1000")
                return false
            }
            items.append(fileURL)
        }

        guard !items.isEmpty else {
            let _ = onCallback(callbackId, false, "1002")
            return false
        }

        #if os(iOS)
        return shareIOS(items: items, callbackId: callbackId)
        #elseif os(macOS)
        return shareMacOS(items: items, callbackId: callbackId)
        #else
        let _ = onCallback(callbackId, false, "1000")
        return false
        #endif
    }

    #if os(iOS)
    @MainActor
    private static func shareIOS(items: [Any], callbackId: UInt64) -> Bool {
        guard let presenter = LxApp.topViewController() else {
            let _ = onCallback(callbackId, false, "1000")
            return false
        }
        let controller = UIActivityViewController(activityItems: items, applicationActivities: nil)
        if let popover = controller.popoverPresentationController {
            popover.sourceView = presenter.view
            popover.sourceRect = CGRect(
                x: presenter.view.bounds.midX,
                y: presenter.view.bounds.midY,
                width: 1,
                height: 1
            )
            popover.permittedArrowDirections = []
        }
        controller.completionWithItemsHandler = { _, completed, _, error in
            if error != nil {
                let _ = onCallback(callbackId, false, "1000")
                return
            }
            let json = completed ? "{\"completed\":true}" : "{\"completed\":false}"
            let _ = onCallback(callbackId, true, json)
        }
        presenter.present(controller, animated: true)
        return true
    }
    #endif

    #if os(macOS)
    @MainActor
    private static func shareMacOS(items: [Any], callbackId: UInt64) -> Bool {
        guard let contentView = NSApp.keyWindow?.contentView ?? NSApp.mainWindow?.contentView else {
            let _ = onCallback(callbackId, false, "1000")
            return false
        }
        let picker = NSSharingServicePicker(items: items)
        let rect = NSRect(
            x: contentView.bounds.midX,
            y: contentView.bounds.midY,
            width: 1,
            height: 1
        )
        picker.show(relativeTo: rect, of: contentView, preferredEdge: .minY)
        let _ = onCallback(callbackId, true, "{}")
        return true
    }
    #endif

    private static func combinedText(title: String, text: String) -> String? {
        switch (title.isEmpty, text.isEmpty) {
        case (true, true):
            return nil
        case (false, true):
            return title
        case (true, false):
            return text
        case (false, false):
            return "\(title)\n\(text)"
        }
    }
}
