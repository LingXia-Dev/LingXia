#if os(macOS)
import AppKit
import Quartz
import CLingXiaRustAPI
import os.log

// MARK: - Entry point

extension LxAppMedia {
    /// Shared controller that manages the Quick Look preview panel.
    @MainActor static var qlController: MacQuickLookController?
    /// In-flight http(s) fetch that has not yet presented QuickLook.
    /// Either this or `qlController` is set, never both.
    @MainActor fileprivate static var pendingRemoteFetch: PendingRemoteFetch?

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
        endPendingRemoteFetch()
    }

    /// Completes an in-flight fetch as `interrupted` and stops its download.
    @MainActor
    fileprivate static func endPendingRemoteFetch() {
        guard let pending = pendingRemoteFetch else {
            return
        }
        pendingRemoteFetch = nil
        pending.task?.cancel()
        emitPreviewResult(callbackId: pending.callbackId, reason: .interrupted, lastIndex: 0)
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
            LXLog.error("Failed to encode preview result for callback \(callbackId)", category: "MediaPreview")
            return
        }
        let _ = onCallback(callbackId, true, json)
    }

    struct PreviewMediaPayload: Codable {
        let path: String
        let media_type: Int32
        let duration_ms: UInt64?
    }

    struct PreviewMediaRequestPayload: Codable {
        let sources: [PreviewMediaPayload]
        let startIndex: Int?
        let advance: String?
    }

    nonisolated static func previewMedia(items_json: RustStr, callback_id: UInt64, presented_callback_id: UInt64, change_callback_id: UInt64) -> Bool {
        let itemsJson = items_json.toString()

        guard let jsonData = itemsJson.data(using: .utf8) else {
            LXLog.error("Failed to convert items JSON to data", category: "MediaPreview")
            return false
        }

        let request: PreviewMediaRequestPayload
        do {
            request = try JSONDecoder().decode(PreviewMediaRequestPayload.self, from: jsonData)
        } catch {
            LXLog.error("Failed to decode items JSON", category: "MediaPreview", error: error)
            return false
        }
        guard !request.sources.isEmpty else {
            LXLog.error("previewMedia called with empty items", category: "MediaPreview")
            return false
        }

        if Thread.isMainThread {
            return MainActor.assumeIsolated {
                previewMediaOnMain(request: request, callbackId: callback_id, presentedCallbackId: presented_callback_id, changeCallbackId: change_callback_id)
            }
        }
        var started = false
        DispatchQueue.main.sync {
            started = previewMediaOnMain(request: request, callbackId: callback_id, presentedCallbackId: presented_callback_id, changeCallbackId: change_callback_id)
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
    private static func previewMediaOnMain(request: PreviewMediaRequestPayload, callbackId: UInt64, presentedCallbackId: UInt64, changeCallbackId: UInt64) -> Bool {
        let urls = request.sources.map { payload -> URL in
            let raw = payload.path.trimmingCharacters(in: .whitespacesAndNewlines)
            if let parsed = URL(string: raw), isRemoteHTTPURL(parsed) {
                return parsed
            }
            if let parsed = URL(string: raw), parsed.scheme != nil {
                return parsed
            }
            return URL(fileURLWithPath: raw)
        }
        guard !urls.isEmpty else {
            LXLog.error("previewMedia called with no valid URLs", category: "MediaPreview")
            return false
        }

        supersedeCurrentPreview()

        let startIndex = request.startIndex ?? 0
        if !urls.contains(where: isRemoteHTTPURL) {
            return showQuickLook(
                urls: urls,
                sourceIndexes: Array(urls.indices),
                startIndex: startIndex,
                callbackId: callbackId,
                presentedCallbackId: presentedCallbackId,
                changeCallbackId: changeCallbackId,
                cacheDirectory: nil
            )
        }

        // QuickLook only previews file URLs. Fetch http(s) items to a temp
        // directory, then hand the local copies to the panel.
        let fetch = PendingRemoteFetch(callbackId: callbackId)
        pendingRemoteFetch = fetch
        let fetchId = fetch.id
        fetch.task = Task.detached(priority: .userInitiated) {
            let materialized = await materializeRemotePreviewURLs(urls)
            await MainActor.run {
                guard pendingRemoteFetch?.id == fetchId else {
                    if let cacheDirectory = materialized.cacheDirectory {
                        try? FileManager.default.removeItem(at: cacheDirectory)
                    }
                    return
                }
                pendingRemoteFetch = nil
                guard let localURLs = materialized.urls else {
                    LXLog.error("previewMedia failed to download remote items", category: "MediaPreview")
                    emitPreviewResult(callbackId: callbackId, reason: .error, lastIndex: 0)
                    return
                }
                let shown = showQuickLook(
                    urls: localURLs,
                    sourceIndexes: materialized.sourceIndexes,
                    startIndex: startIndex,
                    callbackId: callbackId,
                    presentedCallbackId: presentedCallbackId,
                    changeCallbackId: changeCallbackId,
                    cacheDirectory: materialized.cacheDirectory
                )
                if !shown {
                    if let cacheDirectory = materialized.cacheDirectory {
                        try? FileManager.default.removeItem(at: cacheDirectory)
                    }
                    emitPreviewResult(callbackId: callbackId, reason: .error, lastIndex: 0)
                }
            }
        }
        return true
    }

    /// Close an on-screen panel and complete any in-flight remote fetch.
    @MainActor
    private static func supersedeCurrentPreview() {
        LxAppFile.closeQLController()
        qlController?.finish(reason: .interrupted)
        endPendingRemoteFetch()
    }

    @MainActor
    private static func showQuickLook(
        urls: [URL],
        sourceIndexes: [Int],
        startIndex: Int,
        callbackId: UInt64,
        presentedCallbackId: UInt64,
        changeCallbackId: UInt64,
        cacheDirectory: URL?
    ) -> Bool {
        // A requested start that could not be fetched opens on the next item
        // that could, or the last one before it.
        let shownStart = sourceIndexes.firstIndex { $0 >= startIndex }
            ?? max(sourceIndexes.count - 1, 0)
        let controller = MacQuickLookController(
            urls: urls,
            sourceIndexes: sourceIndexes,
            startIndex: shownStart,
            callbackId: callbackId,
            changeCallbackId: changeCallbackId,
            cacheDirectory: cacheDirectory
        )
        guard controller.show() else {
            return false
        }
        qlController = controller
        if presentedCallbackId != 0 {
            let _ = onCallback(presentedCallbackId, true, "{}")
        }
        return true
    }

    @MainActor
    private static func cancelPreviewOnMain(callbackId: UInt64) -> Bool {
        if pendingRemoteFetch?.callbackId == callbackId {
            endPendingRemoteFetch()
            return true
        }
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

/// One in-flight remote fetch. Identity is `id`, so a superseded fetch's
/// completion cannot present QuickLook or emit a second result. `task` is the
/// download itself: without cancelling it, an aborted preview keeps pulling
/// the whole file down.
@MainActor
fileprivate final class PendingRemoteFetch {
    let id = UUID()
    let callbackId: UInt64
    var task: Task<Void, Never>?

    init(callbackId: UInt64) {
        self.callbackId = callbackId
    }
}

private func isRemoteHTTPURL(_ url: URL) -> Bool {
    guard let scheme = url.scheme?.lowercased() else { return false }
    return scheme == "http" || scheme == "https"
}

private struct MaterializedPreviewURLs: Sendable {
    let urls: [URL]?
    /// Request index of each entry in `urls`; failed items leave gaps.
    let sourceIndexes: [Int]
    let cacheDirectory: URL?
}

/// QuickLook picks its renderer from the file extension, so the served type
/// wins over the URL's own: a CDN path like `/image.php?id=1` would otherwise
/// be saved as `.php` and refuse to preview even though the response said JPEG.
private func previewFileExtension(for url: URL, response: URLResponse?) -> String {
    if let fromMimeType = previewExtensionForMimeType(response?.mimeType) {
        return fromMimeType
    }
    let fromPath = url.pathExtension
    return fromPath.isEmpty ? "dat" : fromPath
}

private func previewExtensionForMimeType(_ mimeType: String?) -> String? {
    switch mimeType?.lowercased() {
    case "image/jpeg", "image/jpg":
        return "jpg"
    case "image/png":
        return "png"
    case "image/gif":
        return "gif"
    case "image/webp":
        return "webp"
    case "image/heic", "image/heif":
        return "heic"
    case "video/mp4":
        return "mp4"
    case "video/quicktime":
        return "mov"
    case "video/webm":
        return "webm"
    default:
        return nil
    }
}

/// Fetches every remote item into `cacheDirectory`. One unreachable item does
/// not discard the sequence — the other hosts skip a failed item and keep the
/// session, so a 404 in the middle of a gallery must not blank the whole panel.
/// A failed item is left out entirely (QuickLook cannot show an http URL);
/// `sourceIndexes` keeps what JS sees tied to the request's own indexes.
/// Only a sequence where nothing could be fetched fails outright.
private func materializeRemotePreviewURLs(_ urls: [URL]) async -> MaterializedPreviewURLs {
    if !urls.contains(where: isRemoteHTTPURL) {
        return MaterializedPreviewURLs(urls: urls, sourceIndexes: Array(urls.indices), cacheDirectory: nil)
    }

    let cacheDirectory = FileManager.default.temporaryDirectory
        .appendingPathComponent("lingxia-preview-\(UUID().uuidString)", isDirectory: true)
    guard (try? FileManager.default.createDirectory(
        at: cacheDirectory,
        withIntermediateDirectories: true
    )) != nil else {
        return MaterializedPreviewURLs(urls: nil, sourceIndexes: [], cacheDirectory: nil)
    }

    var shown: [URL] = []
    var sourceIndexes: [Int] = []
    var fetched = 0
    for index in urls.indices {
        guard isRemoteHTTPURL(urls[index]) else {
            shown.append(urls[index])
            sourceIndexes.append(index)
            continue
        }
        if Task.isCancelled {
            break
        }
        guard let local = await downloadPreviewItem(urls[index], into: cacheDirectory, index: index)
        else {
            LXLog.error("previewMedia could not fetch remote item \(index)", category: "MediaPreview")
            continue
        }
        shown.append(local)
        sourceIndexes.append(index)
        fetched += 1
    }
    guard fetched > 0 else {
        try? FileManager.default.removeItem(at: cacheDirectory)
        return MaterializedPreviewURLs(urls: nil, sourceIndexes: [], cacheDirectory: nil)
    }
    return MaterializedPreviewURLs(urls: shown, sourceIndexes: sourceIndexes, cacheDirectory: cacheDirectory)
}

private func downloadPreviewItem(_ source: URL, into cacheDirectory: URL, index: Int) async -> URL? {
    do {
        let (tempURL, response) = try await URLSession.shared.download(from: source)
        if let http = response as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
            // The async download API leaves the body for the caller to remove.
            try? FileManager.default.removeItem(at: tempURL)
            return nil
        }
        let destination = cacheDirectory.appendingPathComponent(
            "item-\(index).\(previewFileExtension(for: source, response: response))"
        )
        if FileManager.default.fileExists(atPath: destination.path) {
            try FileManager.default.removeItem(at: destination)
        }
        try FileManager.default.moveItem(at: tempURL, to: destination)
        return destination
    } catch {
        return nil
    }
}

// MARK: - Quick Look controller

/// Bridges QLPreviewPanel data source/delegate to show native Quick Look previews.
@MainActor
final class MacQuickLookController: NSObject, @preconcurrency QLPreviewPanelDataSource, @preconcurrency QLPreviewPanelDelegate {
    private let items: [QLPreviewURL]
    /// Request index of each item, reported to JS in place of the panel's own.
    private let sourceIndexes: [Int]
    private let startIndex: Int
    let callbackId: UInt64
    /// JS-side change-stream callback id; fired with `{"index": N}` whenever
    /// the displayed item changes (including the initial item). Zero disables.
    private let changeCallbackId: UInt64
    private let cacheDirectory: URL?
    private var lastNotifiedIndex: Int = -1
    private var closeObserver: NSObjectProtocol?
    private var indexObservation: NSKeyValueObservation?
    private var didFinish = false

    init(
        urls: [URL],
        sourceIndexes: [Int],
        startIndex: Int,
        callbackId: UInt64,
        changeCallbackId: UInt64,
        cacheDirectory: URL?
    ) {
        self.items = urls.map { QLPreviewURL(url: $0) }
        self.sourceIndexes = sourceIndexes
        self.startIndex = startIndex
        self.callbackId = callbackId
        self.changeCallbackId = changeCallbackId
        self.cacheDirectory = cacheDirectory
        super.init()
    }

    func show() -> Bool {
        guard let panel = QLPreviewPanel.shared() else {
            LXLog.error("Failed to acquire QLPreviewPanel", category: "MediaPreview")
            return false
        }
        panel.dataSource = self
        panel.delegate = self
        installCloseObserver(for: panel)
        panel.reloadData()
        panel.currentPreviewItemIndex = normalizedIndex(startIndex)
        installIndexObserver(for: panel)
        panel.makeKeyAndOrderFront(nil)
        return true
    }

    /// Observe QuickLook's own page navigation. `.initial` also fires for the
    /// item displayed at open, so the JS change stream always sees item 0..n
    /// transitions without a separate initial hook.
    private func installIndexObserver(for panel: QLPreviewPanel) {
        guard changeCallbackId != 0 else { return }
        indexObservation = panel.observe(\.currentPreviewItemIndex, options: [.initial, .new]) { [weak self] panel, _ in
            DispatchQueue.main.async {
                guard let self, !self.didFinish else { return }
                let index = self.sourceIndex(self.normalizedIndex(panel.currentPreviewItemIndex))
                guard index != self.lastNotifiedIndex else { return }
                self.lastNotifiedIndex = index
                let _ = onCallback(self.changeCallbackId, true, "{\"index\":\(index)}")
            }
        }
    }

    fileprivate func finish(reason: PreviewMediaCloseReason, shouldClosePanel: Bool = true) {
        guard !didFinish else {
            return
        }
        didFinish = true

        let panel = QLPreviewPanel.shared()
        let lastIndex = sourceIndex(currentIndex(from: panel))
        removeCloseObserver()
        indexObservation?.invalidate()
        indexObservation = nil
        panel?.delegate = nil
        panel?.dataSource = nil

        LxAppMedia.clearQLController(self)
        if shouldClosePanel {
            panel?.orderOut(nil)
        }
        if let cacheDirectory {
            try? FileManager.default.removeItem(at: cacheDirectory)
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

    private func sourceIndex(_ shownIndex: Int) -> Int {
        sourceIndexes.indices.contains(shownIndex) ? sourceIndexes[shownIndex] : shownIndex
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
