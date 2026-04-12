#if os(macOS)
import AppKit
import Quartz
import CLingXiaRustAPI
import os.log

// MARK: - Entry point

extension LxAppMedia {
    nonisolated fileprivate static let previewLog = OSLog(subsystem: "LingXia", category: "MediaPreview")

    /// Shared controller that manages the Quick Look preview panel.
    @MainActor static var qlController: MacQuickLookController?

    @MainActor
    static func clearQLController(_ controller: MacQuickLookController? = nil) {
        guard controller == nil || qlController === controller else {
            return
        }
        qlController = nil
    }

    @MainActor
    static func closeQLController() {
        qlController?.finish(reason: .interrupted)
    }

    @MainActor
    fileprivate static func emitPreviewResult(callbackId: UInt64, reason: PreviewMediaCloseReason, lastIndex: Int) {
        guard let data = try? JSONSerialization.data(
            withJSONObject: [
                "reason": reason.rawValue,
                "lastIndex": max(lastIndex, 0)
            ],
            options: []
        ), let json = String(data: data, encoding: .utf8) else {
            os_log(.error, log: previewLog, "Failed to encode preview result for callback %{public}llu", callbackId)
            return
        }
        let _ = onCallback(callbackId, true, json)
    }

    struct PreviewMediaPayload: Codable {
        let path: String
        let media_type: Int32
        let cover_path: String?
        let duration_ms: UInt64?
    }

    struct PreviewMediaRequestPayload: Codable {
        let sources: [PreviewMediaPayload]
        let startIndex: Int?
        let advance: String?
    }

    nonisolated static func previewMedia(items_json: RustStr, callback_id: UInt64) -> Bool {
        let itemsJson = items_json.toString()

        guard let jsonData = itemsJson.data(using: .utf8) else {
            os_log(.error, log: previewLog, "Failed to convert items JSON to data")
            return false
        }

        let request: PreviewMediaRequestPayload
        do {
            request = try JSONDecoder().decode(PreviewMediaRequestPayload.self, from: jsonData)
        } catch {
            os_log(.error, log: previewLog, "Failed to decode items JSON: %{public}@", error.localizedDescription)
            return false
        }
        guard !request.sources.isEmpty else {
            os_log(.error, log: previewLog, "previewMedia called with empty items")
            return false
        }

        if Thread.isMainThread {
            return MainActor.assumeIsolated {
                previewMediaOnMain(request: request, callbackId: callback_id)
            }
        }
        var started = false
        DispatchQueue.main.sync {
            started = previewMediaOnMain(request: request, callbackId: callback_id)
        }
        return started
    }

    nonisolated static func cancelPreview(callback_id: UInt64) -> Bool {
        if Thread.isMainThread {
            return MainActor.assumeIsolated {
                cancelPreviewOnMain(callbackId: callback_id)
            }
        }
        var cancelled = false
        DispatchQueue.main.sync {
            cancelled = cancelPreviewOnMain(callbackId: callback_id)
        }
        return cancelled
    }

    @MainActor
    private static func previewMediaOnMain(request: PreviewMediaRequestPayload, callbackId: UInt64) -> Bool {
        let urls = request.sources.map { payload -> URL in
            if let parsed = URL(string: payload.path), parsed.scheme != nil {
                return parsed
            }
            return URL(fileURLWithPath: payload.path)
        }
        guard !urls.isEmpty else {
            os_log(.error, log: previewLog, "previewMedia called with no valid URLs")
            return false
        }
        return showQuickLook(urls: urls, startIndex: request.startIndex ?? 0, callbackId: callbackId)
    }

    @MainActor
    private static func showQuickLook(urls: [URL], startIndex: Int, callbackId: UInt64) -> Bool {
        LxAppFile.closeQLController()
        qlController?.finish(reason: .interrupted)

        let controller = MacQuickLookController(urls: urls, startIndex: startIndex, callbackId: callbackId)
        guard controller.show() else {
            return false
        }
        qlController = controller
        return true
    }

    @MainActor
    private static func cancelPreviewOnMain(callbackId: UInt64) -> Bool {
        guard let controller = qlController, controller.callbackId == callbackId else {
            return false
        }
        controller.finish(reason: .interrupted)
        return true
    }
}

fileprivate enum PreviewMediaCloseReason: String {
    case manual
    case completed
    case interrupted
    case error
}

// MARK: - Quick Look controller

/// Bridges QLPreviewPanel data source/delegate to show native Quick Look previews.
@MainActor
final class MacQuickLookController: NSObject, @preconcurrency QLPreviewPanelDataSource, @preconcurrency QLPreviewPanelDelegate {
    private let items: [QLPreviewURL]
    private let startIndex: Int
    let callbackId: UInt64
    private var closeObserver: NSObjectProtocol?
    private var didFinish = false

    init(urls: [URL], startIndex: Int, callbackId: UInt64) {
        self.items = urls.map { QLPreviewURL(url: $0) }
        self.startIndex = startIndex
        self.callbackId = callbackId
        super.init()
    }

    func show() -> Bool {
        guard let panel = QLPreviewPanel.shared() else {
            os_log(.error, log: LxAppMedia.previewLog, "Failed to acquire QLPreviewPanel")
            return false
        }
        panel.dataSource = self
        panel.delegate = self
        installCloseObserver(for: panel)
        panel.reloadData()
        panel.currentPreviewItemIndex = normalizedIndex(startIndex)
        panel.makeKeyAndOrderFront(nil)
        return true
    }

    fileprivate func finish(reason: PreviewMediaCloseReason, shouldClosePanel: Bool = true) {
        guard !didFinish else {
            return
        }
        didFinish = true

        let panel = QLPreviewPanel.shared()
        let lastIndex = currentIndex(from: panel)
        removeCloseObserver()
        panel?.delegate = nil
        panel?.dataSource = nil

        LxAppMedia.clearQLController(self)
        if shouldClosePanel {
            panel?.orderOut(nil)
        }
        LxAppMedia.emitPreviewResult(callbackId: callbackId, reason: reason, lastIndex: lastIndex)
    }

    private func installCloseObserver(for panel: QLPreviewPanel) {
        removeCloseObserver()
        closeObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: panel,
            queue: nil
        ) { [weak self] _ in
            DispatchQueue.main.async {
                self?.finish(reason: .manual, shouldClosePanel: false)
            }
        }
    }

    private func removeCloseObserver() {
        guard let closeObserver else {
            return
        }
        NotificationCenter.default.removeObserver(closeObserver)
        self.closeObserver = nil
    }

    private func normalizedIndex(_ index: Int) -> Int {
        guard !items.isEmpty else {
            return 0
        }
        return min(max(index, 0), items.count - 1)
    }

    private func currentIndex(from panel: QLPreviewPanel?) -> Int {
        guard !items.isEmpty else {
            return 0
        }
        let current = panel?.currentPreviewItemIndex ?? startIndex
        return normalizedIndex(current)
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
