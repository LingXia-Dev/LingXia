import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

test('follows the shared effective-language route and stream', async () => {
  const source = await readFile(new URL('../public/i18n.js', import.meta.url), 'utf8');
  const invocations = [];
  const streams = [];
  const storage = new Map();
  let reloads = 0;
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
    location: { reload: () => { reloads += 1; } },
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
  assert.equal(reloads, 1);
});

test('settings selector edits preference while rendering effective language', async () => {
  const source = await readFile(new URL('../pages/settings/index.html', import.meta.url), 'utf8');
  assert.match(source, /app\.getDisplayLanguageState/);
  assert.match(source, /app\.watchDisplayLanguageState/);
  assert.match(source, /app\.setDisplayLanguagePreference/);
  assert.match(source, /configuredLanguage = state\.preference/);
  assert.match(source, /i18n\.setLocale\(state\.effective\)/);
  assert.doesNotMatch(source, /settings\.(?:getLanguage|setLanguage|watchLanguage)/);
});
