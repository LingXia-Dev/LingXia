#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Process-wide host appearance. Rust owns persistence; this object applies
/// the current preference to native windows and reports effective OS changes.
@MainActor
private final class LxAppAppearanceController {
    static let shared = LxAppAppearanceController()

    private(set) var preference: Int32 = 0
    private var callbacks = Set<UInt64>()
    private var lastEffectiveDark: Bool?

    #if os(macOS)
    private var systemAppearanceObservation: NSKeyValueObservation?
    #endif

    private init() {
        lastEffectiveDark = effectiveDark
        #if os(macOS)
        systemAppearanceObservation = NSApp.observe(\.effectiveAppearance) { [weak self] _, _ in
            Task { @MainActor [weak self] in
                self?.systemAppearanceMayHaveChanged()
            }
        }
        #else
        NotificationCenter.default.addObserver(
            forName: UIApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.systemAppearanceMayHaveChanged()
            }
        }
        #endif
    }

    var effectiveDark: Bool {
        switch preference {
        case 1: return false
        case 2: return true
        default:
            #if os(macOS)
            return NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            #else
            let windowStyle = UIApplication.shared.connectedScenes
                .compactMap { $0 as? UIWindowScene }
                .flatMap(\.windows)
                .first(where: { $0.isKeyWindow })?
                .traitCollection.userInterfaceStyle
            return (windowStyle ?? UITraitCollection.current.userInterfaceStyle) == .dark
            #endif
        }
    }

    func setPreference(_ raw: Int32) -> Bool {
        guard (0...2).contains(raw) else { return false }
        preference = raw
        applyToWindows()
        NotificationCenter.default.post(
            name: Notification.Name("LingXiaHostAppearancePreferenceDidChange"),
            object: nil,
            userInfo: ["preference": raw]
        )
        emitIfChanged(force: true)
        return true
    }

    func addListener(_ callbackID: UInt64) {
        callbacks.insert(callbackID)
        emit(to: callbackID)
    }

    func removeListener(_ callbackID: UInt64) {
        callbacks.remove(callbackID)
    }

    func systemAppearanceMayHaveChanged() {
        guard preference == 0 else { return }
        emitIfChanged(force: false)
    }

    private func applyToWindows() {
        #if os(macOS)
        let appearance: NSAppearance? = switch preference {
        case 1: NSAppearance(named: .aqua)
        case 2: NSAppearance(named: .darkAqua)
        default: nil
        }
        NSApp.appearance = appearance
        for window in NSApp.windows {
            window.appearance = appearance
        }
        #else
        let style: UIUserInterfaceStyle = switch preference {
        case 1: .light
        case 2: .dark
        default: .unspecified
        }
        for scene in UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }) {
            for window in scene.windows {
                window.overrideUserInterfaceStyle = style
            }
        }
        #endif
    }

    private func emitIfChanged(force: Bool) {
        let dark = effectiveDark
        guard force || dark != lastEffectiveDark else { return }
        lastEffectiveDark = dark
        _ = onHostAppearanceChanged(preference, dark)
        for callbackID in callbacks {
            emit(to: callbackID)
        }
    }

    private func emit(to callbackID: UInt64) {
        let effective = effectiveDark ? "dark" : "light"
        let json = "{\"preference\":\"\(preferenceLabel)\",\"effective\":\"\(effective)\"}"
        _ = onCallback(callbackID, true, json)
    }

    private var preferenceLabel: String {
        switch preference {
        case 1: "light"
        case 2: "dark"
        default: "system"
        }
    }
}

extension LxApp {
    nonisolated static func hostAppearancePreference() -> Int32 {
        executeOnMain { LxAppAppearanceController.shared.preference }
    }

    nonisolated static func hostAppearanceEffectiveDark() -> Bool {
        executeOnMain { LxAppAppearanceController.shared.effectiveDark }
    }

    nonisolated static func setHostAppearance(preference: Int32) -> Bool {
        executeOnMain { LxAppAppearanceController.shared.setPreference(preference) }
    }

    nonisolated static func addHostAppearanceChangeListener(callback_id: UInt64) {
        executeOnMain { LxAppAppearanceController.shared.addListener(callback_id) }
    }

    nonisolated static func removeHostAppearanceChangeListener(callback_id: UInt64) {
        executeOnMain { LxAppAppearanceController.shared.removeListener(callback_id) }
    }

    nonisolated static func hostSystemAppearanceMayHaveChanged() {
        executeOnMain { LxAppAppearanceController.shared.systemAppearanceMayHaveChanged() }
    }
}
