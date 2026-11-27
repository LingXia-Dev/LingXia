import { expect, test } from '@lingxia/test';
import type { ActionSheetResult } from '@lingxia/types';
import type { PageDefinition } from '@lingxia/types/page';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { androidDevice, androidCancelLabels } from '@lingxia/test/showcase/helpers/android-device.js';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';

// The modal page's form also shows the cancel label as text: only the dialog's button counts.
const isCancelButton = (node: { text: string; className?: string }) =>
  androidCancelLabels.includes(node.text) && node.className === 'android.widget.Button';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
test.configure({ skip: args.platform === 'android' ? undefined : 'Android only' });

for (const kind of ['modal', 'actionsheet'] as const) {
  const id = `ANDROID-${kind.toUpperCase()}-QUEUE-001`;
  test(`a second native ${kind} waits for the first and is usable once the first settles`, {
    id, app: SHOWCASE_APP_ID, covers: [kind === 'modal' ? 'lx.ui.confirm' : 'lx.ui.actionSheet'], timeout: 60000,
    requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
    start: { page: 'home' },
  }, async (t) => {
    const { app, namespace, defer } = bindFixture(t, id);
    const device = androidDevice(t);
    const first = kind === 'modal' ? 'First modal' : 'First choice';
    const second = kind === 'modal' ? 'Second modal' : 'Second choice';
    const cancelButton = (node: { text: string; className?: string }) =>
      kind === 'modal' ? isCancelButton(node) : androidCancelLabels.includes(node.text);
    defer(async () => {
      for (let i = 0; i < 2; i++) {
        const cancel = (await device.nodes()).find(cancelButton);
        if (cancel) await device.tap(cancel);
      }
      await app.logic.eval((_, key) => { delete (globalThis as Record<string, unknown>)[key]; }, namespace);
    });
    await app.logic.eval(({ lx }, kind, key) => {
      const results: unknown[] = [];
      (globalThis as Record<string, unknown>)[key] = results;
      for (const request of ['first', 'second'] as const) {
        const label = `${request[0].toUpperCase()}${request.slice(1)}`;
        const shown = kind === 'modal'
          ? lx.ui.confirm({ title: `${label} modal`, message: 'Answer me', confirmText: 'OK', cancelText: 'Cancel' })
          : lx.ui.actionSheet({ items: [{ id: request, label: `${label} choice` }] });
        void shown.then((result: unknown) => results.push({ request, result }));
      }
    }, kind, namespace);
    const read = () => app.logic.eval((_, key) => (globalThis as Record<string, unknown>)[key], namespace);
    let nodes = await t.waitFor(() => device.nodes(), { until: (items) => items.some((node) => node.text === first) });
    expect(nodes.some((node) => node.text === second)).toBe(false);
    expect(await read()).toEqual([]);
    const cancel = nodes.filter(cancelButton);
    expect(cancel).toHaveLength(1);
    await device.tap(cancel[0]);
    const firstCanceled = kind === 'modal' ? false : { status: 'canceled' };
    await expect.poll(read).toEqual([{ request: 'first', result: firstCanceled }]);
    nodes = await t.waitFor(() => device.nodes(), { until: (items) => items.some((node) => node.text === second) });
    expect(nodes.some((node) => node.text === first)).toBe(false);
    await device.tapText(kind === 'modal' ? 'OK' : 'Second choice');
    await expect.poll(read).toEqual([
      { request: 'first', result: firstCanceled },
      { request: 'second', result: kind === 'modal' ? true : { status: 'ok', id: 'second' } },
    ]);
  });
}

for (const action of ['confirm', 'cancel', 'back'] as const) {
  const id = `ANDROID-MODAL-${action.toUpperCase()}-001`;
  test(`settle a native modal by ${action} without navigating the page`, {
    id, app: SHOWCASE_APP_ID, covers: ['lx.ui.confirm'], timeout: 60000,
    requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  }, async (t) => {
    const { app, defer } = bindFixture(t, id);
    const device = androidDevice(t);
    defer(async () => {
      const cancel = (await device.nodes()).find((node) => isCancelButton(node));
      if (cancel) await device.tap(cancel);
      await app.nav.reset({ page: 'home' });
    });
    await app.nav.reset({ page: 'home' });
    await app.nav.go({ page: 'ui', params: { type: 'modal' } });
    const current = await app.nav.current();
    const page = await app.page<PageDefinition<{ modalResult: { status: 'ok' | 'canceled' } | null }>>();
    await page.view.testId('modal-show').click();
    await t.waitFor(() => device.nodes(), { until: (nodes) => nodes.some((node) => isCancelButton(node)) });
    if (action === 'confirm') {
      // WebView also exposes the form's matching text as an EditText.
      await device.tapText('This is a modal dialog', 'android.widget.TextView');
      expect((await page.data()).modalResult).toBe(null);
      expect((await device.nodes()).some((node) => node.text === 'OK' && node.className === 'android.widget.Button')).toBe(true);
    }
    if (action === 'back') await device.key('back');
    else await device.tapText(action === 'confirm' ? 'OK' : androidCancelLabels, 'android.widget.Button');
    const result = { status: action === 'confirm' ? 'ok' : 'canceled' };
    await expect.poll(async () => (await page.data()).modalResult).toEqual(result);
    await expect(page.view.testId('modal-result')).toContainText(`"status": "${result.status}"`);
    expect((await app.nav.current()).instanceId).toBe(current.instanceId);
    expect((await device.nodes()).some((node) => isCancelButton(node))).toBe(false);
  });
}

