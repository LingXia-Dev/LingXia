import Foundation
import WebKit

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

@MainActor
enum LxAppAppearanceRegistry {
    private static var schemes: [String: Bool] = [:]
    private static var webViews: [String: NSHashTable<WKWebView>] = [:]
    #if os(macOS)
    private static var hostAppearanceObserver: NSKeyValueObservation?
    #endif

    static func hostIsDark() -> Bool {
        #if os(iOS)
        return UIScreen.main.traitCollection.userInterfaceStyle == .dark
        #else
        if hostAppearanceObserver == nil {
            hostAppearanceObserver = NSApp.observe(\.effectiveAppearance, options: [.new]) { _, _ in
                Task { @MainActor in onHostAppearanceChanged() }
            }
        }
        return NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        #endif
    }

    static func register(_ webView: WKWebView, appId: String) {
        let table = webViews[appId] ?? NSHashTable<WKWebView>.weakObjects()
        table.add(webView)
        webViews[appId] = table
        if let dark = schemes[appId] {
            apply(dark, to: webView)
        }
    }

    static func set(appId: String, dark: Bool) {
        schemes[appId] = dark
        for webView in webViews[appId]?.allObjects ?? [] {
            apply(dark, to: webView)
        }
        NotificationCenter.default.post(name: .navBarStateChanged, object: appId)
        NotificationCenter.default.post(name: .tabBarStateChanged, object: appId)
    }

    private static func apply(_ dark: Bool, to webView: WKWebView) {
        #if os(iOS)
        webView.overrideUserInterfaceStyle = dark ? .dark : .light
        #else
        webView.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        #endif
    }
}
