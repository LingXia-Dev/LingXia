import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

function runI18n(source, hostLanguage) {
  const listeners = new Set();
  const document = {
    readyState: 'complete',
    documentElement: { lang: '' },
    querySelectorAll: () => [],
    dispatchEvent: () => {},
    addEventListener: () => {},
  };
  const window = {
    LingXiaBridge: {
      displayLanguage: {
        get: () => hostLanguage.value,
        subscribe(listener) {
          listeners.add(listener);
          return () => listeners.delete(listener);
        },
      },
    },
    addEventListener: () => {},
    setTimeout,
  };

  vm.runInNewContext(source, {
    window,
    document,
    navigator: { languages: ['en-US'], language: 'en-US' },
    Date,
    setTimeout,
  });

  return {
    window,
    notify(next) {
      hostLanguage.value = next;
      for (const listener of [...listeners]) listener();
    },
  };
}

test('follows the product language with no locale of its own', async () => {
  const source = await readFile(new URL('../public/i18n.js', import.meta.url), 'utf8');
  const { window, notify } = runI18n(source, { value: 'en-US' });

  assert.equal(window.LingXiaI18n.locale, 'en-US');

  notify('zh-Hans-CN');
  assert.equal(window.LingXiaI18n.locale, 'zh-CN');

  // No screen-local override: nothing to set, nothing stored, no reload.
  assert.equal(window.LingXiaI18n.setLocale, undefined);
  assert.equal(window.LingXiaI18n.storageKey, undefined);
  assert.equal(window.localStorage, undefined);
  assert.equal(window.location, undefined);
});

test('settings selector edits the product preference only', async () => {
  const source = await readFile(new URL('../pages/settings/index.html', import.meta.url), 'utf8');
  assert.match(source, /app\.getDisplayLanguagePreference/);
  assert.match(source, /app\.watchDisplayLanguagePreference/);
  assert.match(source, /app\.setDisplayLanguagePreference/);
  assert.match(source, /configuredLanguage = preference/);
  // Rendering this page in the product language is the bridge's job, not the
  // selector's.
  assert.doesNotMatch(source, /i18n\.setLocale/);
  assert.doesNotMatch(source, /settings\.(?:getLanguage|setLanguage|watchLanguage)/);
});
