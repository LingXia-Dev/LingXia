#if os(macOS)
import AppKit
import Quartz
import CLingXiaRustAPI
import os.log

// MARK: - Entry point

extension LxAppMedia {
    nonisolated(unsafe) private static let previewLog = OSLog(subsystem: "LingXia", category: "MediaPreview")

    /// Shared controller that manages the Quick Look preview panel.
    @MainActor static var qlController: MacQuickLookController?

    @MainActor
    static func clearQLController() {
        qlController = nil
    }

    struct PreviewMediaPayload: Codable {
        let path: String
        let media_type: Int32
        let cover_path: String?
    }

    nonisolated static func previewMedia(items_json: RustStr) -> Bool {
        let itemsJson = items_json.toString()

        guard let jsonData = itemsJson.data(using: .utf8) else {
            os_log(.error, log: previewLog, "Failed to convert items JSON to data")
            return false
        }

        let payloads: [PreviewMediaPayload]
        do {
            payloads = try JSONDecoder().decode([PreviewMediaPayload].self, from: jsonData)
        } catch {
            os_log(.error, log: previewLog, "Failed to decode items JSON: %{public}@", error.localizedDescription)
            return false
        }
        guard !payloads.isEmpty else {
            os_log(.error, log: previewLog, "previewMedia called with empty items")
            return false
        }

        DispatchQueue.main.async {
            let urls = payloads.compactMap { URL(fileURLWithPath: $0.path) }
            guard !urls.isEmpty else { return }
            showQuickLook(urls: urls)
        }
        return true
    }

    @MainActor
    private static func showQuickLook(urls: [URL]) {
        let controller = MacQuickLookController(urls: urls)
        self.qlController = controller
        controller.show()
    }
}

// MARK: - Quick Look controller

/// Bridges QLPreviewPanel data source/delegate to show native Quick Look previews.
@MainActor
final class MacQuickLookController: NSObject, @preconcurrency QLPreviewPanelDataSource, @preconcurrency QLPreviewPanelDelegate {
    private let items: [QLPreviewURL]

    init(urls: [URL]) {
        self.items = urls.map { QLPreviewURL(url: $0) }
        super.init()
    }

    func show() {
        let panel = QLPreviewPanel.shared()!
        panel.dataSource = self
        panel.delegate = self
        panel.reloadData()
        panel.makeKeyAndOrderFront(nil)
    }

    // MARK: - QLPreviewPanelDataSource

    func numberOfPreviewItems(in panel: QLPreviewPanel!) -> Int {
        items.count
    }

    func previewPanel(_ panel: QLPreviewPanel!, previewItemAt index: Int) -> (any QLPreviewItem)! {
        items[index]
    }

    // MARK: - QLPreviewPanelDelegate

    func previewPanel(_ panel: QLPreviewPanel!, handle event: NSEvent!) -> Bool {
        false
    }
}

// MARK: - QLPreviewItem wrapper

private final class QLPreviewURL: NSObject, QLPreviewItem {
    let previewItemURL: URL?
    let previewItemTitle: String?

    init(url: URL) {
        self.previewItemURL = url
        self.previewItemTitle = url.lastPathComponent
        super.init()
    }
}

#endif
