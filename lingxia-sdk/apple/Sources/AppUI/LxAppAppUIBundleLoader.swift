import Foundation

enum LxAppAppUIBundleLoader {
    static func loadFromMainBundle() throws -> LxAppGeneratedBundleConfig {
        let resourceDirectoryURL = try findGeneratedConfigResourceDirectory()
        let appURL = resourceDirectoryURL.appendingPathComponent("app.json")
        let uiURL = try findPreferredResource(
            names: ["macos-ui.json", "ui.json"],
            in: resourceDirectoryURL
        )

        let decoder = JSONDecoder()
        let appData = try Data(contentsOf: appURL)
        let uiData = try Data(contentsOf: uiURL)

        do {
            let app = try decoder.decode(LxAppGeneratedAppConfig.self, from: appData)
            let ui = try decoder.decode(LxAppUIConfig.self, from: uiData)
            return LxAppGeneratedBundleConfig(app: app, ui: ui, appURL: appURL, uiURL: uiURL)
        } catch {
            throw LxAppUIError.invalidConfig("failed to decode generated bundle config: \(error)")
        }
    }

    static func resolveRelativeResource(
        _ relativePath: String,
        baseURL: URL
    ) -> URL? {
        guard !relativePath.isEmpty else { return nil }

        let candidate = baseURL.deletingLastPathComponent().appendingPathComponent(relativePath)
        var isDirectory: ObjCBool = false
        if FileManager.default.fileExists(atPath: candidate.path, isDirectory: &isDirectory),
           !isDirectory.boolValue {
            return candidate
        }

        return nil
    }

    private static func findGeneratedConfigResourceDirectory() throws -> URL {
        let fileManager = FileManager.default
        for directoryURL in candidateResourceDirectories() {
            let appURL = directoryURL.appendingPathComponent("app.json")
            let hasAppConfig = fileManager.fileExists(atPath: appURL.path)
            let hasUIConfig = ["macos-ui.json", "ui.json"].contains { name in
                fileManager.fileExists(atPath: directoryURL.appendingPathComponent(name).path)
            }
            if hasAppConfig && hasUIConfig {
                return directoryURL
            }
        }

        throw LxAppUIError.missingResource("app.json plus macos-ui.json or ui.json")
    }

    private static func findPreferredResource(names: [String], in directoryURL: URL) throws -> URL {
        for name in names {
            let url = directoryURL.appendingPathComponent(name)
            if FileManager.default.fileExists(atPath: url.path) {
                return url
            }
        }
        throw LxAppUIError.missingResource(names.joined(separator: " or "))
    }

    private static func candidateResourceDirectories() -> [URL] {
        guard let rootURL = Bundle.main.resourceURL else { return [] }

        var directories = [rootURL]
        let childURLs = (try? FileManager.default.contentsOfDirectory(
            at: rootURL,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        )) ?? []

        for childURL in childURLs.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
            guard childURL.pathExtension == "bundle",
                  let bundle = Bundle(url: childURL),
                  let resourceURL = bundle.resourceURL else {
                continue
            }
            directories.append(resourceURL)
        }

        return directories
    }
}
