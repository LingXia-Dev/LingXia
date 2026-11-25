// A component library's word in Logic, as `packageText` in a View reads it: the lxapp's override in
// that language's catalogue (looked up natively), else the package's language, else its source.
export function packageText(words, key, args, language) {
  if (typeof language !== "string" || !language) throw new TypeError(`packageText('${key}'): pass the caller's language`);
  const own = globalThis.__lingxiaAppText?.(key, language, JSON.stringify(args ?? {}));
  if (typeof own === "string") return own;
  const primary = language.split(/[-_]/)[0].toLowerCase();
  const tag = Object.keys(words.languages).find(tag => tag.toLowerCase().split("-")[0] === primary) ?? words.source;
  const message = words.languages[tag]?.[key] ?? words.languages[words.source]?.[key];
  if (!message) throw new Error(`packageText: no package word '${key}'`);
  const text = message(args ?? {}, words.formatter(tag));
  return Array.isArray(text) ? text.join("") : String(text);
}
