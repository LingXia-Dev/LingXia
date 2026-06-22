(function () {
  'use strict';
  if (window.__lingxiaBrowserContextMenu) return;
  window.__lingxiaBrowserContextMenu = true;

  var EXT_TO_MIME = {
    jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png',
    gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml',
    avif: 'image/avif', bmp: 'image/bmp', tiff: 'image/tiff', ico: 'image/x-icon',
    mp4: 'video/mp4', webm: 'video/webm', ogv: 'video/ogg', mov: 'video/quicktime',
    mp3: 'audio/mpeg', m4a: 'audio/mp4', ogg: 'audio/ogg', wav: 'audio/wav',
    pdf: 'application/pdf',
  };

  var MIME_TO_EXT = {
    'image/jpeg': 'jpg', 'image/png': 'png', 'image/gif': 'gif',
    'image/webp': 'webp', 'image/svg+xml': 'svg', 'image/avif': 'avif',
    'image/bmp': 'bmp', 'image/tiff': 'tiff', 'image/x-icon': 'ico',
    'video/mp4': 'mp4', 'video/webm': 'webm', 'video/ogg': 'ogv',
    'video/quicktime': 'mov', 'audio/mpeg': 'mp3', 'audio/mp4': 'm4a',
    'audio/ogg': 'ogg', 'audio/wav': 'wav', 'application/pdf': 'pdf',
  };

  // Common CDN query parameter names that declare the served format.
  // Query params take priority over path extension because CDNs like Twitter
  // serve e.g. ?format=webp regardless of what the path segment looks like.
  var FORMAT_QUERY_KEYS = ['format', 'fmt', 'type', 'ext', 'fm', 'f', 'filetype', 'img_type'];

  function normalizeUrl(raw) {
    if (typeof raw !== 'string') return null;
    var trimmed = raw.trim();
    if (!trimmed) return null;
    try {
      var resolved = new URL(trimmed, document.baseURI).href;
      return /^https?:\/\//i.test(resolved) ? resolved : null;
    } catch (_) {
      return null;
    }
  }

  // Extract the extension from the last path segment, ignoring query/hash.
  function extFromPathname(pathname) {
    var last = (pathname || '').split('/').pop() || '';
    var dot = last.lastIndexOf('.');
    if (dot < 0) return null;
    return last.slice(dot + 1).toLowerCase().split(/[?#]/)[0] || null;
  }

  // Infer MIME type from URL. Query params are checked first (explicit CDN
  // format declaration), then path extension as fallback.
  function mimeFromUrl(url) {
    try {
      var parsed = new URL(url);
      for (var i = 0; i < FORMAT_QUERY_KEYS.length; i++) {
        var val = (parsed.searchParams.get(FORMAT_QUERY_KEYS[i]) || '').toLowerCase();
        if (!val) continue;
        if (EXT_TO_MIME[val]) return EXT_TO_MIME[val];   // e.g. "jpg" → image/jpeg
        if (MIME_TO_EXT[val]) return val;                 // e.g. "image/webp" in param
      }
      var ext = extFromPathname(parsed.pathname);
      if (ext && EXT_TO_MIME[ext]) return EXT_TO_MIME[ext];
    } catch (_) {}
    return null;
  }

  // MIME type from element attributes (e.g. <source type="image/webp">).
  function mimeFromElement(node) {
    if (!node) return null;
    var typeAttr = node.getAttribute && node.getAttribute('type');
    if (typeAttr) {
      var t = typeAttr.toLowerCase();
      if (MIME_TO_EXT[t]) return t;
    }
    return null;
  }

  // Return filename with the correct extension for the given MIME type.
  // - If filename already has the matching extension, return as-is.
  // - If filename has no extension, append the correct one.
  // - If filename has a mismatched extension (e.g. path says .jpg but CDN
  //   serves .webp based on a query param), replace it.
  function withExtension(filename, mime) {
    if (!filename || !mime) return filename;
    var expectedExt = MIME_TO_EXT[mime];
    if (!expectedExt) return filename;

    var dot = filename.lastIndexOf('.');
    if (dot >= 0) {
      var currentExt = filename.slice(dot + 1).toLowerCase();
      // Treat jpeg / jpg as the same.
      var normalised = currentExt === 'jpeg' ? 'jpg' : currentExt;
      if (normalised === expectedExt) return filename;
      // Replace mismatched extension.
      return filename.slice(0, dot) + '.' + expectedExt;
    }
    return filename + '.' + expectedExt;
  }

  // Build the best suggestedFilename we can:
  //   fallback  — value from the element's `download` attribute
  //   mime      — inferred MIME; used to add / fix the extension
  //   url       — last path segment used as the base name
  function inferFilename(url, mime, fallback) {
    var base = null;
    try {
      var last = new URL(url).pathname.split('/').pop();
      if (last) base = decodeURIComponent(last).split(/[?#]/)[0] || null;
    } catch (_) {}

    var name = fallback || base || null;
    if (!name || !mime) return name;
    return withExtension(name, mime);
  }

  function titleFor(url, kind) {
    if (/\.pdf(?:$|[?#])/i.test(url)) return 'Download PDF';
    switch (kind) {
      case 'image': return 'Download Image';
      case 'media': return 'Download Media';
      case 'link':  return 'Download Linked File';
      default:      return 'Download File';
    }
  }

  function candidate(url, kind, fallbackName, mimeOverride) {
    if (!url) return null;
    var mime = mimeOverride || mimeFromUrl(url) || null;
    return {
      url: url,
      suggestedFilename: inferFilename(url, mime, fallbackName || null),
      menuTitle: titleFor(url, kind),
    };
  }

  function backgroundImageUrl(node) {
    try {
      var style = window.getComputedStyle(node);
      var raw = style && typeof style.backgroundImage === 'string' ? style.backgroundImage : '';
      var match = /url\((['"]?)(.*?)\1\)/i.exec(raw);
      return match ? normalizeUrl(match[2]) : null;
    } catch (_) {
      return null;
    }
  }

  function datasetUrl(node) {
    if (!node || !node.dataset) return null;
    var entries = Object.entries(node.dataset);
    for (var i = 0; i < entries.length; i++) {
      var key = entries[i][0], value = entries[i][1];
      if (!value || typeof value !== 'string') continue;
      if (/(url|src|image|media|poster)/i.test(key)) {
        var normalized = normalizeUrl(value);
        if (normalized) return normalized;
      }
    }
    return null;
  }

  function attributeUrl(node, names) {
    for (var i = 0; i < names.length; i++) {
      var value = node.getAttribute && node.getAttribute(names[i]);
      var normalized = normalizeUrl(value);
      if (normalized) return normalized;
    }
    return null;
  }

  function extractCandidate(node) {
    if (!node || node.nodeType !== 1) return null;
    var tag = node.tagName;

    if (tag === 'IMG') {
      var imgUrl = normalizeUrl(node.currentSrc || node.src || node.getAttribute('src') || node.getAttribute('data-src'));
      if (imgUrl) return candidate(imgUrl, 'image', node.getAttribute('download') || null, null);
    }

    if (tag === 'VIDEO' || tag === 'AUDIO') {
      var mediaUrl = normalizeUrl(node.currentSrc || node.src || node.getAttribute('src'));
      if (mediaUrl) return candidate(mediaUrl, 'media', null, null);
      var posterUrl = normalizeUrl(node.getAttribute('poster'));
      if (posterUrl) return candidate(posterUrl, 'image', null, null);
    }

    if (tag === 'A') {
      var linkUrl = normalizeUrl(node.href || node.getAttribute('href'));
      if (linkUrl) return candidate(linkUrl, 'link', node.getAttribute('download') || null, null);
    }

    if (tag === 'SOURCE') {
      var sourceUrl = normalizeUrl(node.src || node.getAttribute('src'));
      if (sourceUrl) return candidate(sourceUrl, 'media', null, mimeFromElement(node));
    }

    var attrNames = ['src', 'href', 'data-src', 'data-url', 'data-image-url', 'data-fullimage', 'data-media-url', 'poster'];
    var attrUrl = attributeUrl(node, attrNames);
    if (attrUrl) {
      var inferredKind = /(jpg|jpeg|png|gif|webp|bmp|svg|avif)(?:$|[?#])/i.test(attrUrl) ? 'image' : 'file';
      return candidate(attrUrl, inferredKind, node.getAttribute && node.getAttribute('download'), null);
    }

    var bgUrl = backgroundImageUrl(node);
    if (bgUrl) return candidate(bgUrl, 'image', null, null);

    var dsUrl = datasetUrl(node);
    if (dsUrl) {
      var dsKind = /(jpg|jpeg|png|gif|webp|bmp|svg|avif)(?:$|[?#])/i.test(dsUrl) ? 'image' : 'file';
      return candidate(dsUrl, dsKind, null, null);
    }

    if (typeof node.querySelector === 'function') {
      var child = node.querySelector('img, video, audio, source, a[href], [src], [data-src]');
      if (child) return extractCandidate(child);
    }

    return null;
  }

  document.addEventListener('contextmenu', function (e) {
    var node = e.target;
    var visited = new Set();
    while (node && node.nodeType === 1) {
      if (visited.has(node)) break;
      visited.add(node);
      var found = extractCandidate(node);
      if (found) {
        try {
          window.webkit.messageHandlers.LingXia.postMessage(JSON.stringify({
            type: 'browserContextMenuDownload',
            payload: found
          }));
        } catch (_) {}
        return;
      }
      node = node.parentElement;
    }
  }, true);
})();
