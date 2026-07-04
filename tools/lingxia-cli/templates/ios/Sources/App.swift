import SwiftUI
// Add the LingXia Swift package dependency in Package.swift before building.
import lingxia

public struct ContentView: View {
    // Use a global flag instead of @State to avoid SwiftUI update cycle issues
    private static var hasInitialized = false

    public var body: some View {
        Color.clear
            .onAppear {
                if !Self.hasInitialized {
                    do {
                        _ = try Lingxia.quickStart()
                        Self.hasInitialized = true
                    } catch {
                        fatalError("Lingxia.quickStart failed: \(error)")
                    }
                }
            }
    }
}

@main
public struct LxAppApp: App {
    public init() { }

    public var body: some Scene {
        WindowGroup {
            ContentView()
                .onOpenURL { url in
                    Lingxia.handleAppLink(url: url)
                }
        }
    }
}
