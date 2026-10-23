#!/usr/bin/env node
// Browser agent conformance: drives `lxdev browser …` against the fixture
// pages in a running app session and checks snapshots, ref actions,
// actionability failures, form input, text, console and screenshots. The
// same suite runs against macOS (WKWebView) and Windows (WebView2).
//
//   node run.mjs [--session NAME] [--lxdev PATH] [--only SUBSTRING]

import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const argv = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : fallback;
};
const LXDEV = flag('--lxdev', 'lxdev');
const SESSION = flag('--session', process.env.LXDEV_SESSION);
const ONLY = flag('--only', null);
const fixtures = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');

// Fixtures over loopback HTTP: a real origin, and `localhost` vs
// `127.0.0.1` gives the page a genuinely cross-origin iframe. The server is
// its own process: the lxdev calls below block this one.
const SERVER = `
const http = require('http'), fs = require('fs'), path = require('path');
const dir = process.argv[1];
const server = http.createServer((req, res) => {
  const name = new URL(req.url, 'http://x').pathname.replace(/^\\/+/, '') || 'form.html';
  try {
    const body = fs.readFileSync(path.join(dir, name.replace(/[^a-z0-9._-]/gi, '')));
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    res.end(body);
  } catch (_) { res.writeHead(404); res.end('not found'); }
});
server.listen(0, '127.0.0.1', () => console.log(server.address().port));
`;
const server = spawn(process.execPath, ['-e', SERVER, fixtures], { stdio: ['ignore', 'pipe', 'inherit'] });
const port = await new Promise((resolve, reject) => {
  server.stdout.once('data', (chunk) => resolve(String(chunk).trim()));
  server.once('exit', (code) => reject(new Error(`fixture server exited (${code})`)));
});
const FORM_URL = `http://127.0.0.1:${port}/form.html`;

class CommandFailed extends Error {
  constructor(args, error) {
    super(`lxdev ${args.join(' ')} failed: ${error.code} ${error.message}`);
    this.code = error.code;
    this.data = error.data || {};
  }
}

function lxdev(args, { json = true } = {}) {
  const full = ['browser', ...args];
  if (SESSION) full.push('--session', SESSION);
  if (json) full.push('--json');
  try {
    const out = execFileSync(LXDEV, full, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 256 * 1024 * 1024 });
    if (!json) return out;
    const trimmed = out.trim();
    return trimmed ? JSON.parse(trimmed) : null;
  } catch (err) {
    const stderr = String(err.stderr || '');
    let envelope = null;
    try { envelope = JSON.parse(stderr.trim().split('\n').pop()); } catch (_) {}
    const domain = envelope && envelope.error && envelope.error.domain;
    throw new CommandFailed(full, domain || { code: 'command_failed', message: stderr.trim() || String(err) });
  }
}

let TAB = 'current';
const tabArgs = () => ['--tab', TAB];
const evaluate = (js) => lxdev(['eval', ...tabArgs(), '--js', js]);
const snapshotText = (extra = []) => lxdev(['snapshot', ...tabArgs(), ...extra], { json: false });

function refOf(text, role, name) {
  const escaped = JSON.stringify(name).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = new RegExp(`- ${role} ${escaped}[^\\n]*\\[ref=(e\\d+)\\]`).exec(text);
  if (!match) throw new Error(`no ${role} "${name}" ref in snapshot:\n${text.slice(0, 3000)}`);
  return match[1];
}

