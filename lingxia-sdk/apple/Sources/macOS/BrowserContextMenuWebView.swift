#if os(macOS)
import AppKit
import OSLog
import WebKit
import CLingXiaRustAPI

private func startBrowserTabDownloadBridge(
    _ tabId: String,
    _ url: String,
    _ userAgent: String,
    _ suggestedFilename: String,
    _ sourcePageUrl: String,
    _ cookie: String
) -> Bool {
    startBrowserTabDownload(
        tabId,
        url,
        userAgent,
        suggestedFilename,
        sourcePageUrl,
        cookie
    )
}

@objc(LingXiaBrowserContextMenuWebView)
final class BrowserContextMenuWebView: WKWebView {
    private static let logger = Logger(subsystem: "LingXia", category: "BrowserContextMenu")

    private struct DownloadCandidate: Decodable {
        let url: String
        let suggestedFilename: String?
        let menuTitle: String
    }

    private struct BrowserRequestContext {
        let userAgent: String
        let sourcePageURL: String
    }

    private var pendingDownloadCandidate: DownloadCandidate?

    override func rightMouseDown(with event: NSEvent) {
        guard let currentPath, currentPath.hasPrefix("/tabs/") else {
            super.rightMouseDown(with: event)
            return
        }

        resolveDownloadCandidate(for: event) { [weak self] candidate in
            guard let self else { return }
            guard let candidate else {
                self.showDefaultContextMenu(for: event)
                return
            }
            self.presentDownloadContextMenu(for: event, candidate: candidate)
        }
    }

