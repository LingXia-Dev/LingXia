import Foundation
import WebKit

public enum RunnerBrowserProfile: String, Decodable, Equatable, Hashable, Sendable {
    case desktop
    case phone
    case tablet
}

/// Keeps the Runner's WKWebViews aligned with the selected device form factor.
///
/// The policy retains the installed WebKit version instead of copying a fixed
/// UA from a specific OS release. It emulates a browser family only; viewport,
/// safe areas, touch input, and platform APIs remain separate concerns.
@MainActor
final class RunnerUserAgentPolicy {
    static let shared = RunnerUserAgentPolicy()

    private final class WebViewState: NSObject {
        var defaultUserAgent: String?
        var appliedProfile: RunnerBrowserProfile?
        var resolvingDefault = false
        var reloadRequested = false
    }

    private let states = NSMapTable<WKWebView, WebViewState>(
        keyOptions: .weakMemory,
        valueOptions: .strongMemory
    )
    private(set) var profile: RunnerBrowserProfile = .desktop

    @discardableResult
    func setProfile(_ profile: RunnerBrowserProfile) -> Bool {
        let changed = self.profile != profile
        self.profile = profile
        return changed
    }

    func apply(to webView: WKWebView, reloadIfChanged: Bool = true) {
        let state = state(for: webView)
        state.reloadRequested = state.reloadRequested || reloadIfChanged
        guard state.defaultUserAgent == nil else {
            applyResolvedProfile(to: webView, state: state)
            return
        }
        guard !state.resolvingDefault else { return }

        state.resolvingDefault = true
        Task { @MainActor [weak self, weak webView] in
            guard let self, let webView else { return }
            let result = try? await webView.evaluateJavaScript("navigator.userAgent")
            let state = self.state(for: webView)
            state.resolvingDefault = false
            guard let userAgent = result as? String, !userAgent.isEmpty else { return }
            state.defaultUserAgent = userAgent
            self.applyResolvedProfile(to: webView, state: state)
        }
    }

    private func state(for webView: WKWebView) -> WebViewState {
        if let state = states.object(forKey: webView) {
            return state
        }
        let state = WebViewState()
        states.setObject(state, forKey: webView)
        return state
    }

    private func applyResolvedProfile(to webView: WKWebView, state: WebViewState) {
        guard state.appliedProfile != profile, let defaultUserAgent = state.defaultUserAgent else {
            state.reloadRequested = false
            return
        }

        let previousProfile = state.appliedProfile
        switch profile {
        case .desktop:
            webView.customUserAgent = nil
        case .phone, .tablet:
            guard let userAgent = Self.mobileUserAgent(from: defaultUserAgent, profile: profile) else {
                state.reloadRequested = false
                return
            }
            webView.customUserAgent = userAgent
        }
        state.appliedProfile = profile

        let scheme = webView.url?.scheme?.lowercased()
        let shouldReload = state.reloadRequested
            && (previousProfile != nil || profile != .desktop)
            && (scheme == "http" || scheme == "https")
        state.reloadRequested = false
        if shouldReload {
            webView.reload()
        }
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

        let platform = profile == .phone
            ? "(iPhone; CPU iPhone OS 18_0 like Mac OS X)"
            : "(iPad; CPU OS 18_0 like Mac OS X)"
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
}