function expectFailure(kind, args) {
  try {
    lxdev(args);
  } catch (err) {
    if (!(err instanceof CommandFailed)) throw err;
    if (err.code !== kind) throw new Error(`expected ${kind}, got ${err.code}: ${err.message}`);
    return err.data;
  }
  throw new Error(`expected ${kind}, but the command succeeded`);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function pngSize(response) {
  const data = response.image && response.image.data;
  const bytes = Buffer.from(data || '', 'base64');
  assert(bytes.slice(1, 4).toString() === 'PNG', 'not a PNG');
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------

const cases = [];
const test = (name, fn) => cases.push({ name, fn });
let snap = '';
const ref = (role, name) => refOf(snap, role, name);
const reload = async () => {
  lxdev(['open', FORM_URL, ...tabArgs()]);
  lxdev(['wait', ...tabArgs(), '--loaded']);
  await sleep(900); // the fixture enables and adds elements after 700ms
  snap = snapshotText();
};

test('snapshot outlines roles, names, states and refs', async () => {
  const opened = lxdev(['open', FORM_URL]);
  TAB = opened.tab_id;
  lxdev(['wait', ...tabArgs(), '--loaded']);
  snap = snapshotText();
  for (const needle of [
    'heading "Agent conformance" [ref=',
    '[level=1]',
    'textbox "Full name" [ref=',
    '[required]',
    'combobox "Country" [ref=',
    'option "Japan"',
    'checkbox "I agree" [ref=',
    'radio "Free" [ref=',
    '[checked]',
    'switch "Dark mode" [ref=',
    'button "Disabled" [ref=',
    '[disabled]',
    'link "Next page" [ref=',
    'navigation "Primary"',
    'textbox "Read only" [ref=',
    '[readonly]',
    'region "Named section"',
    'paragraph "An unnamed section."',
  ]) {
    assert(snap.includes(needle), `snapshot lacks ${needle}`);
  }
});

test('snapshot reaches open shadow roots and same-origin iframes', async () => {
  ref('button', 'Shadow button');
  ref('textbox', 'Shadow input');
  ref('button', 'Inner button');
  assert(/iframe "Cross origin"[^\n]*\[cross-origin: contents unavailable\]/.test(snap), 'cross-origin iframe not marked');
});

test('snapshot marks off-screen elements and masks passwords', async () => {
  await reload();
  lxdev(['fill', ...tabArgs(), '--ref', ref('textbox', 'Password'), '--text', 'secret']);
  snap = snapshotText();
  assert(!snap.includes('secret'), 'password value leaked into the snapshot');
  assert(/link "Bottom"[^\n]*\n/.test(snap), 'bottom link missing');
  const json = lxdev(['snapshot', ...tabArgs(), '--format', 'json']);
  const flat = [];
  const walk = (nodes) => nodes.forEach((n) => { flat.push(n); if (n.children) walk(n.children); });
  walk(json.nodes);
  assert(flat.some((n) => n.offscreen), 'no node marked offscreen');
});

test('snapshot JSON form, subtree root and budget', async () => {
  const json = lxdev(['snapshot', ...tabArgs(), '--format', 'json']);
  assert(Array.isArray(json.nodes) && json.ref_count > 10, 'json snapshot missing nodes/refs');
  assert(typeof json.generation === 'string', 'no generation');
  const sub = snapshotText(['--css', '#profile']);
  assert(sub.includes('form "Profile"') && !sub.includes('Readable content'), 'subtree snapshot not scoped');
  const small = lxdev(['snapshot', ...tabArgs(), '--max-nodes', '5']);
  assert(small.truncated === true && small.text.includes('truncated'), 'budget not enforced');
});

test('snapshot --since-last reports only what changed', async () => {
  await reload();
  lxdev(['fill', ...tabArgs(), '--ref', ref('textbox', 'Full name'), '--text', 'Ada']);
  const diff = lxdev(['snapshot', ...tabArgs(), '--since-last']);
  assert(diff.diff && !diff.diff.unchanged, 'diff missing');
  assert(diff.diff.added.some((l) => l.includes('"Ada"')), 'diff does not show the new value');
  assert(diff.text === undefined, 'diff returned the full text');
  const again = lxdev(['snapshot', ...tabArgs(), '--since-last']);
  assert(again.diff.unchanged === true, 'second diff not unchanged');
});

test('action results can carry the snapshot diff', async () => {
  await reload();
  const result = lxdev(['click', ...tabArgs(), '--ref', ref('checkbox', 'I agree'), '--snapshot', 'diff']);
  assert(result.snapshot && result.snapshot.diff, 'no snapshot diff in action result');
  assert(result.snapshot.diff.added.some((l) => l.includes('checkbox "I agree"') && l.includes('[checked]')), 'diff lacks the checked box');
});

test('fill fires input/change and drives controlled inputs', async () => {
  await reload();
  lxdev(['fill', ...tabArgs(), '--ref', ref('textbox', 'Controlled'), '--text', 'kept']);
  lxdev(['click', ...tabArgs(), '--ref', ref('textbox', 'Full name')]);
  const state = evaluate("({ v: document.getElementById('controlled').value, s: state.controlled, e: state.events })");
  assert(state.v === 'kept' && state.s === 'kept', `controlled input lost its value: ${JSON.stringify(state)}`);
  assert(state.e.includes('input:controlled') && state.e.includes('change:controlled'), 'events missing');
});

test('fill sets date, time, number, range, color and contenteditable', async () => {
  const cases = [['Date', '2026-10-22'], ['Time', '09:30'], ['Count', '42'], ['Volume', '7']];
  for (const [name, value] of cases) {
    const role = name === 'Count' ? 'spinbutton' : name === 'Volume' ? 'slider' : 'textbox';
    lxdev(['fill', ...tabArgs(), '--ref', ref(role, name), '--text', value]);
  }
  lxdev(['fill', ...tabArgs(), '--css', '#color', '--text', '#ff0000']);
  lxdev(['fill', ...tabArgs(), '--ref', ref('textbox', 'Editor'), '--text', 'Hello editor']);
  const values = evaluate("['date','time','count','volume','color'].map(id => document.getElementById(id).value).concat(document.getElementById('editor').innerText)");
  assert(JSON.stringify(values) === JSON.stringify(['2026-10-22', '09:30', '42', '7', '#ff0000', 'Hello editor']), `values: ${JSON.stringify(values)}`);
  const data = expectFailure('usage', ['fill', ...tabArgs(), '--ref', ref('textbox', 'Date'), '--text', 'not a date']);
  assert(data.hint === 'YYYY-MM-DD', 'no format hint');
});

test('type appends, clears, and commits CJK through a composition', async () => {
  await reload();
  const email = ref('textbox', 'Email');
  lxdev(['type', ...tabArgs(), '--ref', email, '--text', 'a@b']);
  lxdev(['type', ...tabArgs(), '--ref', email, '--text', '.co']);
  lxdev(['type', ...tabArgs(), '--ref', ref('textbox', 'Bio'), '--text', 'old']);
  lxdev(['type', ...tabArgs(), '--ref', ref('textbox', 'Bio'), '--text', 'new', '--clear']);
  lxdev(['type', ...tabArgs(), '--ref', ref('textbox', 'IME'), '--text', '你好']);
  const s = evaluate("({ email: document.getElementById('email').value, bio: document.getElementById('bio').value, ime: document.getElementById('ime').value, comp: state.composition })");
  assert(s.email === 'a@b.co', `email ${s.email}`);
  assert(s.bio === 'new', `bio ${s.bio}`);
  assert(s.ime === '你好', `ime ${s.ime}`);
  assert(s.comp.includes('compositionend:你好'), `composition events: ${JSON.stringify(s.comp)}`);
});

test('select_option by label, value, index and multiple', async () => {
  await reload();
  const country = ref('combobox', 'Country');
  let r = lxdev(['select-option', ...tabArgs(), '--ref', country, '--option', 'Japan']);
  assert(r.selected[0].value === 'jp', 'label match failed');
  r = lxdev(['select-option', ...tabArgs(), '--ref', country, '--option', 'cn']);
  assert(r.selected[0].value === 'cn', 'value match failed');
  r = lxdev(['select-option', ...tabArgs(), '--ref', country, '--option-index', '2']);
  assert(r.selected[0].value === 'jp', 'index match failed');
  r = lxdev(['select-option', ...tabArgs(), '--ref', ref('listbox', 'Languages'), '--option', 'zh', '--option', 'French']);
  assert(r.selected.map((o) => o.value).join() === 'zh,fr', 'multiple select failed');
  expectFailure('disabled', ['select-option', ...tabArgs(), '--ref', country, '--option', 'Korea']);
  const missing = expectFailure('not_found', ['select-option', ...tabArgs(), '--ref', country, '--option', 'Atlantis']);
  assert(Array.isArray(missing.available) && missing.available.length === 4, 'available options not reported');
  const events = evaluate('state.events');
  assert(events.includes('change:country') && events.includes('input:country'), 'select events missing');
});

test('check and uncheck are idempotent and reach styled controls', async () => {
  await reload();
  const agree = ref('checkbox', 'I agree');
  let r = lxdev(['check', ...tabArgs(), '--ref', agree]);
  assert(r.checked === true && r.changed === true, 'check failed');
  r = lxdev(['check', ...tabArgs(), '--ref', agree]);
  assert(r.changed === false, 'check not idempotent');
  r = lxdev(['uncheck', ...tabArgs(), '--ref', agree]);
  assert(r.checked === false, 'uncheck failed');
  r = lxdev(['check', ...tabArgs(), '--ref', ref('radio', 'Pro')]);
  assert(r.checked === true, 'radio check failed');
  expectFailure('usage', ['uncheck', ...tabArgs(), '--ref', ref('radio', 'Pro')]);
  const started = Date.now();
  r = lxdev(['check', ...tabArgs(), '--css', '#styled']);
  assert(r.checked === true, 'styled checkbox not checked through its label');
  assert(Date.now() - started < 3000, 'styled checkbox waited out the timeout before using its label');
  r = lxdev(['check', ...tabArgs(), '--ref', ref('switch', 'Dark mode')]);
  assert(r.checked === true, 'switch not toggled');
});

test('set_input_files attaches files without a chooser, even to hidden inputs', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'lx-agent-'));
  const a = join(dir, 'a.txt');
  const b = join(dir, 'b.png');
  writeFileSync(a, 'hello');
  writeFileSync(b, Buffer.from('89504e470d0a1a0a', 'hex'));
  const r = lxdev(['set-input-files', ...tabArgs(), '--css', '#files', a, b]);
  assert(r.files.map((f) => f.name).join() === 'a.txt,b.png', `files ${JSON.stringify(r.files)}`);
  assert(r.files[0].size === 5 && r.files[1].type === 'image/png', 'file size/type wrong');
  const hidden = lxdev(['set-input-files', ...tabArgs(), '--css', '#hidden-file', a]);
  assert(hidden.files.length === 1, 'hidden file input not set');
  expectFailure('usage', ['set-input-files', ...tabArgs(), '--css', '#hidden-file', a, b]);
});

