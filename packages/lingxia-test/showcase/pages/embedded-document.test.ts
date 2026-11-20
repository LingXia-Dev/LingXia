import { expect, test, type Fixture, type TestView } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { currentPageOrNull, waitForCurrentPage, waitForCurrentPageVisible } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const httpBase = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {}).httpBase;
test.configure({ skip: httpBase ? undefined : 'needs --arg httpBase' });

// A host the probe's allow-list does not name; `.invalid` never resolves.
const OUTSIDE = 'https://outside.invalid/';
const NODE = 'probe';

interface Blocked {
  url: string;
  reason: 'navigation' | 'download';
}

interface Bridge {
  invoke<R>(route: string, input: unknown): Promise<R>;
}

/** Calls a dev-only `embeddedDocumentProbe` route from the host page's View. */
function probe<R>(host: TestView, route: string, input: Record<string, unknown>): Promise<R> {
  return host.eval(
    ({ window }, route, input) =>
      (window as unknown as { LingXiaBridge: Bridge }).LingXiaBridge.invoke<Record<string, never>>(`embeddedDocumentProbe.${route}`, input),
    route,
    input,
  ) as unknown as Promise<R>;
}

/** Opens `${httpBase}/page/embedded-a` as an embedded document that may only show the fixture's host. */
async function openDocument(t: Fixture, id: string) {
  const { app, defer } = bindFixture(t, id);
  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'spec-embedded' });
  await waitForCurrentPage(app, 'spec-embedded');
  const host = (await app.page({ name: 'spec-embedded' }, { timeout: 30_000 })).view;
  await host.css('[data-testid="spec-embedded-page"]').first().waitFor({ state: 'attached', timeout: 30_000 });

  const first = `${httpBase}/page/embedded-a`;
  const hosts = [new URL(String(httpBase)).hostname];
  const opened = await probe<{ webtag: string }>(host, 'open', { node: NODE, url: first, hosts });
  defer(async () => { await probe<boolean>(host, 'close', { node: NODE }); });

  /** Runs `body` (a function body returning a JSON-serializable value) in the embedded document. */
  const inDocument = async <R>(body: string): Promise<R> =>
    JSON.parse(await probe<string>(host, 'eval', {
      node: NODE,
      script: `(function(){ return JSON.stringify((function(){ ${body} })()); })()`,
    })) as R;
  const shows = (name: string) => eventually(
    () => inDocument<string | null>(
      "var h = document.querySelector('[data-fixture-page]'); return document.readyState === 'complete' && h ? h.getAttribute('data-fixture-page') : null;",
    ).catch(() => null),
    (value) => value === name,
    { timeoutMs: 15_000, describe: `the embedded document showing ${name}` },
  );
  const blocked = () => probe<Blocked[]>(host, 'blocked', { node: NODE });

  await shows('embedded-a');
  return { host, opened, inDocument, shows, blocked };
}

test('an embedded document has no bridge object and no bridge transport', {
  id: 'EMBEDDED-DOCUMENT-001', app: SHOWCASE_APP_ID, timeout: 60_000,
}, async (t) => {
  const { host, opened, inDocument } = await openDocument(t, 'EMBEDDED-DOCUMENT-001');
  expect(opened.webtag).toContain('~child~probe');

  const seen = await inDocument<Record<string, string>>(`
    var handlers = window.webkit && window.webkit.messageHandlers;
    return {
      bridge: typeof window.LingXiaBridge,
      lx: typeof window.lx,
      proxy: typeof window.LingXiaProxy,
      nativeComponents: typeof window.NativeComponentBridge,
      appleHandler: typeof (handlers && handlers.LingXia),
      appleConsole: typeof (handlers && handlers.LingXiaConsole),
      receiver: typeof window.__LingXiaRecvMessage,
    };`);
  expect(seen).toEqual({
    bridge: 'undefined',
    lx: 'undefined',
    proxy: 'undefined',
    nativeComponents: 'undefined',
    appleHandler: 'undefined',
    appleConsole: 'undefined',
    receiver: 'undefined',
  });

  // The page that owns it is an ordinary lxapp page with its bridge intact.
  expect(await host.eval(({ window }) => typeof (window as unknown as { LingXiaBridge?: unknown }).LingXiaBridge)).toBe('object');
});

