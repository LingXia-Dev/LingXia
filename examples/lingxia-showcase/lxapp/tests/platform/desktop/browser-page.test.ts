import { expect, spec, type Fixture } from '@lingxia/test';
import { runtimePlatform } from '../../helpers/platform.js';

/**
 * The host browser's page driver (`t.automation.browser`) against
 * `fixtures/browser/form.html`, served by the HTTP fixture: snapshots and
 * refs, actions and their typed failures, form input, text, console and
 * screenshots. The same specs hold for WKWebView and WebView2.
 */
const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const httpBase = testArgs.httpBase;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
spec.configure({ skip: httpBase && !selectedGate ? undefined : 'needs --arg httpBase; not in a gate run' });

const FORM_URL = `${httpBase}/browser/form.html`;

type Browser = Fixture['automation']['browser'];

interface FailureDetails {
  target?: unknown;
  coveredBy?: { selector?: string; ref?: string };
  reason?: string;
  available?: unknown[];
  timedOut?: boolean;
  hint?: string;
}

interface SnapshotNode {
  offscreen?: boolean;
  children?: readonly SnapshotNode[];
}

interface PageState {
  events: string[];
  composition: string[];
  controlled: string;
  clicks: Record<string, number>;
  hover: number;
  dropped: boolean;
}

async function snapshotText(browser: Browser, tab: string): Promise<string> {
  return (await browser.snapshot({ tab })).text ?? '';
}

function refOf(snap: string, role: string, name: string): string {
  const escaped = JSON.stringify(name).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = new RegExp(`- ${role} ${escaped}[^\\n]*\\[ref=(e\\d+)\\]`).exec(snap);
  if (!match) throw new Error(`no ${role} "${name}" ref in snapshot:\n${snap.slice(0, 3000)}`);
  return match[1];
}

/** form.html in a fresh tab, after its 700 ms late elements and the srcdoc iframe, with a first snapshot. */
async function openForm(t: Fixture) {
  const browser = t.automation.browser;
  const { tab } = await browser.open({ url: FORM_URL });
  t.defer(() => browser.close({ tab }));
  await browser.wait({ tab, loaded: true });
  await browser.wait({
    tab,
    js: "!!document.getElementById('late') && !document.getElementById('late-enabled').disabled"
      + " && !!document.getElementById('same').contentDocument.getElementById('inner-btn')",
  });
  const snap = await snapshotText(browser, tab);
  return {
    browser,
    tab,
    snap,
    ref: (role: string, name: string) => refOf(snap, role, name),
    state: <T = PageState>(js = 'state') => browser.eval<T>({ tab, js }),
  };
}

async function failure(kind: string, action: () => Promise<unknown>): Promise<FailureDetails> {
  try {
    await action();
  } catch (error) {
    const { code, data } = error as { code?: string; data?: FailureDetails };
    if (code !== kind) throw new Error(`expected ${kind}, got ${code}: ${String((error as Error).message ?? error)}`);
    return data ?? {};
  }
  throw new Error(`expected ${kind}, but the action succeeded`);
}

function flatten(nodes: readonly SnapshotNode[]): SnapshotNode[] {
  return nodes.flatMap((node) => [node, ...flatten(node.children ?? [])]);
}

