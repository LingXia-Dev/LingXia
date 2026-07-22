import Foundation
import WebKit

public enum RunnerBrowserProfile: String, Decodable, Equatable, Hashable, Sendable {
    case desktop
    case phone
    case tablet
}

/// Produces an engine-compatible UA before the Runner opens its first page.
/// Rust-managed lxapp/browser WebViews and Swift URL surfaces both inherit the
/// configured value during creation, before their first navigation.
@MainActor
final class RunnerUserAgentPolicy {
    static let shared = RunnerUserAgentPolicy()

    private(set) var profile: RunnerBrowserProfile = .desktop
    private var defaultUserAgent: String?

    func prepare() async -> Bool {
        guard defaultUserAgent == nil else { return true }
        let probe = WKWebView(frame: .zero)
        let result = try? await probe.evaluateJavaScript("navigator.userAgent")
        guard let userAgent = result as? String,
              !userAgent.isEmpty
        else {
            return false
        }
        defaultUserAgent = userAgent
        return applyConfiguredProfile(reloadExisting: false)
    }

    @discardableResult
    func setProfile(_ profile: RunnerBrowserProfile) -> Bool {
        let changed = self.profile != profile
        self.profile = profile
        if changed, defaultUserAgent != nil {
            if !applyConfiguredProfile(reloadExisting: true) {
                NSLog("LingXia Runner could not apply the browser user agent")
            }
        }
        return changed
    }

    private func applyConfiguredProfile(reloadExisting: Bool) -> Bool {
        let userAgent: String?
        switch profile {
        case .desktop:
            userAgent = nil
        case .phone, .tablet:
            userAgent = defaultUserAgent.flatMap {
                Self.mobileUserAgent(from: $0, profile: profile)
            }
        }
        return RunnerSupport.WebView.configureUserAgentOverride(
            userAgent,
            reloadExisting: reloadExisting
        )
    }

    static func mobileUserAgent(
        from defaultUserAgent: String,
        profile: RunnerBrowserProfile
    ) -> String? {
        guard profile != .desktop,
              defaultUserAgent.hasPrefix("Mozilla/5.0 "),
              defaultUserAgent.contains(" AppleWebKit/"),
              let platformStart = defaultUserAgent.firstIndex(of: "("),
              let platformEnd = defaultUserAgent[platformStart...].firstIndex(of: ")")
        else {
            return nil
        }

        let osMajor = mobileOSMajor(from: defaultUserAgent)
        let platform = profile == .phone
            ? "(iPhone; CPU iPhone OS \(osMajor)_0 like Mac OS X)"
            : "(iPad; CPU OS \(osMajor)_0 like Mac OS X)"
        var userAgent = String(defaultUserAgent[..<platformStart])
            + platform
            + String(defaultUserAgent[defaultUserAgent.index(after: platformEnd)...])
        if !userAgent.contains(" Mobile/") {
            if let safariRange = userAgent.range(of: " Safari/") {
                userAgent.insert(contentsOf: " Mobile/15E148", at: safariRange.lowerBound)
            } else {
                userAgent += " Mobile/15E148"
            }
        }
        return userAgent
    }

    private static func mobileOSMajor(from userAgent: String) -> Int {
        if let versionRange = userAgent.range(of: " Version/"),
           let major = Int(
               userAgent[versionRange.upperBound...]
                   .prefix(while: { $0.isNumber })
           )
        {
            return major
        }
        let macOSMajor = ProcessInfo.processInfo.operatingSystemVersion.majorVersion
        return macOSMajor >= 26 ? macOSMajor : macOSMajor + 3
    }
}
