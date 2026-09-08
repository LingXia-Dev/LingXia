import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

export async function runBrowser(rootDir, exercise) {
const chrome = process.env.CHROME_BIN ?? [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser',
  `${process.env.PROGRAMFILES}/Google/Chrome/Application/chrome.exe`,
].find(existsSync);
assert.ok(chrome, 'Set CHROME_BIN to run the native component browser regression tests');
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
  return result.result.result.value;
} finally {
  ws?.close();
  child.kill();
  await new Promise(resolve => child.exitCode !== null ? resolve() : child.once('exit', resolve));
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}

}
