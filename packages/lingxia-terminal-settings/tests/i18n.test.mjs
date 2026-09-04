import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

test('follows the shared effective-language route and stream', async () => {
  const source = await readFile(new URL('../public/i18n.js', import.meta.url), 'utf8');
  const invocations = [];
  const streams = [];
  const storage = new Map();
  const document = {
    readyState: 'complete',
    documentElement: { lang: '' },
    querySelectorAll: () => [],
    dispatchEvent: () => {},
    addEventListener: () => {},
  };
  const window = {
    LingXiaBridge: {
      invoke(route) {
        invocations.push(route);
        return Promise.resolve('en-US');
      },
      stream(route) {
        const stream = {
          route,
          onEvent(callback) { stream.event = callback; },
          onError(callback) { stream.error = callback; },
        };
        streams.push(stream);
        return stream;
      },
    },
    localStorage: {
      getItem: (key) => storage.get(key) ?? null,
      setItem: (key, value) => storage.set(key, value),
      removeItem: (key) => storage.delete(key),
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
  await Promise.resolve();

  assert.deepEqual(invocations, ['app.getDisplayLanguage']);
  assert.equal(streams[0].route, 'app.watchDisplayLanguage');
  streams[0].event('zh-Hans-CN');
  assert.equal(window.LingXiaI18n.locale, 'zh-CN');
  assert.equal(document.documentElement.lang, 'zh-Hans');
});
