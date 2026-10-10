import { expect, spec } from '@lingxia/test';
import type { ModalResult, ActionSheetResult } from '@lingxia/types';
import type { PageContract } from '@lingxia/types/page';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { androidDevice, androidCancelLabels } from '../../helpers/android-device.js';
import { bindFixture } from '../../helpers/poll.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const androidSpec = args.platform === 'android' ? spec : spec.skip;

for (const kind of ['modal', 'actionsheet'] as const) {
  const id = `ANDROID-${kind.toUpperCase()}-REPLACE-001`;
  androidSpec(`replacing a native ${kind} settles the old request and keeps the new one usable`, {
    id, app: SHOWCASE_APP_ID, covers: [kind === 'modal' ? 'lx.showModal' : 'lx.showActionSheet'], timeout: 60000,
    requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
    start: { page: 'home' },
  }, async (t) => {
    const { app, namespace, defer } = bindFixture(t, id);
    const device = androidDevice(t);
    defer(async () => {
      const cancel = (await device.nodes()).find((node) => androidCancelLabels.includes(node.text));
      if (cancel) await device.tap(cancel);
      await app.logic.eval((_, key) => { delete (globalThis as Record<string, unknown>)[key]; }, namespace);
    });
    await app.logic.eval(({ lx }, kind, key) => {
      const results: unknown[] = [];
      (globalThis as Record<string, unknown>)[key] = results;
      const request = kind === 'modal'
        ? lx.showModal({ title: 'First modal', content: 'Replace me', confirmText: 'OK', cancelText: 'Cancel' })
        : lx.showActionSheet({ items: [{ id: 'first', label: 'First choice' }] });
      void request.then((result) => results.push({ request: 'first', result }));
    }, kind, namespace);
    await t.waitFor(() => device.nodes(), { until: (nodes) => nodes.some((node) => node.text === (kind === 'modal' ? 'First modal' : 'First choice')) });
    await app.logic.eval(({ lx }, kind, key) => {
      const results = (globalThis as Record<string, unknown>)[key] as unknown[];
      const request = kind === 'modal'
        ? lx.showModal({ title: 'Second modal', content: 'Confirm me', confirmText: 'OK', cancelText: 'Cancel' })
        : lx.showActionSheet({ items: [{ id: 'second', label: 'Second choice' }] });
      void request.then((result) => results.push({ request: 'second', result }));
    }, kind, namespace);
    const read = () => app.logic.eval((_, key) => (globalThis as Record<string, unknown>)[key], namespace);
    await expect.poll(read).toEqual([{ request: 'first', result: { status: 'canceled' } }]);
    const nodes = await t.waitFor(() => device.nodes(), { until: (items) => items.some((node) => node.text === (kind === 'modal' ? 'Second modal' : 'Second choice')) });
    expect(nodes.some((node) => node.text === (kind === 'modal' ? 'First modal' : 'First choice'))).toBe(false);
    await device.tapText(kind === 'modal' ? 'OK' : 'Second choice');
    await expect.poll(read).toEqual([
      { request: 'first', result: { status: 'canceled' } },
      { request: 'second', result: kind === 'modal' ? { status: 'ok' } : { status: 'ok', id: 'second' } },
    ]);
  });
}

