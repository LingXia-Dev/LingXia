import { expect, spec } from '@lingxia/test';
import { harmonyDevice, type HarmonyNode } from '@lingxia/test/showcase/helpers/harmony-device.js';

const centre = ({ bounds: [l, t, r, b] }: HarmonyNode): [number, number] => [Math.round((l + r) / 2), Math.round((t + b) / 2)];
const sum = (rgb: number[]) => rgb[0] + rgb[1] + rgb[2];

spec.configure({ requires: { args: ['harmonyDevice'] } });

// Around a GPU page the nav bar takes the page palette's surface colour, which a card on the page
// also uses, in either scheme; the page config names no colour, so its defaults would differ in dark.
spec('the nav bar takes the GPU page palette in light and in dark', {
  id: 'CHROME-NAV-PALETTE-001', covers: ['lx.host.control.appearance.setPreference'],
  reason: 'reads the composited screen through the Harmony device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const app = t.app;
  const device = harmonyDevice(t);
  const previous = await app.logic.eval(({ lx }) => lx.host.control?.appearance.getPreference() ?? null);
  if (previous === null) t.skip('only the Control app sets the appearance');
  await app.view.testId('lab-tab-feedback').click();
  const colours = async () => {
    const layout = await device.nodes();
    const title = layout.filter(node => ['View lab', '视图实验'].includes(node.text)).sort((a, b) => a.bounds[1] - b.bounds[1])[0];
    // The Overlays card's heading: its row past the word is the card's own surface.
    const card = layout.find(node => node.text === 'Overlays');
    if (!title || !card) throw new Error('no nav title or overlays card on screen');
    const width = Math.max(...layout.map(node => node.bounds[2]));
    const [nav, surface] = await device.pixels([[Math.round(width * 0.1), centre(title)[1]], [card.bounds[2] - 20, centre(card)[1]]]);
    return { nav, surface };
  };
  try {
    for (const scheme of ['dark', 'light'] as const) {
      await app.logic.eval(async ({ lx }, scheme) => { await lx.host.control!.appearance.setPreference(scheme); }, scheme);
      let last = { nav: [0, 0, 0], surface: [0, 0, 0] };
      await expect.poll(async () => {
        last = await colours();
        return (scheme === 'dark') === (sum(last.surface) < 200) && last.nav.every((v, i) => Math.abs(v - last.surface[i]) <= 6)
          ? 'match' : `nav ${last.nav} surface ${last.surface}`;
      }, { timeout: 10_000, message: `in ${scheme} the nav bar is the page's surface colour` }).toBe('match');
    }
  } finally {
    if (previous !== null) await app.logic.eval(async ({ lx }, previous) => { await lx.host.control!.appearance.setPreference(previous); }, previous);
  }
});
