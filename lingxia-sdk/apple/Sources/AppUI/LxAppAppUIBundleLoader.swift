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

    /// The CLI merges the host's resources into the main bundle at build time.
    private static func findGeneratedConfigResourceDirectory() throws -> URL {
        guard let rootURL = Bundle.main.resourceURL,
              FileManager.default.fileExists(atPath: rootURL.appendingPathComponent("app.json").path)
        else {
            throw LxAppUIError.missingResource("app.json")
        }
        return rootURL
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
}
