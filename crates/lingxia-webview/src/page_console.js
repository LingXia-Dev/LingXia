// Page console recorder for browser-profile WebViews: a bounded ring of what
// the page logged and threw, kept in the page and read by browser automation
// through script. It never reaches the native log delegate, which a browser
// WebView reserves for its trusted control documents.
(function () {
  if (window !== window.top || window.__LingXiaPageConsole) return;
  var MAX = 500;
  var MAX_TEXT = 4000;
  var entries = [];
  var seq = 0;
  var dropped = 0;

  function format(args) {
    var parts = [];
    for (var i = 0; i < args.length; i++) {
      var arg = args[i];
      try {
        if (arg instanceof Error) parts.push(arg.stack || String(arg));
        else if (arg !== null && typeof arg === 'object') parts.push(JSON.stringify(arg));
        else parts.push(String(arg));
      } catch (e) {
        try { parts.push(String(arg)); } catch (_) { parts.push('[unprintable]'); }
      }
    }
    return parts.join(' ');
  }

  function push(level, text) {
    seq += 1;
    if (text.length > MAX_TEXT) text = text.slice(0, MAX_TEXT) + '…';
    entries.push({ seq: seq, level: level, text: text, timestamp_ms: Date.now() });
    if (entries.length > MAX) {
      entries.shift();
      dropped += 1;
    }
  }

  ['log', 'info', 'warn', 'error', 'debug'].forEach(function (level) {
    var original = console[level];
    if (typeof original !== 'function') return;
    console[level] = function () {
      try { push(level, format(arguments)); } catch (e) {}
      return original.apply(this, arguments);
    };
  });
  window.addEventListener('error', function (event) {
    if (event.target && event.target !== window) {
      var src = event.target.src || event.target.href || '';
      push('error', 'Failed to load ' + (event.target.localName || 'resource') + (src ? ' ' + src : ''));
      return;
    }
    var where = event.filename ? ' (' + event.filename + ':' + event.lineno + ')' : '';
    var stack = event.error && event.error.stack;
    push('error', 'Uncaught ' + (event.message || 'error') + where + (stack ? '\n' + stack : ''));
  }, true);
  window.addEventListener('unhandledrejection', function (event) {
    var reason = event.reason;
    var text;
    try { text = reason && reason.stack ? reason.stack : String(reason); } catch (e) { text = '[unprintable]'; }
    push('error', 'Unhandled promise rejection: ' + text);
  });

  Object.defineProperty(window, '__LingXiaPageConsole', {
    configurable: false,
    enumerable: false,
    value: Object.freeze({
      // Entries after `since`, and how many were pushed out unread.
      take: function (since) {
        var out = [];
        for (var i = 0; i < entries.length; i++) {
          if (entries[i].seq > since) out.push(entries[i]);
        }
        return { entries: out, dropped: dropped, seq: seq };
      },
    }),
  });
})();
