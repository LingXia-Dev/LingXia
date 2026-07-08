import Foundation

struct LxAppDirectoryConfig {
    let dataPath: String
    let cachesPath: String
}

enum LxAppDirectoryError: Error {
    case bundleIdentifierNotFound
    case systemDirectoryNotFound(FileManager.SearchPathDirectory)
}

struct LxAppDirectoryFactory {

    private static func resolveBundleIdentifier() -> String {
        if let bundleId = Bundle.main.bundleIdentifier, !bundleId.isEmpty {
            return bundleId
        }

        if let infoBundleId = Bundle.main.object(forInfoDictionaryKey: "CFBundleIdentifier") as? String,
           !infoBundleId.isEmpty
        {
            return infoBundleId
        }

        let processName = ProcessInfo.processInfo.processName
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if !processName.isEmpty {
            return "com.lingxia.\(processName.lowercased())"
        }

        return "com.lingxia.app"
    }

    /// Create platform-specific directory configuration
    static func createDirectoryConfig() -> LxAppDirectoryConfig {
        do {
            let bundleId = resolveBundleIdentifier()

            #if os(iOS)
            let dataDirectory: FileManager.SearchPathDirectory = .documentDirectory
            #elseif os(macOS)
            let dataDirectory: FileManager.SearchPathDirectory = .applicationSupportDirectory
            #endif

            guard let dataURL = FileManager.default.urls(for: dataDirectory, in: .userDomainMask).first,
                  let cacheURL = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else {
                throw LxAppDirectoryError.systemDirectoryNotFound(dataDirectory)
            }

            // The Runner can host several lxapp projects at once, one process per
            // project (`lingxia dev` in each dir). They share a bundle id, so
            // without isolation they'd share the on-disk metadata DB / caches and
            // the second process's bootstrap would fail. `LINGXIA_RUNNER_INSTANCE`
            // (set by `lingxia dev`, stable per project) nests each instance in its
            // own subtree. Absent → unchanged behavior for normal host apps.
            var dataBase = dataURL.appendingPathComponent(bundleId)
            var cacheBase = cacheURL.appendingPathComponent(bundleId)
            if let instance = ProcessInfo.processInfo.environment["LINGXIA_RUNNER_INSTANCE"],
               !instance.isEmpty {
                dataBase = dataBase.appendingPathComponent("instances").appendingPathComponent(instance)
                cacheBase = cacheBase.appendingPathComponent("instances").appendingPathComponent(instance)
            }
            let dataPath = dataBase.path
            let cachePath = cacheBase.path

            // Create directories if they don't exist
            try FileManager.default.createDirectory(atPath: dataPath, withIntermediateDirectories: true, attributes: nil)
            try FileManager.default.createDirectory(atPath: cachePath, withIntermediateDirectories: true, attributes: nil)

            return LxAppDirectoryConfig(dataPath: dataPath, cachesPath: cachePath)
        } catch {
            fatalError("Failed to create directory config: \(error)")
        }
    }
}
