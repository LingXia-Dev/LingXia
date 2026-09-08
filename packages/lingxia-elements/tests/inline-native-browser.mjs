import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const chrome = process.env.CHROME_BIN ?? [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser',
  `${process.env.PROGRAMFILES}/Google/Chrome/Application/chrome.exe`,
].find(existsSync);
assert.ok(chrome, 'Set CHROME_BIN to run the native component browser regression tests');
const rootDir = fileURLToPath(new URL('../', import.meta.url));
const profile = await mkdtemp(path.join(tmpdir(), 'lingxia-native-browser-'));
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/') { res.setHeader('Content-Type', 'text/html'); res.end('<!doctype html><body></body>'); return; }
    const filename = path.resolve(rootDir, '.' + new URL(req.url, 'http://localhost').pathname);
    if (!filename.startsWith(rootDir)) { res.writeHead(403).end(); return; }
    res.setHeader('Content-Type', 'text/javascript');
    res.end(await readFile(filename));
  } catch { res.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const child = spawn(chrome, ['--headless', '--no-sandbox', '--disable-gpu', '--disable-background-networking',
  '--no-first-run', `--user-data-dir=${profile}`, '--remote-debugging-port=0', 'about:blank'], { stdio: 'ignore' });
let ws;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
try {
  let port;
  for (let i = 0; i < 100; i++) {
    try { port = Number((await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]); break; } catch { await sleep(100); }
  }
  assert.ok(port, 'Chrome did not start');
  const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  ws = new WebSocket(pages.find(page => page.type === 'page').webSocketDebuggerUrl);
  await new Promise(resolve => ws.addEventListener('open', resolve, { once: true }));
  let id = 0;
  const pending = new Map();
  ws.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    if (message.id) { pending.get(message.id)?.(message); pending.delete(message.id); }
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const key = ++id;
    const timeout = setTimeout(() => { pending.delete(key); reject(Error(`CDP timeout: ${method}`)); }, 15000);
    pending.set(key, message => { clearTimeout(timeout); resolve(message); });
    ws.send(JSON.stringify({ id: key, method, params }));
  });
  await send('Page.navigate', { url: `http://127.0.0.1:${server.address().port}` });
  await sleep(100);
  const result = await send('Runtime.evaluate', { expression: `(${exercise.toString()})()`, awaitPromise: true, returnByValue: true });
  assert.equal(result.result?.exceptionDetails, undefined, JSON.stringify(result.result?.exceptionDetails));
  assert.ok(result.result?.result?.value?.passed >= 20, JSON.stringify(result));
  console.log(`Native browser regressions: ${result.result.result.value.passed} passed`);
} finally {
  ws?.close();
  child.kill();
  await new Promise(resolve => child.exitCode !== null ? resolve() : child.once('exit', resolve));
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}

