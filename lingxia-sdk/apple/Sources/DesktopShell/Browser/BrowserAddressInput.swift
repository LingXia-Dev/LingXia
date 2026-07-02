import Foundation
import CLingXiaRustAPI

enum BrowserAddressInputTrigger: String, Codable {
    case edit
    case submit
}

enum BrowserAddressAction: String, Decodable {
    case navigate
    case suggest
    case reject
}

enum BrowserNavigationTarget: String, Decodable {
    case current_tab
    case new_tab
}

struct BrowserAddressInputContextPayload: Encodable {
    let preferred_scheme: String?
    let current_url: String?
    let tab_id: String?
    let allow_search_fallback: Bool
}

struct BrowserAddressInputRequestPayload: Encodable {
    let raw_input: String
    let trigger: BrowserAddressInputTrigger
    let context: BrowserAddressInputContextPayload
}

struct BrowserAddressStatePayload: Decodable {
    let raw_input: String
    let normalized_input: String
    let display_text: String
    let value_kind: String
    let canonical_url: String?
    let inferred_scheme: String?
}

struct BrowserAddressNavigationPayload: Decodable {
    let url: String
    let target: BrowserNavigationTarget
}

struct BrowserAddressInputErrorPayload: Decodable {
    let code: String
    let message: String
}

struct BrowserAddressInputResponsePayload: Decodable {
    let action: BrowserAddressAction
    let state: BrowserAddressStatePayload
    let navigation: BrowserAddressNavigationPayload?
    let suggestions: [BrowserAddressSuggestionPayload]?
    let error: BrowserAddressInputErrorPayload?
}

struct BrowserAddressSuggestionPayload: Decodable {
    let kind: String
    let title: String
    let subtitle: String?
    let fill_text: String
    let navigation: BrowserAddressNavigationPayload?
}

struct BrowserAddressSubmissionResult {
    let url: String
    let displayText: String
}

func handleBrowserAddressSubmission(
    rawInput: String,
    currentURL: String? = nil,
    tabId: String? = nil,
    preferredScheme: String? = nil,
    allowSearchFallback: Bool = false
) -> BrowserAddressSubmissionResult? {
    let request = BrowserAddressInputRequestPayload(
        raw_input: rawInput,
        trigger: .submit,
        context: BrowserAddressInputContextPayload(
            preferred_scheme: preferredScheme,
            current_url: currentURL,
            tab_id: tabId,
            allow_search_fallback: allowSearchFallback
        )
    )

    // The Rust resolver only navigates when built with the `browser-shell`
    // feature (desktop app); this path wins whenever it yields a result.
    let encoder = JSONEncoder()
    if let requestData = try? encoder.encode(request),
       let requestJson = String(data: requestData, encoding: .utf8),
       let responseJson = handleBrowserAddressInput(requestJson),
       let responseData = responseJson.toString().data(using: .utf8),
       let response = try? JSONDecoder().decode(BrowserAddressInputResponsePayload.self, from: responseData),
       response.action == .navigate,
       let url = response.navigation?.url {
        return BrowserAddressSubmissionResult(
            url: url,
            displayText: response.state.display_text
        )
    }

    // Fallback when the resolver yields nothing (e.g. the runner, built without
    // `browser-shell`): mirror RunnerPhoneBrowserSurface — a full http/https/
    // lingxia URL loads as-is, a bare host gets https, else give up (native
    // chrome has no search provider).
    let input = rawInput.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !input.isEmpty else { return nil }
    let target: URL?
    if let url = URL(string: input),
       let scheme = url.scheme?.lowercased(),
       scheme == "http" || scheme == "https" || scheme == "lingxia" {
        target = url
    } else if !input.contains(" "), input.contains("."), let url = URL(string: "https://\(input)") {
        target = url
    } else {
        target = nil
    }
    guard let target else { return nil }
    return BrowserAddressSubmissionResult(
        url: target.absoluteString,
        displayText: target.absoluteString
    )
}