spec('snapshot outlines roles, names, states and refs', {
  id: 'BROWSER-PAGE-001',
  covers: ['BrowserDriver.open', 'BrowserDriver.wait', 'BrowserDriver.snapshot'],
}, async (t) => {
  const { snap } = await openForm(t);
  for (const needle of [
    'heading "Browser page fixture" [ref=',
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
    expect(snap).toContain(needle);
  }
});

spec('snapshot reaches open shadow roots and same-origin iframes', {
  id: 'BROWSER-PAGE-002',
  covers: ['BrowserDriver.snapshot'],
}, async (t) => {
  const { snap, ref } = await openForm(t);
  ref('button', 'Shadow button');
  ref('textbox', 'Shadow input');
  ref('button', 'Inner button');
  expect(snap).toMatch(/iframe "Cross origin"[^\n]*\[cross-origin: contents unavailable\]/);
});

spec('snapshot marks off-screen elements and masks passwords', {
  id: 'BROWSER-PAGE-003',
  covers: ['BrowserDriver.snapshot', 'BrowserDriver.fill'],
}, async (t) => {
  const { browser, tab, ref } = await openForm(t);
  await browser.fill({ tab, ref: ref('textbox', 'Password'), text: 'secret' });
  const snap = await snapshotText(browser, tab);
  expect(snap).not.toContain('secret');
  expect(snap).toMatch(/link "Bottom"[^\n]*\n/);
  const json = await browser.snapshot({ tab, format: 'json' });
  expect(flatten(json.nodes ?? []).some((node) => node.offscreen)).toBe(true);
});

spec('snapshot JSON form, subtree root and budget', {
  id: 'BROWSER-PAGE-004',
  covers: ['BrowserDriver.snapshot'],
}, async (t) => {
  const { browser, tab } = await openForm(t);
  const json = await browser.snapshot({ tab, format: 'json' });
  expect(Array.isArray(json.nodes)).toBe(true);
  expect(json.refCount).toBeGreaterThan(10);
  expect(typeof json.generation).toBe('string');
  const sub = (await browser.snapshot({ tab, css: '#profile' })).text ?? '';
  expect(sub).toContain('form "Profile"');
  expect(sub).not.toContain('Readable content');
  const small = await browser.snapshot({ tab, maxNodes: 5 });
  expect(small.truncated).toBe(true);
  expect(small.text).toContain('truncated');
});

spec('snapshot sinceLast reports only what changed', {
  id: 'BROWSER-PAGE-005',
  covers: ['BrowserDriver.snapshot', 'BrowserDriver.fill'],
}, async (t) => {
  const { browser, tab, ref } = await openForm(t);
  await browser.fill({ tab, ref: ref('textbox', 'Full name'), text: 'Ada' });
  const changed = await browser.snapshot({ tab, sinceLast: true });
  expect(changed.diff?.unchanged).toBe(false);
  expect(changed.diff?.added.some((line) => line.includes('"Ada"'))).toBe(true);
  expect(changed.text).toBeUndefined();
  const again = await browser.snapshot({ tab, sinceLast: true });
  expect(again.diff?.unchanged).toBe(true);
});

spec('action results can carry the snapshot diff', {
  id: 'BROWSER-PAGE-006',
  covers: ['BrowserDriver.click'],
}, async (t) => {
  const { browser, tab, ref } = await openForm(t);
  const result = await browser.click({ tab, ref: ref('checkbox', 'I agree'), snapshot: 'diff' });
  expect(result.snapshot?.diff?.added
    .some((line) => line.includes('checkbox "I agree"') && line.includes('[checked]'))).toBe(true);
});

spec('fill fires input/change and drives controlled inputs', {
  id: 'BROWSER-PAGE-007',
  covers: ['BrowserDriver.fill', 'BrowserDriver.click', 'BrowserDriver.eval'],
}, async (t) => {
  const { browser, tab, ref, state } = await openForm(t);
  await browser.fill({ tab, ref: ref('textbox', 'Controlled'), text: 'kept' });
  await browser.click({ tab, ref: ref('textbox', 'Full name') });
  const s = await state<{ v: string; s: string; e: string[] }>(
    "({ v: document.getElementById('controlled').value, s: state.controlled, e: state.events })",
  );
  expect(s.v).toBe('kept');
  expect(s.s).toBe('kept');
  expect(s.e).toContain('input:controlled');
  expect(s.e).toContain('change:controlled');
});

spec('fill sets date, time, number, range, color and contenteditable', {
  id: 'BROWSER-PAGE-008',
  covers: ['BrowserDriver.fill'],
}, async (t) => {
  const { browser, tab, ref, state } = await openForm(t);
  for (const [role, name, text] of [
    ['textbox', 'Date', '2026-10-22'],
    ['textbox', 'Time', '09:30'],
    ['spinbutton', 'Count', '42'],
    ['slider', 'Volume', '7'],
  ]) {
    await browser.fill({ tab, ref: ref(role, name), text });
  }
  await browser.fill({ tab, css: '#color', text: '#ff0000' });
  await browser.fill({ tab, ref: ref('textbox', 'Editor'), text: 'Hello editor' });
  expect(await state<string[]>(
    "['date','time','count','volume','color'].map(id => document.getElementById(id).value).concat(document.getElementById('editor').innerText)",
  )).toEqual(['2026-10-22', '09:30', '42', '7', '#ff0000', 'Hello editor']);
  const details = await failure('usage', () =>
    browser.fill({ tab, ref: ref('textbox', 'Date'), text: 'not a date' }));
  expect(details.hint).toBe('YYYY-MM-DD');
});

spec('type appends, clears, and commits CJK through a composition', {
  id: 'BROWSER-PAGE-009',
  covers: ['BrowserDriver.type'],
}, async (t) => {
  const { browser, tab, ref, state } = await openForm(t);
  const email = ref('textbox', 'Email');
  await browser.type({ tab, ref: email, text: 'a@b' });
  await browser.type({ tab, ref: email, text: '.co' });
  await browser.type({ tab, ref: ref('textbox', 'Bio'), text: 'old' });
  await browser.type({ tab, ref: ref('textbox', 'Bio'), text: 'new', clear: true });
  await browser.type({ tab, ref: ref('textbox', 'IME'), text: '你好' });
  const s = await state<{ email: string; bio: string; ime: string; comp: string[] }>(
    "({ email: document.getElementById('email').value, bio: document.getElementById('bio').value, ime: document.getElementById('ime').value, comp: state.composition })",
  );
  expect(s.email).toBe('a@b.co');
  expect(s.bio).toBe('new');
  expect(s.ime).toBe('你好');
  expect(s.comp).toContain('compositionend:你好');
});

spec('selectOption by label, value, index and multiple', {
  id: 'BROWSER-PAGE-010',
  covers: ['BrowserDriver.selectOption'],
}, async (t) => {
  const { browser, tab, ref, state } = await openForm(t);
  const country = ref('combobox', 'Country');
  const first = async (options: Parameters<Browser['selectOption']>[0]['options']) =>
    (await browser.selectOption({ tab, ref: country, options })).selected?.[0]?.value;
  expect(await first(['Japan'])).toBe('jp');
  expect(await first(['cn'])).toBe('cn');
  expect(await first([{ index: 2 }])).toBe('jp');
  const multiple = await browser.selectOption({ tab, ref: ref('listbox', 'Languages'), options: ['zh', 'French'] });
  expect((multiple.selected ?? []).map((option) => option.value)).toEqual(['zh', 'fr']);
  await failure('disabled', () => browser.selectOption({ tab, ref: country, options: ['Korea'] }));
  const missing = await failure('not_found', () => browser.selectOption({ tab, ref: country, options: ['Atlantis'] }));
  expect(missing.available).toHaveLength(4);
  const events = await state<string[]>('state.events');
  expect(events).toContain('change:country');
  expect(events).toContain('input:country');
});

spec('check and uncheck are idempotent and reach styled controls', {
  id: 'BROWSER-PAGE-011',
  covers: ['BrowserDriver.check', 'BrowserDriver.uncheck'],
}, async (t) => {
  const { browser, tab, ref } = await openForm(t);
  const agree = ref('checkbox', 'I agree');
  let r = await browser.check({ tab, ref: agree });
  expect(r.checked).toBe(true);
  expect(r.changed).toBe(true);
  expect((await browser.check({ tab, ref: agree })).changed).toBe(false);
  expect((await browser.uncheck({ tab, ref: agree })).checked).toBe(false);
  expect((await browser.check({ tab, ref: ref('radio', 'Pro') })).checked).toBe(true);
  await failure('usage', () => browser.uncheck({ tab, ref: ref('radio', 'Pro') }));
  const started = Date.now();
  r = await browser.check({ tab, css: '#styled' });
  expect(r.checked).toBe(true);
  expect(Date.now() - started).toBeLessThan(3000);
  expect((await browser.check({ tab, ref: ref('switch', 'Dark mode') })).checked).toBe(true);
});

spec('setInputFiles attaches files without a chooser, even to hidden inputs', {
  id: 'BROWSER-PAGE-012',
  covers: ['BrowserDriver.setInputFiles'],
}, async (t) => {
  const { browser, tab } = await openForm(t);
  const { files: [a, b] } = await (await fetch(`${httpBase}/browser/files`)).json() as { files: string[] };
  const r = await browser.setInputFiles({ tab, css: '#files', files: [a, b] });
  expect((r.files ?? []).map((file) => file.name)).toEqual(['a.txt', 'b.png']);
  expect(r.files?.[0]?.size).toBe(5);
  expect(r.files?.[1]?.type).toBe('image/png');
  const hidden = await browser.setInputFiles({ tab, css: '#hidden-file', files: [a] });
  expect(hidden.files).toHaveLength(1);
  await failure('usage', () => browser.setInputFiles({ tab, css: '#hidden-file', files: [a, b] }));
});

spec('actionability failures are typed', {
  id: 'BROWSER-PAGE-013',
  covers: ['BrowserDriver.click', 'BrowserDriver.fill'],
}, async (t) => {
  const { browser, tab, ref } = await openForm(t);
  const disabled = await failure('disabled', () =>
    browser.click({ tab, ref: ref('button', 'Disabled'), timeoutMs: 300 }));
  expect(disabled.timedOut).toBe(true);
  expect(disabled.target).toBeTruthy();
  const covered = await failure('covered', () => browser.click({ tab, css: '#covered', timeoutMs: 300 }));
  expect(covered.coveredBy?.selector).toBe('div#overlay');
  expect(covered.coveredBy?.ref).toMatch(/^e\d+$/);
  await failure('not_visible', () => browser.click({ tab, css: '#hidden-btn', timeoutMs: 300 }));
  await failure('not_editable', () =>
    browser.fill({ tab, ref: ref('textbox', 'Read only'), text: 'x', timeoutMs: 300 }));
  await failure('not_found', () => browser.click({ tab, css: '#nope', timeoutMs: 300 }));
  await failure('usage', () => browser.click({ tab, css: '#[', timeoutMs: 300 }));
  await failure('stale_ref', () => browser.click({ tab, ref: 'e99999' }));
});

spec('actions wait for elements to appear, enable and stop moving', {
  id: 'BROWSER-PAGE-014',
  covers: ['BrowserDriver.click'],
}, async (t) => {
  const browser = t.automation.browser;
  const { tab } = await browser.open({ url: FORM_URL });
  t.defer(() => browser.close({ tab }));
  await browser.click({ tab, css: '#late' });
  await browser.click({ tab, css: '#late-enabled' });
  await browser.click({ tab, css: '#mover' });
  expect(await browser.eval<Record<string, number>>({ tab, js: 'state.clicks' }))
    .toEqual({ late: 1, 'late-enabled': 1, mover: 1 });
});

spec('refs act inside shadow roots and same-origin iframes', {
  id: 'BROWSER-PAGE-015',
  covers: ['BrowserDriver.click', 'BrowserDriver.fill'],
}, async (t) => {
  const { browser, tab, ref, state } = await openForm(t);
  await browser.click({ tab, ref: ref('button', 'Shadow button') });
  await browser.click({ tab, ref: ref('button', 'Inner button') });
  await browser.fill({ tab, ref: ref('textbox', 'Inner input'), text: 'framed' });
  const s = await state<{ clicks: Record<string, number>; framed: string }>(
    "({ clicks: state.clicks, framed: document.getElementById('same').contentDocument.getElementById('inner-input').value })",
  );
  expect(s.clicks.shadow).toBe(1);
  expect(s.clicks.inner).toBe(1);
  expect(s.framed).toBe('framed');
});

spec('hover, drag, press and scrollTo', {
  id: 'BROWSER-PAGE-016',
  covers: ['BrowserDriver.hover', 'BrowserDriver.drag', 'BrowserDriver.press', 'BrowserDriver.scrollTo'],
}, async (t) => {
  const { browser, tab, ref, state } = await openForm(t);
  const hovered = await browser.hover({ tab, ref: ref('button', 'Hover me') });
  // WKWebView has no browser-level pointer input: hover is DOM-synthesized.
  if (await runtimePlatform(t.app) === 'macos') expect(hovered.trusted).toBe(false);
  await browser.drag({ tab, css: '#drag-src', to: { css: '#drop-zone' } });
  const name = ref('textbox', 'Full name');
  await browser.fill({ tab, ref: name, text: 'Adam' });
  await browser.press({ tab, ref: name, key: 'Backspace' });
  const scrolled = await browser.scrollTo({ tab, css: '#bottom' });
  const s = await state<{ hover: number; dropped: boolean; name: string; h: number }>(
    "({ hover: state.hover, dropped: state.dropped, name: document.getElementById('name').value, h: innerHeight })",
  );
  expect(s.hover).toBeGreaterThanOrEqual(1);
  expect(s.dropped).toBe(true);
  expect(s.name).toBe('Ada');
  expect(scrolled.rect?.y).toBeGreaterThanOrEqual(0);
  expect(scrolled.rect?.y).toBeLessThan(s.h);
});

spec('stale refs say why: removed or navigated', {
  id: 'BROWSER-PAGE-017',
  covers: ['BrowserDriver.check', 'BrowserDriver.click', 'BrowserDriver.fill'],
}, async (t) => {
  const { browser, tab, ref, state } = await openForm(t);
  const agree = ref('checkbox', 'I agree');
  await state("document.getElementById('agree').closest('label').remove(), true");
  expect((await failure('stale_ref', () => browser.check({ tab, ref: agree }))).reason).toBe('removed');
  const name = ref('textbox', 'Full name');
  await browser.click({ tab, ref: ref('link', 'Next page'), waitNavigation: true, complete: true });
  expect((await failure('stale_ref', () => browser.fill({ tab, ref: name, text: 'x' }))).reason).toBe('navigated');
  // Once the new page has issued refs of its own, an old one still cannot reach them.
  expect(await snapshotText(browser, tab)).toContain('textbox "Search"');
  expect((await failure('stale_ref', () => browser.fill({ tab, ref: name, text: 'x' }))).reason).toBe('navigated');
});

spec('a navigating action reports the page it led to', {
  id: 'BROWSER-PAGE-018',
  covers: ['BrowserDriver.click'],
}, async (t) => {
  const { browser, tab, ref } = await openForm(t);
  const r = await browser.click({
    tab, ref: ref('link', 'Next page'), waitNavigation: true, complete: true, snapshot: 'full',
  });
  expect(r.snapshot?.navigated).toBe(true);
  expect(r.snapshot?.text).toContain('heading "Second page"');
});

spec('text returns readable Markdown', {
  id: 'BROWSER-PAGE-019',
  covers: ['BrowserDriver.text'],
}, async (t) => {
  const { browser, tab } = await openForm(t);
  const { markdown } = await browser.text({ tab });
  for (const needle of ['## Readable content', '[link](https://example.com/doc)', '**bold**', '- First', '  - Nested', '1. One', '| Key | Value |', '```']) {
    expect(markdown).toContain(needle);
  }
  expect(markdown).not.toContain('Footer text');
  expect((await browser.text({ tab, all: true })).markdown).toContain('Footer text');
  const small = await browser.text({ tab, maxChars: 20 });
  expect(small.truncated).toBe(true);
  expect(small.markdown).toContain('truncated');
});

spec('console captures logs and uncaught errors', {
  id: 'BROWSER-PAGE-020',
  covers: ['BrowserDriver.console.list', 'BrowserDriver.console.clear'],
}, async (t) => {
  const { browser, tab } = await openForm(t);
  await browser.console.clear({ tab });
  await browser.click({ tab, css: '#log-btn' });
  await browser.click({ tab, css: '#throw-btn' });
  const list = await t.waitFor(() => browser.console.list({ tab }), {
    until: (read) => read.entries.some((entry) => entry.text.includes('boom')),
  });
  const texts = list.entries.map((entry) => `${entry.level}:${entry.text}`);
  expect(texts.some((text) => text.includes('hello from page'))).toBe(true);
  expect(texts.some((text) => text.startsWith('error:') && text.includes('page error'))).toBe(true);
  expect(texts.some((text) => text.startsWith('error:Uncaught') && text.includes('boom'))).toBe(true);
  expect((await browser.console.list({ tab, since: list.nextSeq })).entries).toHaveLength(0);
  const errors = await browser.console.list({ tab, levels: ['error'] });
  expect(errors.entries.every((entry) => entry.level === 'error')).toBe(true);
});

spec('screenshots: viewport, full page, element, annotated', {
  id: 'BROWSER-PAGE-021',
  covers: ['BrowserDriver.screenshot'],
}, async (t) => {
  const { browser, tab, ref } = await openForm(t);
  const png = (shot: { format: string; base64: string }) => {
    expect(shot.format).toBe('png');
    expect(shot.base64.startsWith('iVBORw0KGgo')).toBe(true);
  };
  const viewport = await browser.screenshot({ tab });
  png(viewport);
  const full = await browser.screenshot({ tab, fullPage: true });
  png(full);
  expect(full.height).toBeGreaterThan(viewport.height);
  // Only a page past the capture limit clips (Windows) or scales (macOS).
  expect(full.clipped ?? false).toBe(false);
  const element = await browser.screenshot({ tab, ref: ref('heading', 'Browser page fixture') });
  png(element);
  expect(element.width).toBeLessThan(viewport.width);
  expect(element.height).toBeLessThan(viewport.height / 2);
  const annotated = await browser.screenshot({ tab, annotate: true });
  png(annotated);
  expect(annotated.marks?.length ?? 0).toBeGreaterThan(5);
  expect((annotated.marks ?? []).every((mark) => /^e\d+$/.test(mark.ref) && mark.box.width > 0)).toBe(true);
  expect(await browser.eval<number>({ tab, js: "document.querySelectorAll('lingxia-page-marks').length" })).toBe(0);
});