async function exercise() {
  const { registerInlineNativeAuthorComponents } = await import('/dist/inline-native/elements.js');
  const messages = [];
  const handlers = new Map();
  window.LingXiaBridge = { nativeComponents: {
    send: message => messages.push(structuredClone(message)),
    register: (id, handler) => { handlers.set(id, handler); return () => handlers.delete(id); },
  } };
  registerInlineNativeAuthorComponents();
  let passed = 0;
  const check = (condition, name) => { if (!condition) throw Error(name); passed++; };
  const settle = () => new Promise(resolve => setTimeout(resolve, 120));
  const root = document.createElement('lx-native-root');
  root.style.cssText = 'width:320px;height:180px';
  const button = document.createElement('lx-native-button');
  button.id = 'button'; button.label = 'Play'; button.icon = 'play';
  const text = document.createElement('lx-native-text');
  text.id = 'text'; text.textContent = 'Inherited text';
  const fallback = document.createElement('div');
  fallback.setAttribute('data-lx-native-fallback', '');
  fallback.setAttribute('aria-hidden', 'true'); fallback.hidden = true; fallback.textContent = 'Retry';
  root.append(button, text, fallback); document.body.append(root);
  await settle();
  check(root.lastCompileResult()?.ok, 'initial compilation');
  check(!fallback.hidden && !fallback.hasAttribute('aria-hidden'), 'initial fallback is accessible');
  let rect = button.getBoundingClientRect();
  check(rect.width > 30 && rect.height >= 26, 'label and icon have intrinsic dimensions');
  const oldWidth = rect.width;
  button.label = 'A much longer playback label'; await settle();
  check(button.getBoundingClientRect().width > oldWidth, 'label update remeasures button');
  button.style.width = '240px'; button.style.height = '48px'; await settle();
  check(button.getBoundingClientRect().width === 240 && button.getBoundingClientRect().height === 48, 'explicit button sizing');

  const rootMessage = messages.find(message => message.action === 'root.commit');
  const host = handlers.get(rootMessage.id);
  let ready = 0; root.addEventListener('ready', () => ready++);
  host({ action: 'root.leaseGranted', leaseId: 'lease', sequence: 1, leaseDurationMs: 8000 });
  host({ action: 'root.leaseActive' });
  check(ready === 1 && fallback.hidden, 'activation hides fallback and emits ready');
  host({ action: 'root.applied' }); host({ action: 'root.applied' }); host({ action: 'root.leaseActive' });
  check(ready === 1, 'commit acknowledgements and duplicate activation do not repeat ready');
  await settle();
  const latestGeometry = () => messages.filter(message => message.action === 'geometry.snapshot').at(-1);
  const before = latestGeometry();
  const banner = document.createElement('div'); banner.style.height = '100px';
  document.body.prepend(banner); await settle();
  const after = latestGeometry();
  check(after.revision > before.revision, 'external sibling invalidates native geometry');
  check(after.roots[0].contentRect.y === root.getBoundingClientRect().y + window.scrollY
    && after.roots[0].contentRect.y - before.roots[0].contentRect.y === 100, 'new document position is published');
  const movementStart = latestGeometry().revision;
  banner.animate([{ height: '100px' }, { height: '150px' }], { duration: 150, fill: 'forwards' });
  await new Promise(resolve => setTimeout(resolve, 250));
  check(latestGeometry().revision > movementStart, 'position changes without DOM mutations are observed');

  document.body.style.color = 'rgb(255, 0, 0)'; document.body.style.fontSize = '32px'; await settle();
  let props = root.lastCompileResult().root.children.find(node => node.authorId === 'text').props;
  check(props.color === 'rgb(255, 0, 0)' && props.fontSize === '32px', 'external inherited typography reaches native text');
  text.setAttribute('font-size', '48'); text.setAttribute('line-height', '60'); await settle();
  check(getComputedStyle(text).fontSize === '48px' && getComputedStyle(text).lineHeight === '60px', 'typography props affect CSS measurement');
  check(text.getBoundingClientRect().height > 32, 'text geometry grows with font prop');
  text.style.fontSize = '24px'; await settle();
  props = root.lastCompileResult().root.children.find(node => node.authorId === 'text').props;
  check(props.fontSize === '24px', 'CSS precedence matches native typography');
  text.removeAttribute('font-size'); text.style.removeProperty('font-size'); await settle();
  check(getComputedStyle(text).fontSize === '32px', 'removing typography prop restores inheritance');

  let presses = 0; button.addEventListener('press', () => presses++);
  button.id = 'renamed'; await settle();
  check(!handlers.has('button') && handlers.has('renamed'), 'id change migrates native handler');
  handlers.get('renamed')({ event: 'press', detail: { source: 'pointer' } });
  check(presses === 1, 'press delivered after id change');
  let focus = 0; button.addEventListener('focus', () => focus++);
  handlers.get('renamed')({ event: 'focus', detail: { source: 'keyboard' } });
  check(focus === 1, 'available platform events are not dropped by bridge');
  host({ action: 'root.error', message: 'test failure' });
  check(!fallback.hidden && !fallback.hasAttribute('aria-hidden'), 'failure fallback is accessible');
  await root.retry();
  host({ action: 'root.leaseGranted', leaseId: 'retry-lease', sequence: 1, leaseDurationMs: 8000 });
  host({ action: 'root.leaseActive' });
  check(ready === 2 && fallback.hidden, 'recovered root emits a new ready transition');
  root.remove(); await settle();
  check(!handlers.has('renamed') && !handlers.has(rootMessage.id), 'disconnect unregisters native handlers');
  const count = messages.length; button.label = 'Detached'; await settle();
  check(messages.length === count, 'disconnected root does not publish layout');
  return { passed };
}
