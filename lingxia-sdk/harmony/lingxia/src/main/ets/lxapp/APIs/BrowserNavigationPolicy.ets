import {
  handleBrowserNavigationPolicy
} from 'liblingxia.so';

export type BrowserNavigationPolicyDecision = 'in_webview' | 'open_external' | 'deny';

export interface BrowserNavigationPolicyRequest {
  raw_url: string;
  has_user_gesture?: boolean;
  is_main_frame?: boolean;
}

export interface BrowserNavigationPolicyResponse {
  decision: BrowserNavigationPolicyDecision;
  reason?: string | null;
}

function parseBrowserNavigationPolicyResponse(json: string): BrowserNavigationPolicyResponse | null {
  try {
    const parsed: BrowserNavigationPolicyResponse = JSON.parse(json) as BrowserNavigationPolicyResponse;
    return parsed;
  } catch (_error) {
    return null;
  }
}

export function resolveBrowserNavigationPolicy(
  rawUrl: string,
  hasUserGesture: boolean,
  isMainFrame: boolean = true
): BrowserNavigationPolicyResponse | null {
  const trimmed = rawUrl.trim();
  if (!trimmed) {
    return null;
  }

  const request: BrowserNavigationPolicyRequest = {
    raw_url: trimmed,
    has_user_gesture: hasUserGesture,
    is_main_frame: isMainFrame
  };
  const requestJson: string = JSON.stringify(request);
  const responseJson: string | null = handleBrowserNavigationPolicy(requestJson);
  if (!responseJson) {
    return null;
  }

  return parseBrowserNavigationPolicyResponse(responseJson);
}
