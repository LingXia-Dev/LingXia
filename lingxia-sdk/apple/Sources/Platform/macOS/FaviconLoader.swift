#if os(macOS)
import AppKit
import WebKit

/// Shared favicon resolution for browser tabs (sidebar rows and the docked
/// aside browser's title tabs).
///
/// Resolution order: Rust's shared favicon cache first (the ONE store — pins
/// read it too), then the page-declared `<link rel=icon>` (most modern sites
/// never expose `/favicon.ico`) falling back to `origin/favicon.ico`. A
/// resolved image is written back into the cache, so visiting a site gives
/// its pin tile an icon as well. The declared link needs the DOM, so
/// resolution waits for the load to settle (bounded) before querying.
@MainActor
enum FaviconLoader {
    /// Resolve the favicon for the webview's current page. Returns nil when
    /// nothing usable is found; callers keep their placeholder then.
    static func resolve(webView: WKWebView) async -> NSImage? {
        let pageURL = webView.url

        // Shared cache first — pins, tab rows, and the docked browser all
        // read (and fill) the same store.
        if let pageURL {
            let cached = browserBookmarkFaviconPath(pageURL.absoluteString).toString()
            if !cached.isEmpty, let image = NSImage(contentsOfFile: cached), image.isValid {
                return image
            }
        }

        // Wait (bounded) for the load to settle so the DOM query sees the
        // page's declared icon link.
        for _ in 0..<25 where webView.isLoading {
            try? await Task.sleep(nanoseconds: 200_000_000)
        }
        // The page navigated away while we waited — stale request.
        guard webView.url == pageURL else { return nil }

        if let declared = await declaredIconURL(in: webView),
           let resolved = await fetchImage(from: declared) {
            return stored(resolved, for: pageURL)
        }
        if let fallback = defaultIconURL(for: pageURL),
           let resolved = await fetchImage(from: fallback) {
            return stored(resolved, for: pageURL)
        }
        return nil
    }

    /// Write the resolved bytes into Rust's shared cache (also wakes the pin
    /// grid) and pass the image through.
    private static func stored(_ resolved: (NSImage, Data), for pageURL: URL?) -> NSImage {
        if let pageURL {
            resolved.1.withUnsafeBytes { buffer in
                if let base = buffer.bindMemory(to: UInt8.self).baseAddress {
                    _ = browserFaviconStore(
                        pageURL.absoluteString,
                        UnsafeBufferPointer(start: base, count: buffer.count))
                }
            }
        }
        return resolved.0
    }

    private static func declaredIconURL(in webView: WKWebView) async -> URL? {
        let js = """
        (function () {
          const link = document.querySelector('link[rel~="icon"]')
            || document.querySelector('link[rel="shortcut icon"]')
            || document.querySelector('link[rel="apple-touch-icon"]');
          return link ? link.href : null;
        })()
        """
        let href = try? await webView.evaluateJavaScript(js) as? String
        return href.flatMap { URL(string: $0) }
    }

    private static func defaultIconURL(for pageURL: URL?) -> URL? {
        guard let pageURL,
              let scheme = pageURL.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              let host = pageURL.host else { return nil }
        let port = pageURL.port.map { ":\($0)" } ?? ""
        return URL(string: "\(scheme)://\(host)\(port)/favicon.ico")
    }

    private static func fetchImage(from url: URL) async -> (NSImage, Data)? {
        guard let (data, response) = try? await URLSession.shared.data(from: url),
              let http = response as? HTTPURLResponse,
              http.statusCode == 200,
              !(http.value(forHTTPHeaderField: "Content-Type") ?? "").hasPrefix("text/"),
              let image = NSImage(data: data),
              image.isValid else { return nil }
        return (image, data)
    }
}
#endif
