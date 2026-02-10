#if os(macOS)
import AppKit
import UniformTypeIdentifiers
import CLingXiaSwiftAPI
import CLingXiaRustAPI

extension LxAppMedia {
    nonisolated static func chooseMedia(
        max_count: UInt32,
        mode: RustStr,
        source_types_json: RustStr,
        camera_facing: RustStr,
        max_duration: RustStr,
        callback_id: UInt64
    ) -> Bool {
        let modeStr = mode.toString().lowercased()
        let sourceTypesJson = source_types_json.toString()
        let _ = (camera_facing, max_duration)

        DispatchQueue.main.async {
            guard let sourceTypesData = sourceTypesJson.data(using: .utf8),
                  let sourceTypes = try? JSONDecoder().decode([String].self, from: sourceTypesData) else {
                let _ = onCallback(callback_id, false, "1002")
                return
            }

            let allowAlbum = sourceTypes.contains("album")
            let allowCamera = sourceTypes.contains("camera")

            if allowCamera && !allowAlbum {
                let _ = onCallback(callback_id, false, "6001")
                sendDone(callback_id)
                return
            }

            guard allowAlbum else {
                let _ = onCallback(callback_id, false, "1002")
                return
            }

            let selectionLimit = modeStr == "video" ? 1 : max(1, Int(max_count))
            presentOpenPanel(
                mode: modeStr,
                selectionLimit: selectionLimit,
                callbackId: callback_id
            )
        }

        return true
    }

    nonisolated private static func sendDone(_ callbackId: UInt64) {
        let _ = onCallback(callbackId, true, "{\"done\":true}")
    }

    @MainActor
    private static func presentOpenPanel(mode: String, selectionLimit: Int, callbackId: UInt64) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = selectionLimit > 1
        panel.resolvesAliases = true
        panel.title = "Choose Media"
        panel.prompt = "Choose"

        configureOpenPanelTypes(panel, mode: mode)

        guard panel.runModal() == .OK else {
            let _ = onCallback(callbackId, false, "2000")
            sendDone(callbackId)
            return
        }

        let urls = Array(panel.urls.prefix(selectionLimit))
        guard !urls.isEmpty else {
            let _ = onCallback(callbackId, false, "2000")
            sendDone(callbackId)
            return
        }

        let items = urls.compactMap { createMediaItem(url: $0, mode: mode) }
        guard !items.isEmpty,
              let data = try? JSONSerialization.data(withJSONObject: items, options: []),
              let json = String(data: data, encoding: .utf8) else {
            let _ = onCallback(callbackId, false, "1000")
            sendDone(callbackId)
            return
        }

        let _ = onCallback(callbackId, true, json)
        sendDone(callbackId)
    }

    @MainActor
    private static func configureOpenPanelTypes(_ panel: NSOpenPanel, mode: String) {
        switch mode {
        case "image":
            panel.allowedContentTypes = [.image]
        case "video":
            panel.allowedContentTypes = [.movie]
        default:
            panel.allowedContentTypes = [.image, .movie]
        }
    }

    private static func createMediaItem(url: URL, mode: String) -> [String: Any]? {
        guard url.isFileURL else { return nil }
        guard let fileType = detectFileType(url: url, preferredMode: mode) else { return nil }
        return [
            "uri": url.path,
            "fileType": fileType,
            "isOriginal": true
        ]
    }

    private static func detectFileType(url: URL, preferredMode: String) -> String? {
        if preferredMode == "image" { return "image" }
        if preferredMode == "video" { return "video" }

        if #available(macOS 11.0, *) {
            let ext = url.pathExtension.lowercased()
            if let type = UTType(filenameExtension: ext) {
                if type.conforms(to: .image) { return "image" }
                if type.conforms(to: .movie) || type.conforms(to: .audiovisualContent) { return "video" }
            }
        }

        return nil
    }
}
#endif
