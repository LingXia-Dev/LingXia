#if os(iOS)
import Foundation
import CLingXiaRustAPI

enum LxAppMediaStorage {

    private static func timestamp() -> Int64 {
        Int64(Date().timeIntervalSince1970 * 1000)
    }

    static func cacheDirectory() -> URL? {
        let cachePath: String = {
            let current = getCurrentLxApp()
            let appId = current.appid.toString()
            let info = getLxAppInfo(appId)
            return info.cache_dir.toString()
        }()

        let baseURL: URL
        if cachePath.isEmpty {
            guard let fallback = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else {
                return nil
            }
            baseURL = fallback
        } else {
            baseURL = URL(fileURLWithPath: cachePath, isDirectory: true)
        }

        do {
            try FileManager.default.createDirectory(at: baseURL, withIntermediateDirectories: true)
        } catch {
            return nil
        }

        return baseURL
    }

    static func makeFileURL(prefix: String, preferredExtension ext: String) -> URL? {
        guard let base = cacheDirectory() else { return nil }
        let sanitizedExt = ext.isEmpty ? "" : ".\(ext)"
        return base.appendingPathComponent("\(prefix)_\(timestamp())\(sanitizedExt)")
    }

    static func write(data: Data, prefix: String, fileExtension: String) throws -> URL {
        guard let url = makeFileURL(prefix: prefix, preferredExtension: fileExtension) else {
            throw NSError(domain: "LxAppMediaStorage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Cache directory unavailable"])
        }
        try data.write(to: url, options: .atomic)
        return url
    }

    static func copy(from sourceURL: URL, prefix: String, fallbackExtension: String, requiresSecurityScope: Bool) throws -> URL {
        guard let destinationURL = makeFileURL(
            prefix: prefix,
            preferredExtension: sourceURL.pathExtension.isEmpty ? fallbackExtension : sourceURL.pathExtension
        ) else {
            throw NSError(domain: "LxAppMediaStorage", code: 2, userInfo: [NSLocalizedDescriptionKey: "Cache directory unavailable"])
        }

        let accessed = requiresSecurityScope ? sourceURL.startAccessingSecurityScopedResource() : false
        defer {
            if requiresSecurityScope && accessed {
                sourceURL.stopAccessingSecurityScopedResource()
            }
        }

        if FileManager.default.fileExists(atPath: destinationURL.path) {
            try FileManager.default.removeItem(at: destinationURL)
        }

        try FileManager.default.copyItem(at: sourceURL, to: destinationURL)
        return destinationURL
    }
}
#endif
