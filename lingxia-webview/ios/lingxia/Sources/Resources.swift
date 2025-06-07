import Foundation
import CLingXiaFFI
import os.log

// Re-export the main rust functions for easier access
@_exported import CLingXiaFFI

private let resourceLogger = OSLog(subsystem: "LingXia", category: "Resources")

/// Get the resource bundle for the package
/// - Returns: The bundle containing resources, or nil if not found
private func getResourceBundle() -> Bundle? {
    let mainBundle = Bundle.main

    // Look for the SPM resource bundle first
    if let bundlePath = mainBundle.path(forResource: "miniapp_miniapp", ofType: "bundle"),
       let resourceBundle = Bundle(path: bundlePath) {
        return resourceBundle
    }

    // Fallback to main bundle
    return mainBundle
}

/// Read asset data from the bundle resources
/// - Parameter path: The relative path to the asset within the bundle
/// - Returns: The asset data as bytes, or empty array if not found
public func read_asset_data(path: String) -> [UInt8] {
    guard let bundle = getResourceBundle() else {
        os_log("Failed to get resource bundle", log: resourceLogger, type: .error)
        return []
    }

    // Handle different path formats
    let cleanPath = path.hasPrefix("/") ? String(path.dropFirst()) : path
    let components = cleanPath.components(separatedBy: "/")

    guard !components.isEmpty else {
        os_log("Invalid path: %{public}@", log: resourceLogger, type: .error, path)
        return []
    }

    let filename = components.last!
    let pathExtension = (filename as NSString).pathExtension
    let nameWithoutExtension = (filename as NSString).deletingPathExtension

    // Build the subdirectory path if exists
    let subdirectory = components.count > 1 ? components.dropLast().joined(separator: "/") : nil

    // Try to find the resource
    guard let resourceURL = bundle.url(
        forResource: nameWithoutExtension,
        withExtension: pathExtension.isEmpty ? nil : pathExtension,
        subdirectory: subdirectory
    ) else {
        os_log("Resource not found: %{public}@", log: resourceLogger, type: .error, path)
        return []
    }

    do {
        let data = try Data(contentsOf: resourceURL)
        return Array(data)
    } catch {
        os_log("Failed to read asset data for %{public}@: %{public}@", log: resourceLogger, type: .error, path, error.localizedDescription)
        return []
    }
}

/// List contents of an asset directory
/// - Parameter dirPath: The directory path within the bundle
/// - Returns: Array of file/directory names in the directory
public func list_asset_directory(dirPath: String) -> [String] {
    guard let bundle = getResourceBundle() else {
        os_log("Failed to get resource bundle", log: resourceLogger, type: .error)
        return []
    }

    // Handle root directory case
    let cleanPath = dirPath.hasPrefix("/") ? String(dirPath.dropFirst()) : dirPath
    let directoryPath = cleanPath.isEmpty ? nil : cleanPath

    // Get the bundle's resource path
    guard let bundleResourcePath = bundle.resourcePath else {
        os_log("Bundle has no resource path", log: resourceLogger, type: .error)
        return []
    }

    let fullPath = directoryPath.map { "\(bundleResourcePath)/\($0)" } ?? bundleResourcePath

    do {
        let fileManager = FileManager.default
        let contents = try fileManager.contentsOfDirectory(atPath: fullPath)

        // Filter out hidden files
        return contents.filter { !$0.hasPrefix(".") }
    } catch {
        os_log("Failed to list directory %{public}@: %{public}@", log: resourceLogger, type: .error, dirPath, error.localizedDescription)
        return []
    }
}