test('actionability failures are typed', async () => {
  await reload();
  const disabled = expectFailure('disabled', ['click', ...tabArgs(), '--ref', ref('button', 'Disabled'), '--timeout-ms', '300']);
  assert(disabled.timed_out === true && disabled.target, 'disabled failure lacks details');
  const covered = expectFailure('covered', ['click', ...tabArgs(), '--css', '#covered', '--timeout-ms', '300']);
  assert(covered.covered_by && covered.covered_by.selector === 'div#overlay' && /^e\d+$/.test(covered.covered_by.ref), `covered_by ${JSON.stringify(covered.covered_by)}`);
  expectFailure('not_visible', ['click', ...tabArgs(), '--css', '#hidden-btn', '--timeout-ms', '300']);
  expectFailure('not_editable', ['fill', ...tabArgs(), '--ref', ref('textbox', 'Read only'), '--text', 'x', '--timeout-ms', '300']);
  expectFailure('not_found', ['click', ...tabArgs(), '--css', '#nope', '--timeout-ms', '300']);
  expectFailure('usage', ['click', ...tabArgs(), '--css', '#[', '--timeout-ms', '300']);
  expectFailure('stale_ref', ['click', ...tabArgs(), '--ref', 'e99999']);
});

test('actions wait for elements to appear, enable and stop moving', async () => {
  lxdev(['open', FORM_URL, ...tabArgs()]);
  lxdev(['click', ...tabArgs(), '--css', '#late']);
  lxdev(['click', ...tabArgs(), '--css', '#late-enabled']);
  lxdev(['click', ...tabArgs(), '--css', '#mover']);
  const clicks = evaluate('state.clicks');
  assert(clicks.late === 1 && clicks['late-enabled'] === 1 && clicks.mover === 1, `clicks ${JSON.stringify(clicks)}`);
});

