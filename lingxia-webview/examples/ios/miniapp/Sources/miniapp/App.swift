import SwiftUI
import UIKit
import lingxia
import os.log

/// Main application entry point for LingXia MiniApp system
public class LingXiaMain {
    private static let log = OSLog(subsystem: "LingXia", category: "Main")

    /// Starts the LingXia MiniApp system
    @MainActor
    public static func start() {

        guard Thread.isMainThread else {
            DispatchQueue.main.async {
                start()
            }
            return
        }

        // Initialize MiniApp system as main app (replaceRoot mode)
        MiniApp.initialize(mode: .replaceRoot)
        MiniApp.openHomeMiniApp()
    }
}

public struct ContentView: View {
    @State private var hasStarted = false

    public var body: some View {
        Color.clear
            .onAppear {
                if !hasStarted {
                    hasStarted = true
                    Task { @MainActor in
                        LingXiaMain.start()
                    }
                }
            }
    }
}

@main
public struct MiniAppApp: App {
    public init() {
    }

    public var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
