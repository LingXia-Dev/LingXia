import {
  handleBrowserAddressInput,
  type BrowserAddressInputContext,
  type BrowserAddressInputRequest,
  type BrowserAddressInputResponse
} from 'liblingxia.so';

export interface BrowserAddressSubmissionResult {
  url: string;
  displayText: string;
}

function parseBrowserAddressInputResponse(json: string): BrowserAddressInputResponse | null {
  try {
    const parsed: BrowserAddressInputResponse = JSON.parse(json) as BrowserAddressInputResponse;
    return parsed;
  } catch (_error) {
    return null;
  }
}

export function submitBrowserAddress(raw: string, currentUrl: string): BrowserAddressSubmissionResult | null {
  const trimmed = raw.trim();
  if (!trimmed) {
    return null;
  }

  const context: BrowserAddressInputContext = {
    current_url: currentUrl,
    allow_search_fallback: false,
  };
  const request: BrowserAddressInputRequest = {
    raw_input: trimmed,
    trigger: 'submit',
    context: context,
  };
  const requestJson: string = JSON.stringify(request);

  const responseJson: string | null = handleBrowserAddressInput(requestJson);
  if (!responseJson) {
    return null;
  }

  const response: BrowserAddressInputResponse | null = parseBrowserAddressInputResponse(responseJson);
  if (!response) {
    return null;
  }

  const normalized: string | undefined = response.navigation?.url;
  if (response.action !== 'navigate' || !normalized) {
    return null;
  }

  return {
    url: normalized,
    displayText: response.state.display_text,
  };
}