test('refs act inside shadow roots and same-origin iframes', async () => {
  await reload();
  lxdev(['click', ...tabArgs(), '--ref', ref('button', 'Shadow button')]);
  lxdev(['click', ...tabArgs(), '--ref', ref('button', 'Inner button')]);
  lxdev(['fill', ...tabArgs(), '--ref', ref('textbox', 'Inner input'), '--text', 'framed']);
  const s = evaluate("({ clicks: state.clicks, framed: document.getElementById('same').contentDocument.getElementById('inner-input').value })");
  assert(s.clicks.shadow === 1 && s.clicks.inner === 1, `clicks ${JSON.stringify(s.clicks)}`);
  assert(s.framed === 'framed', 'iframe input not filled');
});

test('hover, drag, press and scroll_to', async () => {
  await reload();
  lxdev(['hover', ...tabArgs(), '--ref', ref('button', 'Hover me')]);
  lxdev(['drag', ...tabArgs(), '--css', '#drag-src', '--to-css', '#drop-zone']);
  const name = ref('textbox', 'Full name');
  lxdev(['fill', ...tabArgs(), '--ref', name, '--text', 'Adam']);
  lxdev(['press', ...tabArgs(), '--ref', name, '--key', 'Backspace']);
  const r = lxdev(['scroll-to', ...tabArgs(), '--css', '#bottom']);
  const s = evaluate("({ hover: state.hover, dropped: state.dropped, name: document.getElementById('name').value, h: innerHeight })");
  assert(s.hover >= 1, 'hover not delivered');
  assert(s.dropped === true, 'drop not delivered');
  assert(s.name === 'Ada', `press Backspace: ${s.name}`);
  assert(r.rect.y >= 0 && r.rect.y < s.h, 'scroll_to left the element off screen');
});

