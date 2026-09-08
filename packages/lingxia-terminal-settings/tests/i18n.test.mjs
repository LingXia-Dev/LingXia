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
    document,
    notify(next) {
      hostLanguage.value = next;
      for (const listener of [...listeners]) listener();
    },
  };
}

test('follows the product language and has no screen-local override', async () => {
  const source = await readFile(new URL('../public/i18n.js', import.meta.url), 'utf8');
  const hostLanguage = { value: 'en-US' };
  const { window, document, notify } = runI18n(source, hostLanguage);

  assert.equal(window.LingXiaI18n.locale, 'en-US');
  assert.equal(document.documentElement.lang, 'en');

  notify('zh-Hans-CN');
  assert.equal(window.LingXiaI18n.locale, 'zh-CN');
  assert.equal(document.documentElement.lang, 'zh-Hans');

  // The product owns the language: this screen exposes no way to pick one and
  // stores nothing of its own.
  assert.equal(window.LingXiaI18n.setLocale, undefined);
  assert.equal(window.LingXiaI18n.followApp, undefined);
  assert.equal(window.LingXiaI18n.storageKey, undefined);
  assert.equal(window.localStorage, undefined);
});

test('narrows a language it has no catalog for instead of dropping strings', async () => {
  const source = await readFile(new URL('../public/i18n.js', import.meta.url), 'utf8');
  const { window } = runI18n(source, { value: 'ja-JP' });

  assert.equal(window.LingXiaI18n.locale, 'en-US');
  assert.equal(window.LingXiaI18n.t('app.title'), 'Terminal');
});