/** A tap on the scrim above the dialog, in the band a fifth of the way down the screen. */
async function tapOutside(device: ReturnType<typeof androidDevice>) {
  const { width, height } = await device.size();
  await device.tap({ text: '', description: '', resource: '', bounds: [0, Math.round(height * .2), width, Math.round(height * .3)] });
}

test('a tap outside a native confirm is consumed: only an answer settles it', {
  id: 'ANDROID-MODAL-OUTSIDE-001', app: SHOWCASE_APP_ID, covers: ['lx.ui.confirm'], timeout: 60000,
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'ANDROID-MODAL-OUTSIDE-001');
  const device = androidDevice(t);
  defer(async () => {
    const cancel = (await device.nodes()).find((node) => isCancelButton(node));
    if (cancel) await device.tap(cancel);
    await app.nav.reset({ page: 'home' });
  });
  await app.nav.reset({ page: 'home' });
  await app.nav.go({ page: 'ui', params: { type: 'modal' } });
  const current = await app.nav.current();
  const page = await app.page<PageDefinition<{ modalResult: { status: 'ok' | 'canceled' } | null }>>();
  await page.view.testId('modal-show').click();
  await t.waitFor(() => device.nodes(), { until: (nodes) => nodes.some((node) => isCancelButton(node)) });
  await tapOutside(device);
  expect((await device.nodes()).some((node) => isCancelButton(node))).toBe(true);
  expect((await page.data()).modalResult).toBe(null);
  expect((await app.nav.current()).instanceId).toBe(current.instanceId);
  await device.tapText(androidCancelLabels, 'android.widget.Button');
  await expect.poll(async () => (await page.data()).modalResult).toEqual({ status: 'canceled' });
});

test('an alert consumes outside taps and settles as confirmed on Back', {
  id: 'ANDROID-MODAL-CONFIRM-ONLY-001', app: SHOWCASE_APP_ID, covers: ['lx.ui.confirm'], timeout: 60000,
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'ANDROID-MODAL-CONFIRM-ONLY-001');
  const device = androidDevice(t);
  const isOk = (node: { text: string; className?: string }) => node.text === 'OK' && node.className === 'android.widget.Button';
  defer(async () => {
    const ok = (await device.nodes()).find(isOk);
    if (ok) await device.tap(ok);
    await app.nav.reset({ page: 'home' });
  });
  await app.nav.reset({ page: 'home' });
  await app.nav.go({ page: 'ui', params: { type: 'modal' } });
  const current = await app.nav.current();
  const page = await app.page<PageDefinition<{ modalResult: { status: 'ok' | 'canceled' } | null }>>();
  await page.view.testId('modal-show-cancel').click();
  await page.view.testId('modal-show').click();
  const nodes = await t.waitFor(() => device.nodes(), { until: (items) => items.some(isOk) });
  expect(nodes.some((node) => isCancelButton(node))).toBe(false);
  await tapOutside(device);
  expect((await device.nodes()).some(isOk)).toBe(true);
  expect((await page.data()).modalResult).toBe(null);
  await device.key('back');
  await expect.poll(async () => (await page.data()).modalResult).toEqual({ status: 'ok' });
  expect((await device.nodes()).some(isOk)).toBe(false);
  expect((await app.nav.current()).instanceId).toBe(current.instanceId);
});

for (const action of ['select', 'cancel', 'back', 'outside'] as const) {
  const id = `ANDROID-ACTIONSHEET-${action.toUpperCase()}-001`;
  test(`settle a native action sheet by ${action} without navigating the page`, {
    id, app: SHOWCASE_APP_ID, covers: ['lx.ui.actionSheet'], timeout: 60000,
    requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  }, async (t) => {
    const { app, defer } = bindFixture(t, id);
    const device = androidDevice(t);
    defer(async () => {
      const cancel = (await device.nodes()).find((node) => androidCancelLabels.includes(node.text));
      if (cancel) await device.tap(cancel);
      await app.nav.reset({ page: 'home' });
    });
    await app.nav.reset({ page: 'home' });
    await app.nav.go({ page: 'ui', params: { type: 'actionsheet' } });
    const current = await app.nav.current();
    const page = await app.page<PageDefinition<{ actionSheetResult: ActionSheetResult | null }>>();
    await page.view.testId('actionsheet-show').click();
    await t.waitFor(() => device.nodes(), { until: (nodes) => nodes.some((node) => androidCancelLabels.includes(node.text)) });
    if (action === 'back') await device.key('back');
    else if (action === 'outside') {
      const { width, height } = await device.size();
      await device.tap({ text: '', description: '', resource: '', bounds: [0, Math.round(height * .2), width, Math.round(height * .3)] });
    } else await device.tapText(action === 'select' ? 'Send Email' : androidCancelLabels);
    const result = action === 'select' ? { status: 'ok', id: '2' } : { status: 'canceled' };
    await expect.poll(async () => (await page.data()).actionSheetResult).toEqual(result);
    await expect(page.view.testId('actionsheet-result')).toContainText(`"status": "${result.status}"`);
    expect((await app.nav.current()).instanceId).toBe(current.instanceId);
    expect((await device.nodes()).some((node) => androidCancelLabels.includes(node.text))).toBe(false);
  });
}
