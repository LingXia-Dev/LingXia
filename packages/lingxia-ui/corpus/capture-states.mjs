// Captures each page's initial Logic `data` from a built lxapp (`dist/logic.js`) into
// corpus/states/<app>/<page>.json, so corpus runs need no Logic. Rerun after changing a page's data.
//   node corpus/capture-states.mjs <lxapp dir> <app name>
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import vm from 'node:vm';

const [root, app] = process.argv.slice(2);
const source = await readFile(join(root, 'dist/logic.js'), 'utf8');
const inert = new Proxy(function () {}, { get: () => inert, apply: () => inert, construct: () => inert });
let registered;
const pages = {};
const context = vm.createContext({
  lx: inert, console, setTimeout, clearTimeout, AbortController, URL, TextEncoder, TextDecoder,
  getApp: () => registered,
  __registerApp: definition => { registered = definition; },
  __registerPage: (path, definition) => { pages[path.replace(/\/index\.js$/, '').replace(/^pages\//, '')] = definition.data ?? {}; },
});
context.globalThis = context;
vm.runInContext(source, context);
const dir = new URL(`./states/${app}/`, import.meta.url);
await mkdir(dir, { recursive: true });
for (const [page, data] of Object.entries(pages))
  await writeFile(new URL(`${page}.json`, dir), JSON.stringify(data, null, 2) + '\n');
console.log(`captured ${Object.keys(pages).length} page states for ${app}`);
