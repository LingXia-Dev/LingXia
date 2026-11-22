import { expect, test } from '@lingxia/test';

// A simulated phone gives the page the device's own viewport and pixel ratio whatever screen
// the Runner's window is on, so a page screenshot is the device's pixels, the same every time.
test('a page on each phone preset measures and renders as the device does', {
  id: 'DEVICE-PIXELS-001', covers: ['view:environment'], start: { page: 'view' },
}, async (t) => {
  const device = t.automation.device;
  const presets = await device.list().catch(() => null);
  if (!presets) return t.skip('no simulated device: not a Runner');
  const before = await device.get();
  if (before.group !== 'phone') return t.skip('a desktop or tablet preset: changing to a phone restarts Logic');
  const view = t.app.view;
  try {
    for (const preset of presets.filter(preset => preset.group === 'phone')) {
      await device.set({ id: preset.id, landscape: false });
      // An Android or HarmonyOS phone's viewport is its pixels over its density: compare to a hundredth.
      const hundredths = (values: number[]) => values.map(value => Math.round(value * 100) / 100);
      await expect.poll(async () => {
        const { width, height } = await view.viewport();
        return hundredths([width, height]);
      }, { message: `${preset.id} lays the page out in its own viewport` }).toEqual(hundredths([preset.width, preset.height]));
      // A screenshot is the last presented frame: the first at the new size follows the layout.
      const pixels = [Math.round(preset.width * preset.pixelRatio), Math.round(preset.height * preset.pixelRatio)];
      const size = async () => {
        const shot = await view.screenshot();
        return [shot.width, shot.height];
      };
      await expect.poll(size, { message: `${preset.id} presents the device's pixels` }).toEqual(pixels);
      // The same on every call from then on.
      expect(await size(), `${preset.id}, again`).toEqual(pixels);
    }
  } finally {
    await device.set({ id: before.id, landscape: before.landscape });
  }
});

// What each phone's host gives a page, in points: the device's safe area `[top, right, bottom,
// left]` in portrait and landscape, below the top of which the host draws its 44-point navigation
// bar, and the host's tab bar, whose reach above the bottom edge differs by host. The Android and
// HarmonyOS safe areas marked `provisional` in devices.json are not yet read off the phones.
type Edges = [number, number, number, number];
const PHONES: Record<string, { host: 'ios' | 'android' | 'harmony'; portrait: Edges; landscape: Edges }> = {
  'iphone-13-mini': { host: 'ios', portrait: [50, 0, 34, 0], landscape: [0, 50, 21, 50] },
  'iphone-13-pro': { host: 'ios', portrait: [47, 0, 34, 0], landscape: [0, 47, 21, 47] },
  'iphone-15-pro': { host: 'ios', portrait: [59, 0, 34, 0], landscape: [0, 59, 21, 59] },
  'iphone-11': { host: 'ios', portrait: [48, 0, 34, 0], landscape: [0, 48, 21, 48] },
  'iphone-15-pro-max': { host: 'ios', portrait: [59, 0, 34, 0], landscape: [0, 59, 21, 59] },
  'huawei-nova-13': { host: 'harmony', portrait: [39, 0, 28, 0], landscape: [0, 0, 28, 39] },
};
const NAVIGATION_BAR = 44;
/** The tab bar's reach above the bottom edge: its items' band plus the system's bottom area. iOS
 * grows its 64 through the home indicator, Android stacks its 64 on the system bar, HarmonyOS grows
 * its 72 to clear the gesture zone. */
const TAB_BAR_REACH = {
  ios: (bottom: number) => 64 + bottom,
  android: (bottom: number) => 64 + bottom,
  harmony: (bottom: number) => 72 + Math.max(0, Math.max(12, bottom - 6) - 12),
};

// Insets come from the preset alone: the same numbers on a 1x monitor with the phone at full
// size as on a Retina laptop that shrinks it to fit, so this spec passes on both unchanged.
test('a page on each phone preset gets the safe area and bars its device gives it', {
  id: 'DEVICE-CHROME-001', covers: ['view:environment'], start: { page: 'view' },
}, async (t) => {
  const device = t.automation.device;
  const presets = await device.list().catch(() => null);
  if (!presets) return t.skip('no simulated device: not a Runner');
  const before = await device.get();
  if (before.group !== 'phone') return t.skip('a desktop or tablet preset: changing to a phone restarts Logic');
  const root = t.app.view.testId('view-index');
  // A density of 2.75 carries points through pixels: compare to a hundredth.
  const hundredths = (values: number[]) => values.map(value => Math.round(value * 100) / 100);
  const read = async () => {
    const edges = ['top', 'right', 'bottom', 'left'] as const;
    const of = async (kind: string) => hundredths(await Promise.all(edges.map(async edge => Number(await root.getAttribute(`data-${kind}-${edge}`)))));
    return { safe: await of('safe'), insets: await of('inset') };
  };
  try {
    for (const preset of presets.filter(preset => preset.group === 'phone')) {
      const phone = PHONES[preset.id];
      expect(phone, `${preset.id} has a pinned safe area`).toBeDefined();
      for (const landscape of [false, true]) {
        await device.set({ id: preset.id, landscape });
        const safe = landscape ? phone.landscape : phone.portrait;
        const reach = TAB_BAR_REACH[phone.host](safe[2]);
        const want = { safe, insets: [safe[0] + NAVIGATION_BAR, safe[1], Math.max(safe[2], reach), safe[3]] };
        await expect.poll(read, { message: `${preset.id} ${landscape ? 'landscape' : 'portrait'}: [top, right, bottom, left]` }).toEqual(want);
      }
    }
  } finally {
    await device.set({ id: before.id, landscape: before.landscape });
  }
});