    private func resolveDownloadCandidate(
        for event: NSEvent,
        completion: @escaping (DownloadCandidate?) -> Void
    ) {
        let point = convert(event.locationInWindow, from: nil)
        let domX = max(0, min(bounds.width, point.x))
        let domY = max(0, min(bounds.height, bounds.height - point.y))
        let js = """
        (function(x, y) {
          const EXT_TO_MIME = {
            jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', gif: 'image/gif',
            webp: 'image/webp', svg: 'image/svg+xml', avif: 'image/avif', bmp: 'image/bmp',
            tiff: 'image/tiff', ico: 'image/x-icon', mp4: 'video/mp4', webm: 'video/webm',
            ogv: 'video/ogg', mov: 'video/quicktime', mp3: 'audio/mpeg', m4a: 'audio/mp4',
            ogg: 'audio/ogg', wav: 'audio/wav', pdf: 'application/pdf',
          };
          const MIME_TO_EXT = {
            'image/jpeg': 'jpg', 'image/png': 'png', 'image/gif': 'gif', 'image/webp': 'webp',
            'image/svg+xml': 'svg', 'image/avif': 'avif', 'image/bmp': 'bmp',
            'image/tiff': 'tiff', 'image/x-icon': 'ico', 'video/mp4': 'mp4',
            'video/webm': 'webm', 'video/ogg': 'ogv', 'video/quicktime': 'mov',
            'audio/mpeg': 'mp3', 'audio/mp4': 'm4a', 'audio/ogg': 'ogg',
            'audio/wav': 'wav', 'application/pdf': 'pdf',
          };
          const FORMAT_QUERY_KEYS = ['format', 'fmt', 'type', 'ext', 'fm', 'f', 'filetype', 'img_type'];
          function mimeFromUrl(url) {
            try {
              const parsed = new URL(url);
              for (const key of FORMAT_QUERY_KEYS) {
                const val = (parsed.searchParams.get(key) || '').toLowerCase();
                if (!val) continue;
                if (EXT_TO_MIME[val]) return EXT_TO_MIME[val];
                if (MIME_TO_EXT[val]) return val;
              }
              const ext = (parsed.pathname.split('/').pop() || '').split('.').pop().toLowerCase().split(/[?#]/)[0];
              if (ext && EXT_TO_MIME[ext]) return EXT_TO_MIME[ext];
            } catch (_) {}
            return null;
          }
          function mimeFromElement(node) {
            if (!node) return null;
            const typeAttr = node.getAttribute && node.getAttribute('type');
            if (typeAttr) {
              const lower = typeAttr.toLowerCase();
              if (MIME_TO_EXT[lower]) return lower;
            }
            return null;
          }
          function withExtension(filename, mime) {
            if (!filename || !mime) return filename;
            const expectedExt = MIME_TO_EXT[mime];
            if (!expectedExt) return filename;
            const dot = filename.lastIndexOf('.');
            if (dot >= 0) {
              const currentExt = filename.slice(dot + 1).toLowerCase();
              const normalised = currentExt === 'jpeg' ? 'jpg' : currentExt;
              if (normalised === expectedExt) return filename;
              return filename.slice(0, dot) + '.' + expectedExt;
            }
            return filename + '.' + expectedExt;
          }
          function normalizeUrl(raw) {
            if (typeof raw !== 'string') return null;
            const trimmed = raw.trim();
            if (!trimmed) return null;
            try {
              const resolved = new URL(trimmed, document.baseURI).href;
              return /^https?:\\/\\//i.test(resolved) ? resolved : null;
            } catch (_) {
              return null;
            }
          }
          function inferFilename(url, mime, fallback) {
            let base = null;
            try {
              const last = new URL(url).pathname.split('/').pop();
              if (last) base = decodeURIComponent(last).split(/[?#]/)[0] || null;
            } catch (_) {}
            const name = fallback || base || null;
            if (!name || !mime) return name;
            return withExtension(name, mime);
          }
          function titleFor(url, kind) {
            if (/\\.pdf(?:$|[?#])/i.test(url)) {
              return 'Download PDF';
            }
            switch (kind) {
              case 'image': return 'Download Image';
              case 'media': return 'Download Media';
              case 'link': return 'Download Linked File';
              default: return 'Download File';
            }
          }
          function backgroundImageUrl(node) {
            try {
              const style = window.getComputedStyle(node);
              const raw = style && typeof style.backgroundImage === 'string' ? style.backgroundImage : '';
              const match = /url\\((['"]?)(.*?)\\1\\)/i.exec(raw);
              return match ? normalizeUrl(match[2]) : null;
            } catch (_) {
              return null;
            }
          }
          function datasetUrl(node) {
            if (!node || !node.dataset) return null;
            for (const [key, value] of Object.entries(node.dataset)) {
              if (!value || typeof value !== 'string') continue;
              if (/(url|src|image|media|poster)/i.test(key)) {
                const normalized = normalizeUrl(value);
                if (normalized) return normalized;
              }
            }
            return null;
          }
          function parseJsonishUrl(raw) {
            if (typeof raw !== 'string' || !raw.trim()) return null;
            const normalizedDirect = normalizeUrl(raw);
            if (normalizedDirect) return normalizedDirect;
            const patterns = [
              /"(?:mediaurl|mediaUrl|murl|imgurl|imgUrl|sourceUrl|imageUrl)"\\s*:\\s*"([^"]+)"/i,
              /'(?:mediaurl|mediaUrl|murl|imgurl|imgUrl|sourceUrl|imageUrl)'\\s*:\\s*'([^']+)'/i
            ];
            for (const pattern of patterns) {
              const match = pattern.exec(raw);
              if (match) {
                const normalized = normalizeUrl(match[1]);
                if (normalized) return normalized;
              }
            }
            return null;
          }
          function attributeUrl(node, names) {
            for (const name of names) {
              const value = node.getAttribute && node.getAttribute(name);
              const normalized = normalizeUrl(value);
              if (normalized) return normalized;
              const jsonish = parseJsonishUrl(value);
              if (jsonish) return jsonish;
            }
            return null;
          }
          function firstDescendantUrl(node) {
            if (!node || typeof node.querySelector !== 'function') return null;
            const descendant = node.querySelector('img, video, audio, source, a[href], [src], [data-src]');
            return descendant ? extractCandidate(descendant) : null;
          }
          function candidate(url, kind, fallbackName, mimeOverride) {
            if (!url) return null;
            const mime = mimeOverride || mimeFromUrl(url) || null;
            return {
              url,
              suggestedFilename: inferFilename(url, mime, fallbackName || null),
              menuTitle: titleFor(url, kind)
            };
          }
          function bingDetailMediaCandidate() {
            try {
              const pageUrl = new URL(window.location.href);
              if (!/bing\\.com$/i.test(pageUrl.hostname)) return null;
              if (!/\\/images\\/search/i.test(pageUrl.pathname)) return null;
              if (pageUrl.searchParams.get('view') !== 'detailV2' && pageUrl.searchParams.get('mode') !== 'overlay') {
                return null;
              }
              const mediaUrl = normalizeUrl(pageUrl.searchParams.get('mediaurl') || pageUrl.searchParams.get('imgurl'));
              return mediaUrl ? candidate(mediaUrl, 'image', null) : null;
            } catch (_) {
              return null;
            }
          }
          function extractCandidate(node) {
            if (!node || node.nodeType !== 1) return null;
            const tag = node.tagName;
            if (tag === 'IMG') {
              const url = normalizeUrl(node.currentSrc || node.src || node.getAttribute('src') || node.getAttribute('data-src'));
              if (url) {
                const downloadName = node.getAttribute('download') || null;
                return candidate(url, 'image', downloadName);
              }
            }
            if (tag === 'VIDEO' || tag === 'AUDIO') {
              const mediaUrl = normalizeUrl(node.currentSrc || node.src || node.getAttribute('src'));
              if (mediaUrl) {
                return candidate(mediaUrl, 'media', null);
              }
              const posterUrl = normalizeUrl(node.getAttribute('poster'));
              if (posterUrl) {
                return candidate(posterUrl, 'image', null);
              }
            }
            if (tag === 'A') {
              const linkUrl = normalizeUrl(node.href || node.getAttribute('href'));
              if (linkUrl) {
                const downloadName = node.getAttribute('download') || null;
                return candidate(linkUrl, 'link', downloadName);
              }
            }
            if (tag === 'SOURCE') {
              const sourceUrl = normalizeUrl(node.src || node.getAttribute('src'));
              if (sourceUrl) {
                return candidate(sourceUrl, 'media', null, mimeFromElement(node));
              }
            }
            const attrUrl = attributeUrl(node, ['src', 'href', 'data-src', 'data-url', 'data-image-url', 'data-fullimage', 'data-media-url', 'poster']);
            if (attrUrl) {
              const inferredKind = /(jpg|jpeg|png|gif|webp|bmp|svg|avif)(?:$|[?#])/i.test(attrUrl) ? 'image' : 'file';
              return candidate(attrUrl, inferredKind, node.getAttribute && node.getAttribute('download'));
            }

            const bgUrl = backgroundImageUrl(node);
            if (bgUrl) {
              return candidate(bgUrl, 'image', null);
            }

            const dataUrl = datasetUrl(node);
            if (dataUrl) {
              const inferredKind = /(jpg|jpeg|png|gif|webp|bmp|svg|avif)(?:$|[?#])/i.test(dataUrl) ? 'image' : 'file';
              return candidate(dataUrl, inferredKind, null);
            }

            return firstDescendantUrl(node);
          }

          const stack = typeof document.elementsFromPoint === 'function'
            ? document.elementsFromPoint(x, y)
            : [document.elementFromPoint(x, y)].filter(Boolean);
          const visited = new Set();
          const bingDetailCandidate = bingDetailMediaCandidate();

          for (const originNode of stack) {
            let node = originNode;
            while (node && node.nodeType === 1) {
              if (!visited.has(node)) {
                visited.add(node);
                const found = extractCandidate(node);
                if (found) {
                  if (bingDetailCandidate) {
                    try {
                      const foundUrl = new URL(found.url);
                      if (/bing\\.com$/i.test(foundUrl.hostname) || /\\.svg(?:$|[?#])/i.test(found.url)) {
                        return JSON.stringify(bingDetailCandidate);
                      }
                    } catch (_) {}
                  }
                  return JSON.stringify(found);
                }
              }
              node = node.parentElement;
            }
          }

          if (bingDetailCandidate) {
            return JSON.stringify(bingDetailCandidate);
          }

          return null;
        })(\(domX), \(domY));
        """

        evaluateJavaScript(js) { result, error in
            guard error == nil else {
                Self.logger.error("Resolve download candidate JS failed path=\(self.currentPath ?? "-", privacy: .public) error=\(String(describing: error), privacy: .public)")
                completion(nil)
                return
            }

            let jsonString: String?
            if let string = result as? String {
                jsonString = string
            } else if result is NSNull || result == nil {
                jsonString = nil
            } else {
                jsonString = nil
            }

            guard let jsonString,
                  let data = jsonString.data(using: .utf8),
                  let candidate = try? JSONDecoder().decode(DownloadCandidate.self, from: data),
                  candidate.url.hasPrefix("http://") || candidate.url.hasPrefix("https://") else {
                Self.logger.debug("No custom download candidate for path=\(self.currentPath ?? "-", privacy: .public)")
                completion(nil)
                return
            }

            Self.logger.info("Resolved download candidate path=\(self.currentPath ?? "-", privacy: .public) title=\(candidate.menuTitle, privacy: .public) url=\(candidate.url, privacy: .public)")
            completion(candidate)
        }
    }

