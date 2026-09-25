import assert from 'node:assert/strict';

// The host facts a View reads through useLxHost(): one object until a field
// changes, and a change never comes from the viewport's size alone.
globalThis.window = {
  __LX_BRIDGE_CFG: {
    os: 'macOS',
    hostClass: 'desktop',
    displayLanguage: 'en-US',
    surfaceContext: { sizeClass: 'regular', width: 1200, height: 800, aside: true },
    surfaceContextRevision: 1,
  },
};
// The runtime stamps <html lang> and <html dir> at load and on each change.
globalThis.document = { documentElement: {} };
const { getHost, subscribeHost, textDirection } = await import('../dist/test/host.mjs');
assert.equal(document.documentElement.lang, 'en-US');
assert.equal(document.documentElement.dir, 'ltr');

const first = getHost();
assert.deepEqual(first, {
  sizeClass: 'regular',
  aside: true,
  displayLanguage: 'en-US',
  formFactor: 'desktop',
  os: 'macOS',
  runner: false,
});
assert.equal(getHost(), first, 'one object until a field changes');

let heard = 0;
const stop = subscribeHost(() => heard++);
assert.equal(heard, 0, 'change-only: nothing on subscribe');

window.__lingxiaApplySurfaceContext({ sizeClass: 'regular', width: 1000, height: 700, aside: true }, 2);
assert.equal(heard, 0, 'a width change within the size class is not a host change');
assert.equal(getHost(), first);

window.__lingxiaApplySurfaceContext({ sizeClass: 'compact', width: 500, height: 700, aside: true }, 3);
assert.equal(heard, 1, 'crossing the size class is');
assert.equal(getHost().sizeClass, 'compact');
assert.notEqual(getHost(), first, 'and a change is a new object');

window.__lingxiaApplyDisplayLanguage('ar-EG');
assert.equal(heard, 2, 'a language change is');
assert.equal(getHost().displayLanguage, 'ar-EG');
assert.equal(document.documentElement.lang, 'ar-EG');
assert.equal(document.documentElement.dir, 'rtl', 'a right-to-left language turns the page');

stop();
window.__lingxiaApplyDisplayLanguage('en-US');
assert.equal(heard, 2, 'unsubscribed listeners stay quiet');

// A document that loaded during a change gets the change and, once its bridge
// is ready, the current value; either may land last, and the newer revision
// wins.
let languages = [];
const stopLanguages = subscribeHost(() => languages.push(getHost().displayLanguage));
window.__lingxiaApplyDisplayLanguage('zh-CN', 5);
assert.deepEqual(languages, ['zh-CN']);
window.__lingxiaApplyDisplayLanguage('en-US', 3);
assert.equal(getHost().displayLanguage, 'zh-CN', 'an older revision never wins');
window.__lingxiaApplyDisplayLanguage('zh-CN', 5);
assert.deepEqual(languages, ['zh-CN'], 'a repeat of the current revision changes nothing');
window.__lingxiaApplyDisplayLanguage('fr-FR', 6);
assert.equal(getHost().displayLanguage, 'fr-FR');
window.__lingxiaApplyDisplayLanguage('en-US');
assert.equal(getHost().displayLanguage, 'en-US', 'a push without a revision still applies');
stopLanguages();

// The runtime stamps <html dir> with <html lang>.
assert.equal(textDirection('ar-EG'), 'rtl');
assert.equal(textDirection('he'), 'rtl');
assert.equal(textDirection('fa-IR'), 'rtl');
assert.equal(textDirection('zh-CN'), 'ltr');
assert.equal(textDirection('en-US'), 'ltr');
assert.equal(textDirection('not a tag'), 'ltr');

console.log('host store: ok');