test('stale refs say why: removed or navigated', async () => {
  await reload();
  const agree = ref('checkbox', 'I agree');
  evaluate("document.getElementById('agree').closest('label').remove(), true");
  const removed = expectFailure('stale_ref', ['check', ...tabArgs(), '--ref', agree]);
  assert(removed.reason === 'removed', `reason ${removed.reason}`);
  const name = ref('textbox', 'Full name');
  lxdev(['click', ...tabArgs(), '--ref', ref('link', 'Next page'), '--wait-navigation', '--complete']);
  const navigated = expectFailure('stale_ref', ['fill', ...tabArgs(), '--ref', name, '--text', 'x']);
  assert(navigated.reason === 'navigated', `reason ${navigated.reason}`);
  // Once the new page has issued refs of its own, an old one still cannot
  // reach one of them.
  const next = snapshotText();
  assert(next.includes('textbox "Search"'), 'second page not snapshotted');
  const again = expectFailure('stale_ref', ['fill', ...tabArgs(), '--ref', name, '--text', 'x']);
  assert(again.reason === 'navigated', `reason after new refs ${again.reason}`);
});

test('a navigating action reports the page it led to', async () => {
  await reload();
  const r = lxdev(['click', ...tabArgs(), '--ref', ref('link', 'Next page'), '--wait-navigation', '--complete', '--snapshot', 'full']);
  assert(r.snapshot && r.snapshot.navigated === true, 'navigation not reported');
  assert(r.snapshot.text.includes('heading "Second page"'), `snapshot is not of the new page:\n${r.snapshot.text}`);
});