for (const action of ['confirm', 'cancel', 'back', 'outside'] as const) {
  const id = `ANDROID-MODAL-${action.toUpperCase()}-001`;
  androidSpec(`settle a native modal by ${action} without navigating the page`, {
    id, app: SHOWCASE_APP_ID, covers: ['lx.showModal'], timeout: 60000,
    requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  }, async (t) => {
    const { app, defer } = bindFixture(t, id);
    const device = androidDevice(t);
    defer(async () => {
      const cancel = (await device.nodes()).find((node) => node.text === 'CANCEL');
      if (cancel) await device.tap(cancel);
      await app.nav.relaunch({ page: 'home' });
    });
    await app.nav.relaunch({ page: 'home' });
    await app.nav.to({ page: 'ui', query: { type: 'modal' } });
    const current = await app.nav.current();
    const page = await app.page<PageContract<{ modalResult: ModalResult | null }>>();
    await page.view.testId('modal-show').click();
    await t.waitFor(() => device.nodes(), { until: (nodes) => nodes.some((node) => node.text === 'CANCEL') });
    if (action === 'confirm') {
      await device.tapText('This is a modal dialog');
      expect((await page.data()).modalResult).toBe(null);
      expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(true);
    }
    if (action === 'back') await device.key('back');
    else if (action === 'outside') {
      const { width, height } = await device.size();
      await device.tap({ text: '', description: '', resource: '', bounds: [0, Math.round(height * .2), width, Math.round(height * .3)] });
    } else await device.tapText(action === 'confirm' ? 'OK' : 'CANCEL');
    const result = { status: action === 'confirm' ? 'ok' : 'canceled' };
    await expect.poll(async () => (await page.data()).modalResult).toEqual(result);
    await expect(page.view.testId('modal-result')).toContainText(`"status": "${result.status}"`);
    expect((await app.nav.current()).instanceId).toBe(current.instanceId);
    expect((await device.nodes()).some((node) => node.text === 'CANCEL')).toBe(false);
  });
}

androidSpec('a confirm-only modal consumes Back and outside taps until confirmed', {
  id: 'ANDROID-MODAL-CONFIRM-ONLY-001', app: SHOWCASE_APP_ID, covers: ['lx.showModal'], timeout: 60000,
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'ANDROID-MODAL-CONFIRM-ONLY-001');
  const device = androidDevice(t);
  defer(async () => {
    const ok = (await device.nodes()).find((node) => node.text === 'OK');
    if (ok) await device.tap(ok);
    await app.nav.relaunch({ page: 'home' });
  });
  await app.nav.relaunch({ page: 'home' });
  await app.nav.to({ page: 'ui', query: { type: 'modal' } });
  const current = await app.nav.current();
  const page = await app.page<PageContract<{ modalResult: ModalResult | null }>>();
  await page.view.css('#modalShowCancel').click();
  await page.view.testId('modal-show').click();
  const nodes = await t.waitFor(() => device.nodes(), { until: (items) => items.some((node) => node.text === 'OK') });
  expect(nodes.some((node) => node.text === 'CANCEL')).toBe(false);
  await device.key('back');
  const { width, height } = await device.size();
  await device.tap({ text: '', description: '', resource: '', bounds: [0, Math.round(height * .2), width, Math.round(height * .3)] });
  expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(true);
  expect((await page.data()).modalResult).toBe(null);
  expect((await app.nav.current()).instanceId).toBe(current.instanceId);
  await device.tapText('OK');
  await expect.poll(async () => (await page.data()).modalResult).toEqual({ status: 'ok' });
});

for (const action of ['select', 'cancel', 'back', 'outside'] as const) {
  const id = `ANDROID-ACTIONSHEET-${action.toUpperCase()}-001`;
  androidSpec(`settle a native action sheet by ${action} without navigating the page`, {
    id, app: SHOWCASE_APP_ID, covers: ['lx.showActionSheet'], timeout: 60000,
    requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  }, async (t) => {
    const { app, defer } = bindFixture(t, id);
    const device = androidDevice(t);
    defer(async () => {
      const cancel = (await device.nodes()).find((node) => androidCancelLabels.includes(node.text));
      if (cancel) await device.tap(cancel);
      await app.nav.relaunch({ page: 'home' });
    });
    await app.nav.relaunch({ page: 'home' });
    await app.nav.to({ page: 'ui', query: { type: 'actionsheet' } });
    const current = await app.nav.current();
    const page = await app.page<PageContract<{ actionSheetResult: ActionSheetResult | null }>>();
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
