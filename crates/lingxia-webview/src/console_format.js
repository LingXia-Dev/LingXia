// Console arguments as one line of text. Must never throw into the page
// (circular objects, errors, revoked proxies).
function formatConsoleArgs(args) {
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
