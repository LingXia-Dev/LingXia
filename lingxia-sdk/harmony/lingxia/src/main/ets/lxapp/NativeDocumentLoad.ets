// One native-issued HTML load. Matching content authorizes only navigation;
// the native document binding still owns all bridge authority.
export class NativeDocumentLoad {
  private pending: boolean = true;

  private html: string;
  private base64: string;
  private historyUrl: string;
  private acceptedUrl: string = '';
  private terminalUrl: string = '';

  constructor(html: string, base64: string, historyUrl: string) {
    this.html = html;
    this.base64 = base64;
    this.historyUrl = historyUrl;
  }

  // ArkWeb begins at the data URL but commits at the native history URL.
  // Normalize only the exact document admitted above, once.
  beginUrl(url: string): string {
    const accepted = this.acceptedUrl;
    this.acceptedUrl = '';
    if (accepted.length > 0 && accepted === url) {
      this.terminalUrl = url;
      return this.historyUrl;
    }
    return url;
  }

  endUrl(url: string): string {
    const terminal = this.terminalUrl;
    this.terminalUrl = '';
    return terminal.length > 0 && terminal === url ? this.historyUrl : url;
  }

  consume(url: string, isMainFrame: boolean): boolean {
    if (!isMainFrame || !this.pending) return false;
    this.pending = false;
    const comma = url.indexOf(',');
    if (comma < 0) return false;
    const metadata = url.substring(0, comma).toLowerCase().split(';');
    if (metadata[0] !== 'data:text/html') return false;
    let encoded: boolean = false;
    for (let i = 1; i < metadata.length; i++) {
      if (metadata[i] === 'base64') encoded = true;
      else if (metadata[i] !== 'charset=utf-8' && metadata[i] !== 'charset=utf8') return false;
    }
    const payload = url.substring(comma + 1);
    try {
      const matches = encoded
        ? decodeURIComponent(payload) === this.base64
        : payload === this.html || decodeURIComponent(payload) === this.html;
      if (matches) this.acceptedUrl = url;
      return matches;
    } catch (_) {
      return false;
    }
  }
}
