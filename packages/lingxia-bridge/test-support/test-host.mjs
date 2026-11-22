import assert from 'node:assert/strict';

// The host facts a View reads through useLxHost(): one object until a field
// changes, and a change never comes from the viewport's size alone.
globalThis.window = {
  __LX_BRIDGE_CFG: {
    // A host before the lower-case ids; the bridge reads it as `macos`.
    os: 'macOS',
    hostClass: 'desktop',
    displayLanguage: 'en-US',
    surfaceContext: { sizeClass: 'regular', width: 1200, height: 800, canDock: true },
    surfaceContextRevision: 1,
  },
};
// The runtime stamps <html lang> and <html dir> at load and on each change.
globalThis.document = { documentElement: {} };
const { getHost, subscribeHost, textDirection, registerCatalogues, t } = await import('../dist/test/host.mjs');
assert.equal(document.documentElement.lang, 'en-US');
assert.equal(document.documentElement.dir, 'ltr');

const first = getHost();
assert.deepEqual(first, {
  sizeClass: 'regular',
  canDock: true,
  language: 'en-US',
  formFactor: 'desktop',
  os: 'macos',
  runner: false,
});
assert.equal(getHost(), first, 'one object until a field changes');

let heard = 0;
const stop = subscribeHost(() => heard++);
assert.equal(heard, 0, 'change-only: nothing on subscribe');

window.__lingxiaApplySurfaceContext({ sizeClass: 'regular', width: 1000, height: 700, canDock: true }, 2);
assert.equal(heard, 0, 'a width change within the size class is not a host change');
assert.equal(getHost(), first);

window.__lingxiaApplySurfaceContext({ sizeClass: 'compact', width: 500, height: 700, canDock: true }, 3);
assert.equal(heard, 1, 'crossing the size class is');
assert.equal(getHost().sizeClass, 'compact');
assert.notEqual(getHost(), first, 'and a change is a new object');

window.__lingxiaApplyDisplayLanguage('ar-EG');
assert.equal(heard, 2, 'a language change is');
assert.equal(getHost().language, 'ar-EG');
assert.equal(document.documentElement.lang, 'ar-EG');
assert.equal(document.documentElement.dir, 'rtl', 'a right-to-left language turns the page');

stop();
window.__lingxiaApplyDisplayLanguage('en-US');
assert.equal(heard, 2, 'unsubscribed listeners stay quiet');

// A document that loaded during a change gets the change and, once its bridge
// is ready, the current value; either may land last, and the newer revision
// wins.
let languages = [];
const stopLanguages = subscribeHost(() => languages.push(getHost().language));
window.__lingxiaApplyDisplayLanguage('zh-CN', 5);
assert.deepEqual(languages, ['zh-CN']);
window.__lingxiaApplyDisplayLanguage('en-US', 3);
assert.equal(getHost().language, 'zh-CN', 'an older revision never wins');
window.__lingxiaApplyDisplayLanguage('zh-CN', 5);
assert.deepEqual(languages, ['zh-CN'], 'a repeat of the current revision changes nothing');
window.__lingxiaApplyDisplayLanguage('fr-FR', 6);
assert.equal(getHost().language, 'fr-FR');
window.__lingxiaApplyDisplayLanguage('en-US');
assert.equal(getHost().language, 'en-US', 'a push without a revision still applies');
stopLanguages();

// An lxapp written in fewer languages than the user reads narrows the display
// language; <html lang> and the copy follow the narrowing. The page's own push
// repeats the broadcast's revision to add it.
window.__lingxiaApplyDisplayLanguage('zh-CN', 7);
assert.equal(getHost().language, 'zh-CN', 'without a narrowing, content follows');
window.__lingxiaApplyDisplayLanguage('zh-CN', 7, 'en');
assert.equal(getHost().language, 'en');
assert.equal(document.documentElement.lang, 'en');
window.__lingxiaApplyDisplayLanguage('zh-CN', 7);
assert.equal(getHost().language, 'en', 'the bare broadcast never undoes the narrowing');
window.__lingxiaApplyDisplayLanguage('ja-JP', 8);
assert.equal(getHost().language, 'en', 'a narrowed lxapp waits for its page push');
window.__lingxiaApplyDisplayLanguage('ar-EG', 9, 'ar');
assert.equal(document.documentElement.dir, 'rtl', 'direction comes from the content language');

// The runtime stamps <html dir> with <html lang>.
assert.equal(textDirection('ar-EG'), 'rtl');
assert.equal(textDirection('he'), 'rtl');
assert.equal(textDirection('fa-IR'), 'rtl');
assert.equal(textDirection('zh-CN'), 'ltr');
assert.equal(textDirection('en-US'), 'ltr');
assert.equal(textDirection('not a tag'), 'ltr');

// `t` reads the page's language at the call and narrows it to a catalogue; a key only the source
// has falls back to it.
assert.throws(() => t('greet'), /no i18n\/ catalogues/);
registerCatalogues({
  source: 'en-US',
  formatter: (tag) => ({ tag }),
  languages: {
    'en-US': { greet: (a, f) => `Hello ${a.name} (${f.tag})`, only: () => 'source only' },
    'zh-CN': { greet: (a, f) => `你好 ${a.name} (${f.tag})` },
  },
});
window.__lingxiaApplyDisplayLanguage('zh-TW', 10, 'zh-TW');
assert.equal(t('greet', { name: 'A' }), '你好 A (zh-CN)', 'same language when the tag is not shipped');
assert.equal(t('only'), 'source only');
window.__lingxiaApplyDisplayLanguage('fr-FR', 11, 'fr-FR');
assert.equal(t('greet', { name: 'A' }), 'Hello A (en-US)');
assert.throws(() => t('missing'), /no message 'missing'/);

console.log('host store: ok');
