import { expect, spec, type Fixture } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { androidDevice } from '../../helpers/android-device.js';
import { bindFixture } from '../../helpers/poll.js';
import type { ProbeDocument } from '../../helpers/view.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const androidSpec = args.platform === 'android' ? spec : spec.skip;

androidSpec('normalize cascading native picker indices before resolving the child column', {
  id: 'ANDROID-PICKER-BOUNDS-001', app: SHOWCASE_APP_ID, covers: ['lx.navigateTo'],
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  start: { page: 'picker' }, timeout: 60000,
}, async (t) => {
  const { page, device } = await openPicker(t, 'ANDROID-PICKER-BOUNDS-001');
  // Exercise the public custom element boundary with deliberately invalid indices.
  await page.view.eval(({ document }) => {
    const doc = document as unknown as ProbeDocument;
    const picker = doc.createElement('lx-picker');
    picker.setAttribute('id', 'picker-bounds-probe');
    picker.setAttribute('mode', 'cascading');
    picker.setAttribute('columns', JSON.stringify([['North', 'South'], { North: ['NorthCity'], South: ['SouthCity'] }]));
    picker.setAttribute('default-index', '[99,-4,42]');
    picker.addEventListener('change', (event) => {
      picker.setAttribute('data-result', JSON.stringify((event as { detail: unknown }).detail));
    });
    doc.body.appendChild(picker);
  });
  const nodes = await t.waitFor(() => device.nodes(), { until: (items) => items.some((node) => node.text === 'OK'), timeout: 15000 });
  expect(nodes.some((node) => node.text === 'SouthCity')).toBe(true);
  expect(nodes.some((node) => node.text === 'NorthCity')).toBe(false);
  await device.tapText('OK');
  await expect.poll(() => page.view.eval(({ document }) => {
    const result = document.getElementById('picker-bounds-probe')?.getAttribute('data-result');
    return result ? JSON.parse(result) : null;
  })).toEqual({ index: [1, 0], confirmed: true });
  expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(false);
});

async function openPicker(t: Fixture, id: string) {
  const { app, defer } = bindFixture(t, id);
  const device = androidDevice(t);
  defer(async () => {
    const cancel = (await device.nodes()).find((node) => node.text === 'Cancel' || node.text === '\u53d6\u6d88');
    if (cancel) await device.tap(cancel);
    await app.nav.relaunch({ page: 'home' });
  });
  const page = await app.page<PageContract<{ coffee?: string; location?: string[]; multiTime: string[]; activeTab: string }>>({ name: 'picker' });
  const advanceColumn = async (current: string, next: string) => {
    const nodes = await t.waitFor(() => device.nodes(), { until: (items) => items.some((node) => node.text === next), timeout: 15000 });
    const from = nodes.find((node) => node.text === current);
    const to = nodes.find((node) => node.text === next);
    if (!from || !to) throw new Error(`Missing picker rows ${current}, ${next}`);
    const x = Math.round((to.bounds[0] + to.bounds[2]) / 2);
    await device.swipe({ x1: x, x2: x, y1: Math.round((to.bounds[1] + to.bounds[3]) / 2),
      y2: Math.round((from.bounds[1] + from.bounds[3]) / 2), duration: 650 });
  };
  return { page, device, advanceColumn };
}

androidSpec('choose a native picker value and render the confirmed selection', {
  id: 'ANDROID-PICKER-CONFIRM-001', app: SHOWCASE_APP_ID, covers: ['lx.navigateTo'],
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  start: { page: 'picker' }, timeout: 60000,
}, async (t) => {
  const { page, device, advanceColumn } = await openPicker(t, 'ANDROID-PICKER-CONFIRM-001');
  const trigger = page.view.css('[role="button"]');
  await trigger.click();
  await advanceColumn('Espresso', 'Americano');
  await expect.poll(async () => (await page.data()).coffee).toBe('Americano');
  await device.tapText('OK');
  await expect.poll(async () => (await page.data()).coffee).toBe('Americano');
  await expect(trigger).toContainText('Americano');
  expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(false);
  await trigger.click();
  await device.tapText('OK');
  await expect.poll(async () => (await page.data()).coffee).toBe('Americano');
  expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(false);
});

androidSpec('cancel a native picker and reopen it with the original value', {
  id: 'ANDROID-PICKER-CANCEL-001', app: SHOWCASE_APP_ID, covers: ['lx.navigateTo'],
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  start: { page: 'picker' }, timeout: 60000,
}, async (t) => {
  const { page, device } = await openPicker(t, 'ANDROID-PICKER-CANCEL-001');
  const trigger = page.view.css('[role="button"]');
  const before = (await page.data()).coffee;
  await trigger.click();
  await device.tapText('Cancel');
  expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(false);
  expect((await page.data()).coffee).toBe(before);
  await expect(trigger).toContainText('Select coffee');
  await trigger.click();
  await device.tapText('OK');
  await expect.poll(async () => (await page.data()).coffee).toBe('Espresso');
  await expect(trigger).toContainText('Espresso');
  expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(false);
});

androidSpec('cascade the native picker city column and preserve the selection on reopen', {
  id: 'ANDROID-PICKER-CASCADE-001', app: SHOWCASE_APP_ID, covers: ['lx.navigateTo'],
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  start: { page: 'picker' }, timeout: 60000,
}, async (t) => {
  const { page, device, advanceColumn } = await openPicker(t, 'ANDROID-PICKER-CASCADE-001');
  await page.view.css('button').nth(1).click();
  await expect.poll(async () => (await page.data()).activeTab).toBe('multiSelector');
  const trigger = page.view.css('[role="button"]');
  await trigger.click();
  await advanceColumn('Asia', 'Europe');
  await advanceColumn('London', 'Paris');
  await expect.poll(async () => (await page.data()).location).toEqual(['Europe', 'Paris']);
  await device.tapText('\u786e\u5b9a');
  await expect(trigger).toContainText('Europe - Paris');
  expect((await device.nodes()).some((node) => node.text === '\u786e\u5b9a')).toBe(false);
  await trigger.click();
  await device.tapText('\u786e\u5b9a');
  await expect.poll(async () => (await page.data()).location).toEqual(['Europe', 'Paris']);
  expect((await device.nodes()).some((node) => node.text === '\u786e\u5b9a')).toBe(false);
});

androidSpec('change both native picker columns and render the selected time', {
  id: 'ANDROID-PICKER-COLUMNS-001', app: SHOWCASE_APP_ID, covers: ['lx.navigateTo'],
  requires: { args: ['androidDevice'] }, reason: 'requires Android OS input fixture',
  start: { page: 'picker' }, timeout: 60000,
}, async (t) => {
  const { page, device, advanceColumn } = await openPicker(t, 'ANDROID-PICKER-COLUMNS-001');
  const tab = page.view.css('button').nth(1);
  await expect(tab).toHaveText('multiSelector');
  await tab.click();
  await expect.poll(async () => (await page.data()).activeTab).toBe('multiSelector');
  const trigger = page.view.testId('picker-time-trigger');
  await expect(trigger).toContainText('09:30');
  await trigger.click();
  await advanceColumn('09', '10');
  await expect.poll(async () => (await page.data()).multiTime).toEqual(['10', '30']);
  await advanceColumn('30', '31');
  await expect.poll(async () => (await page.data()).multiTime).toEqual(['10', '31']);
  await device.tapText('OK');
  await expect.poll(async () => (await page.data()).multiTime).toEqual(['10', '31']);
  await expect(trigger).toContainText('10:31');
  expect((await device.nodes()).some((node) => node.text === 'OK')).toBe(false);
});