test('an embedded document cannot download: the response is cancelled and reported', {
  id: 'EMBEDDED-DOCUMENT-002', app: SHOWCASE_APP_ID, timeout: 60_000,
}, async (t) => {
  const { inDocument, blocked, shows } = await openDocument(t, 'EMBEDDED-DOCUMENT-002');
  const attachment = `${httpBase}/attachment/report.bin`;
  await inDocument<null>(`location.href = ${JSON.stringify(attachment)}; return null;`);

  const reports = await eventually(blocked, (list) => list.some((entry) => entry.reason === 'download'), {
    timeoutMs: 15_000, describe: 'the blocked download report',
  });
  expect(reports.filter((entry) => entry.reason === 'download')).toEqual([{ url: attachment, reason: 'download' }]);
  // Nothing replaced the document.
  await shows('embedded-a');
});

test('window.open in an embedded document loads in place and is judged as a navigation', {
  id: 'EMBEDDED-DOCUMENT-003', app: SHOWCASE_APP_ID, timeout: 60_000,
}, async (t) => {
  const { inDocument, blocked, shows } = await openDocument(t, 'EMBEDDED-DOCUMENT-003');

  await inDocument<null>(`window.open(${JSON.stringify(OUTSIDE)}); return null;`);
  const refused = await eventually(blocked, (list) => list.length > 0, {
    timeoutMs: 15_000, describe: 'the refused new window',
  });
  expect(refused).toEqual([{ url: OUTSIDE, reason: 'navigation' }]);
  await shows('embedded-a');

  await inDocument<null>(`window.open(${JSON.stringify(`${httpBase}/page/embedded-b`)}); return null;`);
  await shows('embedded-b');
  expect(await blocked()).toEqual(refused);
});

test('JS dialogs in an embedded document are answered without UI', {
  id: 'EMBEDDED-DOCUMENT-004', app: SHOWCASE_APP_ID, timeout: 60_000,
}, async (t) => {
  const { inDocument } = await openDocument(t, 'EMBEDDED-DOCUMENT-004');
  const answers = await inDocument<{ confirmed: boolean; prompted: string | null; ms: number }>(`
    var started = Date.now();
    alert('embedded alert');
    var confirmed = confirm('embedded confirm');
    var prompted = prompt('embedded prompt', 'default');
    return { confirmed: confirmed, prompted: prompted, ms: Date.now() - started };`);
  expect(answers.confirmed).toBe(false);
  expect(answers.prompted).toBe(null);
  // A dialog on screen would have held the script until someone dismissed it.
  expect(answers.ms).toBeLessThan(3_000);
});

test('an embedded document cannot navigate its top level outside the allow-list', {
  id: 'EMBEDDED-DOCUMENT-005', app: SHOWCASE_APP_ID, timeout: 60_000,
}, async (t) => {
  const { host, inDocument, blocked, shows } = await openDocument(t, 'EMBEDDED-DOCUMENT-005');

  await inDocument<null>(`location.href = ${JSON.stringify(OUTSIDE)}; return null;`);
  const refused = await eventually(blocked, (list) => list.length > 0, {
    timeoutMs: 15_000, describe: 'the refused navigation',
  });
  expect(refused).toEqual([{ url: OUTSIDE, reason: 'navigation' }]);
  await shows('embedded-a');

  // The first load is judged the same way: a second document may not open there at all.
  const error = await probe<unknown>(host, 'open', { node: 'outside', url: OUTSIDE, hosts: [new URL(String(httpBase)).hostname] })
    .then(() => null, (reason: unknown) => String(reason));
  expect(error).not.toBe(null);
});
