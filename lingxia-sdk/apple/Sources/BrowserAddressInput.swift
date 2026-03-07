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

    let encoder = JSONEncoder()
    guard let requestData = try? encoder.encode(request),
          let requestJson = String(data: requestData, encoding: .utf8),
          let responseJson = handleBrowserAddressInput(requestJson),
          let responseData = responseJson.toString().data(using: .utf8) else {
        return nil
    }

    guard let response = try? JSONDecoder().decode(BrowserAddressInputResponsePayload.self, from: responseData),
          response.action == .navigate,
          let url = response.navigation?.url else {
        return nil
    }

    return BrowserAddressSubmissionResult(
        url: url,
        displayText: response.state.display_text
    )
}
