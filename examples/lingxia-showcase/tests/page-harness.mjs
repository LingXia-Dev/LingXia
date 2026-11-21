import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const require = createRequire(new URL('../../../packages/lingxia-view/package.json', import.meta.url));
const { buildSync } = require('esbuild');

/** Bundle a page's Logic module; its `definePage` default lands on `__page`. */
export function bundlePage(entry) {
  return buildSync({
    entryPoints: [fileURLToPath(new URL(entry, import.meta.url))],
    bundle: true, write: false, platform: 'neutral', format: 'iife', globalName: '__page', logLevel: 'silent',
  }).outputFiles[0].text;
}

/**
 * Instantiate a bundled page as the Logic runtime does: `setup(page)` with
 * `data` / `update` / `signal`, returning its hooks and actions flattened onto
 * one object. `onWrite` sees every `update`.
 */
export function mountPage(source, globals, { onWrite } = {}) {
  const sandbox = { AbortController, ...globals };
  vm.runInNewContext(source, sandbox);
  const definition = sandbox.__page.default;
  const lifetime = new AbortController();
  let data = structuredClone(definition.data ?? {});
  let params = {};
  const api = {
    get data() { return data; },
    update(recipe) {
      onWrite?.();
      if (lifetime.signal.aborted) return;
      const draft = structuredClone(data);
      recipe(draft);
      data = draft;
    },
    async flush() {},
    get params() { return params; },
    name: 'page',
    signal: lifetime.signal,
    close() {},
    setResult() {},
    opener: undefined,
    surface: undefined,
  };
  const { actions = {}, onLoad, onUnload, ...hooks } = definition.setup ? definition.setup(api) : {};
  return {
    ...hooks,
    ...actions,
    get data() { return data; },
    onLoad(options = {}) { params = options; return onLoad?.(options); },
    onUnload() {
      try { return onUnload?.(); } finally { lifetime.abort(); }
    },
  };
}
