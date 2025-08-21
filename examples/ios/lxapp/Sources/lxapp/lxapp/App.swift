import SwiftUI
import UIKit
import lingxia
import Foundation
import os.log

// Global log for test installation
private let testInstallLog = OSLog(subsystem: "LingXia", category: "TestInstall")

public struct ContentView: View {
    // Use a global flag instead of @State to avoid SwiftUI update cycle issues
    private static var hasInitialized = false

    public var body: some View {
        Color.clear
            .onAppear {
                if !Self.hasInitialized {
                    Self.hasInitialized = true
                    LxApp.initialize()
                    // Enable WebView debugging
                    LxApp.enableWebViewDebugging()

                    // Install test lxapp from Bundle Resources
                    let _ = installTestLxApp()

                    LxApp.openHomeLxApp()
                }
            }
    }
}

/**
 * Install test lxapp for openLxApp functionality testing
 * This copies homelxapp from Bundle Resources to create a test lxapp
 * Returns true if successful, false otherwise
 */
private func installTestLxApp() -> Bool {
    os_log("Installing test lxapp: 95dc2dcfcccc191", log: testInstallLog, type: .info)

    do {
        let fileManager = FileManager.default

        // Get app data directory
        guard let documentsURL = fileManager.urls(for: .documentDirectory, in: .userDomainMask).first else {
            os_log("Failed to get documents directory", log: testInstallLog, type: .error)
            return false
        }

        // Construct paths for source and destination
        // Use the same path structure as the iOS directory provider (includes bundle ID)
        guard let bundleId = Bundle.main.bundleIdentifier else {
            os_log("Bundle identifier not found", log: testInstallLog, type: .error)
            return false
        }
        let appDataDir = documentsURL.appendingPathComponent(bundleId)
        let lingxiaDir = appDataDir.appendingPathComponent("lingxia")
        let lxappsDir = lingxiaDir.appendingPathComponent("lxapps")
        let versionsDir = lingxiaDir.appendingPathComponent("versions")

        // Get homelxapp from SPM Bundle Resources
        // In SPM, resources are in the module bundle, not the main bundle
        guard let bundleResourceURL = Bundle.module.resourceURL else {
            os_log("SPM module bundle resource URL not found", log: testInstallLog, type: .error)
            return false
        }
        let sourceDir = bundleResourceURL.appendingPathComponent("homelxapp")

        // Use the exact hash from the error message for the test lxapp
        let testAppId = "testlxapp"
        let hashedDirName = "95dc2dcfcccc191" // Direct hash from error message

        let destDir = lxappsDir.appendingPathComponent(hashedDirName)
        let sourceVersionFile = versionsDir.appendingPathComponent("homelxapp.txt")
        let destVersionFile = versionsDir.appendingPathComponent("\(testAppId).txt") // Version file uses original appId

        os_log("Using hash directory: %@ for appId: %@", log: testInstallLog, type: .info, hashedDirName, testAppId)

        // Check if source homelxapp exists in Bundle Resources
        guard fileManager.fileExists(atPath: sourceDir.path) else {
            os_log("Source homelxapp not found in Bundle Resources at: %@", log: testInstallLog, type: .error, sourceDir.path)
            return false
        }

        // Remove destination if it already exists
        if fileManager.fileExists(atPath: destDir.path) {
            try fileManager.removeItem(at: destDir)
            os_log("Removed existing test lxapp directory: %@", log: testInstallLog, type: .info, destDir.path)
        }

        // Copy the entire homelxapp directory to create test lxapp
        try fileManager.copyItem(at: sourceDir, to: destDir)
        os_log("Successfully copied homelxapp to test lxapp directory: %@", log: testInstallLog, type: .info, destDir.path)

        // Modify lxapp.json to change appid from homelxapp to testlxapp
        let lxappJsonPath = destDir.appendingPathComponent("lxapp.json")
        if let jsonData = try? Data(contentsOf: lxappJsonPath),
           var jsonObject = try? JSONSerialization.jsonObject(with: jsonData) as? [String: Any] {
            jsonObject["appid"] = testAppId
            if let modifiedData = try? JSONSerialization.data(withJSONObject: jsonObject, options: .prettyPrinted) {
                try modifiedData.write(to: lxappJsonPath)
                os_log("Successfully updated lxapp.json with appid: %@", log: testInstallLog, type: .info, testAppId)
            }
        }

        // Copy version file if it exists
        if fileManager.fileExists(atPath: sourceVersionFile.path) {
            if fileManager.fileExists(atPath: destVersionFile.path) {
                try fileManager.removeItem(at: destVersionFile)
            }
            try fileManager.copyItem(at: sourceVersionFile, to: destVersionFile)
            os_log("Successfully copied version file", log: testInstallLog, type: .info)
        }

        os_log("Test lxapp %@ installation completed successfully in directory: %@", log: testInstallLog, type: .info, testAppId, hashedDirName)
        return true

    } catch {
        os_log("Failed to install test lxapp: %@", log: testInstallLog, type: .error, error.localizedDescription)
        return false
    }
}

@main
public struct LxAppApp: App {
    public init() { }

    public var body: some Scene {
        WindowGroup {
            ContentView()
                .onOpenURL { url in
                    LxApp.handleAppLink(url: url)
                }
        }
    }
}