    private func showDefaultContextMenu(for event: NSEvent) {
        super.rightMouseDown(with: event)
    }

    private func presentDownloadContextMenu(for event: NSEvent, candidate: DownloadCandidate) {
        pendingDownloadCandidate = candidate
        Self.logger.info("Presenting custom browser download menu path=\(self.currentPath ?? "-", privacy: .public) title=\(candidate.menuTitle, privacy: .public) url=\(candidate.url, privacy: .public)")

        let menu = super.menu(for: event) ?? NSMenu()
        let item = NSMenuItem(
            title: candidate.menuTitle,
            action: #selector(handleDownloadResource(_:)),
            keyEquivalent: ""
        )
        item.target = self

        if menu.items.isEmpty {
            menu.addItem(item)
        } else {
            menu.insertItem(item, at: 0)
            menu.insertItem(NSMenuItem.separator(), at: 1)
        }

        NSMenu.popUpContextMenu(menu, with: event, for: self)
    }

    @objc private func handleDownloadResource(_ sender: Any?) {
        _ = sender
        guard let candidate = pendingDownloadCandidate,
              let currentPath,
              let tabId = currentPath.split(separator: "/").last.map(String.init),
              !tabId.isEmpty else {
            pendingDownloadCandidate = nil
            return
        }

        pendingDownloadCandidate = nil
        fetchBrowserRequestContext(for: candidate.url) { [weak self] context in
            guard self != nil else { return }
            let started = startBrowserTabDownloadBridge(
                tabId,
                candidate.url,
                context.userAgent,
                candidate.suggestedFilename ?? "",
                context.sourcePageURL,
                ""
            )
            if started {
                Self.logger.info("Browser download bridge accepted tab=\(tabId, privacy: .public) url=\(candidate.url, privacy: .public)")
            } else {
                Self.logger.error("Browser download bridge rejected tab=\(tabId, privacy: .public) url=\(candidate.url, privacy: .public)")
            }
        }
    }

    private func fetchBrowserRequestContext(
        for targetURLString: String,
        completion: @escaping (BrowserRequestContext) -> Void
    ) {
        let sourcePageURL = url?.absoluteString ?? ""
        let initialUserAgent = customUserAgent?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let _ = targetURLString

        resolveUserAgent(fallback: initialUserAgent) { userAgent in
            completion(
                BrowserRequestContext(
                    userAgent: userAgent,
                    sourcePageURL: sourcePageURL
                )
            )
        }
    }

    private func resolveUserAgent(
        fallback: String,
        completion: @escaping (String) -> Void
    ) {
        if !fallback.isEmpty {
            completion(fallback)
            return
        }

        evaluateJavaScript("navigator.userAgent") { result, _ in
            if let userAgent = result as? String {
                completion(userAgent)
            } else {
                completion("")
            }
        }
    }
}
#endif