test('text returns readable Markdown', async () => {
  await reload();
  const t = lxdev(['text', ...tabArgs()]).markdown;
  for (const needle of ['## Readable content', '[link](https://example.com/doc)', '**bold**', '- First', '  - Nested', '1. One', '| Key | Value |', '```']) {
    assert(t.includes(needle), `markdown lacks ${needle}:\n${t}`);
  }
  assert(!t.includes('Footer text'), 'page chrome leaked into main text');
  assert(lxdev(['text', ...tabArgs(), '--all']).markdown.includes('Footer text'), '--all lacks the footer');
  const small = lxdev(['text', ...tabArgs(), '--max-chars', '20']);
  assert(small.truncated && small.markdown.includes('truncated'), 'text budget not enforced');
});

test('console captures logs and uncaught errors', async () => {
  lxdev(['console', ...tabArgs(), '--clear']);
  lxdev(['click', ...tabArgs(), '--css', '#log-btn']);
  lxdev(['click', ...tabArgs(), '--css', '#throw-btn']);
  await sleep(300);
  const list = lxdev(['console', ...tabArgs()]);
  const texts = list.entries.map((e) => `${e.level}:${e.text}`);
  assert(texts.some((t) => t.includes('hello from page')), `no log: ${JSON.stringify(texts)}`);
  assert(texts.some((t) => t.startsWith('error:') && t.includes('page error')), `no error: ${JSON.stringify(texts)}`);
  assert(texts.some((t) => t.startsWith('error:Uncaught') && t.includes('boom')), `no uncaught error: ${JSON.stringify(texts)}`);
  const newer = lxdev(['console', ...tabArgs(), '--since', String(list.next_seq)]);
  assert(newer.entries.length === 0, 'since did not filter');
  const errors = lxdev(['console', ...tabArgs(), '--level', 'error']);
  assert(errors.entries.every((e) => e.level === 'error'), 'level filter failed');
});

test('screenshots: viewport, full page, element, annotated', async () => {
  await reload();
  const viewport = pngSize(lxdev(['screenshot', ...tabArgs()]));
  const full = pngSize(lxdev(['screenshot', ...tabArgs(), '--full-page']));
  assert(full.height > viewport.height, `full page ${full.height} not taller than viewport ${viewport.height}`);
  const element = lxdev(['screenshot', ...tabArgs(), '--ref', ref('heading', 'Agent conformance')]);
  const elementSize = pngSize(element);
  assert(elementSize.width < viewport.width && elementSize.height < viewport.height / 2, 'element capture not cropped');
  const annotated = lxdev(['screenshot', ...tabArgs(), '--annotate']);
  pngSize(annotated);
  assert(Array.isArray(annotated.marks) && annotated.marks.length > 5, 'no marks');
  assert(annotated.marks.every((m) => /^e\d+$/.test(m.ref) && m.box.width > 0), 'bad mark');
  const leftover = evaluate("document.querySelectorAll('lingxia-agent-marks').length");
  assert(leftover === 0, 'marks overlay left on the page');
});

// ---------------------------------------------------------------------------

let passed = 0;
let failed = 0;
for (const [i, c] of cases.entries()) {
  if (ONLY && !c.name.includes(ONLY)) continue;
  try {
    await c.fn();
    passed++;
    console.log(`ok ${i + 1} - ${c.name}`);
  } catch (err) {
    failed++;
    console.log(`not ok ${i + 1} - ${c.name}\n  ${String(err.message || err).split('\n').join('\n  ')}`);
  }
}
console.log(`\n${passed} passed, ${failed} failed`);
server.kill();
process.exit(failed ? 1 : 0);
