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
