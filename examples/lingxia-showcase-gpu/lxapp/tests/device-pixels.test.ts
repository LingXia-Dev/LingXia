import { expect, spec } from '@lingxia/test';

// A simulated phone gives the page the device's own viewport and pixel ratio whatever screen
// the Runner's window is on, so a page screenshot is the device's pixels, the same every time.
spec('a page on each phone preset measures and renders as the device does', {
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
      await expect.poll(async () => {
        const { width, height } = await view.viewport();
        return [width, height];
      }, { message: `${preset.id} lays the page out in its own viewport` }).toEqual([preset.width, preset.height]);
      // A screenshot is the last presented frame: the first at the new size follows the layout.
      const pixels = [preset.width * preset.pixelRatio, preset.height * preset.pixelRatio];
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

// What the iOS host gives a page on each phone, in points: the safe-area top its navigation bar
// sits below (the status bar is shorter on Dynamic Island phones), the 44 pt bar, the 64 pt tab
// bar, the 34 pt home indicator; in landscape the sensor housing's side and a 21 pt indicator.
const SAFE_TOP: Record<string, number> = {
  'iphone-13-mini': 50, 'iphone-13-pro': 47, 'iphone-15-pro': 59, 'iphone-11': 48, 'iphone-15-pro-max': 59,
};
const NAVIGATION_BAR = 44;
const TAB_BAR = 64;

// Insets come from the preset alone: the same numbers on a 1x monitor with the phone at full
// size as on a Retina laptop that shrinks it to fit, so this spec passes on both unchanged.
spec('a page on each phone preset gets the safe area and bars its device gives it', {
  id: 'DEVICE-CHROME-001', covers: ['view:environment'], start: { page: 'view' },
}, async (t) => {
  const device = t.automation.device;
  const presets = await device.list().catch(() => null);
  if (!presets) return t.skip('no simulated device: not a Runner');
  const before = await device.get();
  if (before.group !== 'phone') return t.skip('a desktop or tablet preset: changing to a phone restarts Logic');
  const root = t.app.view.testId('view-index');
  const read = async () => {
    const edges = ['top', 'right', 'bottom', 'left'] as const;
    const of = async (kind: string) => Promise.all(edges.map(async edge => Number(await root.getAttribute(`data-${kind}-${edge}`))));
    return { safe: await of('safe'), insets: await of('inset') };
  };
  try {
    for (const preset of presets.filter(preset => preset.group === 'phone')) {
      const top = SAFE_TOP[preset.id];
      expect(top, `${preset.id} has a pinned safe area`).toBeDefined();
      for (const landscape of [false, true]) {
        await device.set({ id: preset.id, landscape });
        const want = landscape
          ? { safe: [0, top, 21, top], insets: [NAVIGATION_BAR, top, TAB_BAR, top] }
          : { safe: [top, 0, 34, 0], insets: [top + NAVIGATION_BAR, 0, TAB_BAR, 0] };
        await expect.poll(read, { message: `${preset.id} ${landscape ? 'landscape' : 'portrait'}: [top, right, bottom, left]` }).toEqual(want);
      }
    }
  } finally {
    await device.set({ id: before.id, landscape: before.landscape });
  }
});
