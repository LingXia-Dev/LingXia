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
    /// Product pin (`light`/`dark`); `nil` follows the system. Overlay chrome
    /// and `hostIsDark()` read this so iOS matches Android's night-mode pin.
    private static var hostPin: Bool?
    #if os(macOS)
    private static var hostAppearanceObserver: NSKeyValueObservation?
    #endif

    static func hostIsDark() -> Bool {
        observeHostLocale()
        if let hostPin {
            return hostPin
        }
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

    /// Pin the host's own chrome to a scheme, or hand it back to the system.
    /// Every lxapp still resolving `auto` follows through the runtime.
    static func setHostColorMode(dark: Bool?) {
        hostPin = dark
        #if os(macOS)
        // Read once so the effectiveAppearance observer is installed before the
        // assignment that will fire it.
        _ = hostIsDark()
        guard let dark else {
            NSApp.appearance = nil
            return
        }
        NSApp.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        #else
        applyHostPinToWindows()
        #endif
    }

    #if os(iOS)
    private static func applyHostPinToWindows() {
        let style: UIUserInterfaceStyle
        switch hostPin {
        case true: style = .dark
        case false: style = .light
        case nil: style = .unspecified
        }
        for scene in UIApplication.shared.connectedScenes {
            guard let windowScene = scene as? UIWindowScene else { continue }
            for window in windowScene.windows {
                window.overrideUserInterfaceStyle = style
            }
        }
    }
    #endif

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

    #if os(iOS)
    /// Colors for host overlays (capsule sheet, action sheet) in the current
    /// lxapp's scheme.
    struct OverlayColors {
        let scrim: UIColor
        let surface: UIColor
        let title: UIColor
        let secondary: UIColor
        let separator: UIColor
        let icon: UIColor
    }

    static func overlayColors() -> OverlayColors {
        if overlayIsDark() {
            return OverlayColors(
                scrim: UIColor.black.withAlphaComponent(0.55),
                surface: UIColor(red: 0.11, green: 0.11, blue: 0.12, alpha: 1),
                title: .white,
                secondary: UIColor(white: 0.63, alpha: 1),
                separator: UIColor(white: 1, alpha: 0.12),
                icon: UIColor(white: 0.90, alpha: 1)
            )
        }
        return OverlayColors(
            scrim: UIColor.black.withAlphaComponent(0.4),
            surface: .white,
            title: .black,
            secondary: UIColor(white: 0.60, alpha: 1),
            separator: UIColor(red: 0.93, green: 0.93, blue: 0.93, alpha: 1),
            icon: UIColor(red: 0.20, green: 0.20, blue: 0.20, alpha: 1)
        )
    }
    #endif

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
