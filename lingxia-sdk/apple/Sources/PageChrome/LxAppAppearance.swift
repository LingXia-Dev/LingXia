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
    private static var hostLocaleObserver: NSObjectProtocol?
    #if os(macOS)
    private static var hostAppearanceObserver: NSKeyValueObservation?
    #endif

    static func hostIsDark() -> Bool {
        observeHostLocale()
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

    static func observeHostLocale() {
        guard hostLocaleObserver == nil else { return }
        hostLocaleObserver = NotificationCenter.default.addObserver(
            forName: NSLocale.currentLocaleDidChangeNotification,
            object: nil,
            queue: .main
        ) { _ in
            Task { @MainActor in onHostLocaleChanged(Locale.current.identifier) }
        }
    }

    /// The lxapp's applied scheme, if one has been resolved yet.
    static func resolvedDark(appId: String) -> Bool? {
        schemes[appId]
    }

    /// Scheme for chrome that belongs to no single lxapp — modals, action
    /// sheets and friends. They sit above the current lxapp, so they follow
    /// its scheme first and the host's only as a fallback.
    static func overlayIsDark() -> Bool {
        if let appId = LxAppCore.currentAppId, let dark = schemes[appId] {
            return dark
        }
        return hostIsDark()
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
        // Setup froze light-resolved cgColors on the webview/scroll layers,
        // and the overscroll canvas never follows page CSS — re-resolve both
        // so rubber-banding shows the scheme's background, not white.
        guard webView.isOpaque else { return }
        let traits = UITraitCollection(userInterfaceStyle: dark ? .dark : .light)
        let background = UIColor.systemBackground.resolvedColor(with: traits)
        webView.underPageBackgroundColor = background
        webView.backgroundColor = background
        webView.layer.backgroundColor = background.cgColor
        webView.scrollView.backgroundColor = background
        webView.scrollView.layer.backgroundColor = background.cgColor
        #else
        webView.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        // Setup pre-paints fixed white (light-first); once a scheme resolves,
        // the canvas must follow it or dark pages flash white on load/resize.
        if !webView.drawsTransparentCanvas {
            let background =
                dark
                ? NSColor(srgbRed: 0x1C / 255.0, green: 0x1C / 255.0, blue: 0x1E / 255.0, alpha: 1)
                : NSColor.white
            webView.layer?.backgroundColor = background.cgColor
        }
        #endif
    }
}
